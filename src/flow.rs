//! Flow fields: how far it is to fly from a flock's hauler to anywhere near
//! it through open space (D6).
//!
//! Drones don't each search for a path: that would be far too much with
//! hundreds of thousands of them. Each flock has one field instead, a
//! sweep outward from its hauler over open ground in 2 m squares, rebuilt
//! every few seconds (all flocks due side by side on the simulation's
//! threads). From it a drone's flight to any cell is known at once: its
//! length (for choosing work and timing the flight), whether the cell can be
//! reached at all, and the way (downhill toward the hauler), which the game
//! draws drones following on screen.

/// Side of the field's squares (m).
pub const CELL: i64 = 2;
/// Cost of a step to a side neighbour and to a corner neighbour; 10 is one
/// square (2 m), so a unit is 0.2 m.
const STRAIGHT: u32 = 10;
const DIAG: u32 = 14;
const M_PER_UNIT: f64 = CELL as f64 / STRAIGHT as f64;
/// Unreached.
const FAR: u16 = u16::MAX;

/// One flock's field.
#[derive(Debug, Clone)]
pub struct FlowField {
    /// Where it was swept from (the hauler's spot then).
    pub hub: (f64, f64),
    /// The world position (m) of the lower-left corner of square (0, 0),
    /// and squares across (it is square).
    pub ox: i64,
    pub oy: i64,
    pub n: i64,
    /// Distance from the hub to each square's middle, in units (FAR: unreached).
    dist: Vec<u16>,
}

impl FlowField {
    /// Sweep outward from `hub` over squares `open(sx, sy)` says can be flown
    /// through (square (sx, sy) covers world cells sx*CELL .. sx*CELL+CELL),
    /// up to `radius` metres. Corners are only cut where both sides are open.
    pub fn build(hub: (f64, f64), radius: f64, mut open: impl FnMut(i64, i64) -> bool) -> FlowField {
        let r = (radius / CELL as f64).ceil() as i64;
        let n = 2 * r + 1;
        let (hx, hy) = ((hub.0.floor() as i64).div_euclid(CELL), (hub.1.floor() as i64).div_euclid(CELL));
        let (ox, oy) = ((hx - r) * CELL, (hy - r) * CELL);
        let size = (n * n) as usize;
        // Whether each square is open: 0 not looked at yet, 1 open, 2 not.
        let mut seen = vec![0u8; size];
        let mut dist = vec![FAR; size];
        let mut is_open = |i: usize, seen: &mut Vec<u8>| -> bool {
            if seen[i] == 0 {
                let (sx, sy) = ((i as i64) % n, (i as i64) / n);
                seen[i] = if open(ox / CELL + sx, oy / CELL + sy) { 1 } else { 2 };
            }
            seen[i] == 1
        };
        // A bucket queue (costs are small whole numbers): bucket d % 16 holds
        // squares at distance d.
        let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); 16];
        let start = (r * n + r) as usize;
        dist[start] = 0;
        buckets[0].push(start as u32);
        let mut pending = 1usize;
        let mut d: u32 = 0;
        while pending > 0 {
            let b = (d % 16) as usize;
            let list = std::mem::take(&mut buckets[b]);
            for &i in &list {
                pending -= 1;
                let i = i as usize;
                if dist[i] as u32 != d {
                    continue;
                }
                let (x, y) = ((i as i64) % n, (i as i64) / n);
                let mut side_open = [false; 4];
                for (k, (dx, dy)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= n || ny >= n {
                        continue;
                    }
                    let j = (ny * n + nx) as usize;
                    if !is_open(j, &mut seen) {
                        continue;
                    }
                    side_open[k] = true;
                    let nd = d + STRAIGHT;
                    if nd < dist[j] as u32 && nd < FAR as u32 {
                        dist[j] = nd as u16;
                        buckets[(nd % 16) as usize].push(j as u32);
                        pending += 1;
                    }
                }
                for (dx, dy, a, c) in [(1, 1, 0, 2), (1, -1, 0, 3), (-1, 1, 1, 2), (-1, -1, 1, 3)] {
                    if !(side_open[a] && side_open[c]) {
                        continue;
                    }
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= n || ny >= n {
                        continue;
                    }
                    let j = (ny * n + nx) as usize;
                    if !is_open(j, &mut seen) {
                        continue;
                    }
                    let nd = d + DIAG;
                    if nd < dist[j] as u32 && nd < FAR as u32 {
                        dist[j] = nd as u16;
                        buckets[(nd % 16) as usize].push(j as u32);
                        pending += 1;
                    }
                }
            }
            d += 1;
        }
        FlowField { hub, ox, oy, n, dist }
    }

    fn square(&self, x: f64, y: f64) -> Option<usize> {
        let sx = (x.floor() as i64 - self.ox).div_euclid(CELL);
        let sy = (y.floor() as i64 - self.oy).div_euclid(CELL);
        (sx >= 0 && sy >= 0 && sx < self.n && sy < self.n).then_some((sy * self.n + sx) as usize)
    }

    fn middle(&self, i: usize) -> (f64, f64) {
        let (sx, sy) = ((i as i64) % self.n, (i as i64) / self.n);
        ((self.ox + sx * CELL) as f64 + CELL as f64 / 2.0, (self.oy + sy * CELL) as f64 + CELL as f64 / 2.0)
    }

    /// Flying distance (m) from the hub to point (x, y), if it was reached.
    pub fn dist_m(&self, x: f64, y: f64) -> Option<f64> {
        let i = self.square(x, y)?;
        (self.dist[i] != FAR).then(|| self.dist[i] as f64 * M_PER_UNIT)
    }

    /// Flying distance (m) from the hub to beside cell c (a drone cuts from
    /// the open side): the nearest reached square touching it. None: it
    /// can't be reached from here.
    pub fn reach_m(&self, c: (i64, i64)) -> Option<f64> {
        // Its square and the eight round it (a cell on a jagged face may sit
        // in a square that is mostly rock, beside open ones), each a
        // square's width further (diagonals 1.4).
        let sx = (c.0 - self.ox).div_euclid(CELL);
        let sy = (c.1 - self.oy).div_euclid(CELL);
        let mut best = u32::MAX;
        for dy in -1i64..=1 {
            let y = sy + dy;
            if y < 0 || y >= self.n {
                continue;
            }
            for dx in -1i64..=1 {
                let x = sx + dx;
                if x < 0 || x >= self.n {
                    continue;
                }
                let d = self.dist[(y * self.n + x) as usize];
                if d == FAR {
                    continue;
                }
                let extra = if dx != 0 && dy != 0 { DIAG } else if dx != 0 || dy != 0 { STRAIGHT } else { 0 };
                best = best.min(d as u32 + extra);
            }
        }
        (best != u32::MAX).then(|| (best as f64 * M_PER_UNIT - CELL as f64).max(0.0))
    }

    /// The way from the hub to `to`, `s` metres along it (the way is found
    /// backwards, downhill from `to`'s square toward the hub). None if `to`
    /// wasn't reached.
    pub fn along(&self, to: (f64, f64), s: f64) -> Option<(f64, f64)> {
        let mut i = self.near_square(to)?;
        let total = self.dist[i] as f64 * M_PER_UNIT;
        if s >= total {
            return Some(to);
        }
        // Walk down from the far end until within `s` of the hub.
        let mut prev = to;
        let mut at_d = total;
        let mut guard = 0;
        while guard < 4096 {
            guard += 1;
            let next = self.downhill(i)?;
            let nd = self.dist[next] as f64 * M_PER_UNIT;
            let p = self.middle(next);
            if nd <= s {
                // Between prev (at at_d) and p (at nd).
                let k = if at_d > nd { (at_d - s) / (at_d - nd) } else { 1.0 };
                return Some((prev.0 + (p.0 - prev.0) * k, prev.1 + (p.1 - prev.1) * k));
            }
            prev = p;
            at_d = nd;
            i = next;
        }
        Some(self.hub)
    }

    /// The reached square for `p`: its own, or one touching it.
    fn near_square(&self, p: (f64, f64)) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            if let Some(i) = self.square(p.0 + dx, p.1 + dy).filter(|&i| self.dist[i] != FAR) {
                if best.is_none_or(|b| self.dist[i] < self.dist[b]) {
                    best = Some(i);
                }
            }
        }
        best
    }

    /// The neighbouring square nearest the hub.
    fn downhill(&self, i: usize) -> Option<usize> {
        let (x, y) = ((i as i64) % self.n, (i as i64) / self.n);
        let mut best: Option<usize> = None;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= self.n || ny >= self.n {
                continue;
            }
            let j = (ny * self.n + nx) as usize;
            if self.dist[j] < self.dist[i] && best.is_none_or(|b| self.dist[j] < self.dist[b]) {
                best = Some(j);
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goes_round_a_wall() {
        // A wall from y = -10 to 10 at x = 10 (2 m thick), open elsewhere.
        let wall = |sx: i64, sy: i64| !(sx == 5 && (-5..=5).contains(&sy));
        let f = FlowField::build((0.5, 0.5), 60.0, wall);
        let straight = 20.0;
        let d = f.dist_m(20.5, 0.5).unwrap();
        assert!(d > straight + 8.0, "should go round the wall: {d}");
        // Halfway along, the way is off to one side of the wall's ends.
        let p = f.along((20.5, 0.5), d / 2.0).unwrap();
        assert!(p.1.abs() >= 9.0, "the way should pass the end of the wall: {p:?}");
        // Sealed in: not reachable.
        let boxed = |sx: i64, sy: i64| !(sx.abs() == 10 && sy.abs() <= 10 || sy.abs() == 10 && sx.abs() <= 10) || (sx.abs() < 10 && sy.abs() < 10);
        let g = FlowField::build((0.5, 0.5), 60.0, boxed);
        assert!(g.dist_m(40.5, 0.5).is_none());
    }
}
