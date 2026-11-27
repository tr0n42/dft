//! Heat-sink bots (Benjamin, 25 Nov; docs/design/ground-units.md), two roles:
//!
//! - **Small pools**: it settles over a pool of magma (or molten metal or
//!   sulfur) small enough for its tentacles to reach all of, and cools it
//!   completely, so it kills no cutters.
//! - **Deep**: at a breach a tunneler capped (or a wide pool under open
//!   space, where it makes its own cap), it comes down the shaft and freezes
//!   a roughly vertical passage through the magma, wide enough for a
//!   tunneler, with frozen walls either side. Its own crew of cutters (built
//!   with it) digs out the middle behind it and never the walls. Once
//!   through the layer, tunnelers may bore on from the passage's bottom.
//!
//! Cooling never fails; the hotter the magma, the longer each cell takes.
//! No power beamed yet (the economy is undecided). With nothing left to do
//! it powers down and is left as scrap for the cutters to reclaim. Magma
//! reaching its body wrecks it like any other craft.
//!
//! Unit file: the platform calls `bots_work` once a game second (and
//! `scan_molten` now and then).

use super::*;

/// What a heat-sink bot is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatState {
    /// Flying to its pool, or to the mouth of its shaft.
    Fly = 0,
    /// Cooling a small pool, or freezing a passage.
    Work = 1,
    /// (Old saves only.)
    Leave = 2,
    /// Coming down the shaft to its job.
    Down = 3,
}

#[derive(Debug, Clone)]
pub struct HeatBot {
    pub(super) id: u32,
    pub(super) state: HeatState,
    pub(super) pos: (f64, f64),
    pub(super) prev: (f64, f64),
    pub(super) moved_at: u64,
    /// The pool's surface cell it works, and where it hovers over it.
    pub(super) spot: (i64, i64),
    pub(super) hover: (f64, f64),
    /// How far its tentacles reach (m), seconds of life (unused since 25
    /// Nov: cooling never fails), seconds with nothing to freeze, kg frozen.
    pub(super) reach: f64,
    pub(super) life: f64,
    pub(super) idle_s: f64,
    pub(super) frozen_kg: f64,
    /// Where its tentacles last froze (up to six), for drawing.
    pub(super) tips: Vec<(i64, i64)>,
    /// Its deep job (a heat job's id), the way down to it (a shaft's line)
    /// and how far along it is; how far down the passage is frozen past the
    /// cap (m), and dug out by its crew (m). Cells' worth of freezing banked.
    pub(super) job: Option<u32>,
    pub(super) path: Vec<(f64, f64)>,
    pub(super) along: usize,
    pub(super) front_m: f64,
    pub(super) dug_m: f64,
    pub(super) bank: f64,
    /// Metres in a row the front has found no hot liquid (through the layer
    /// at CLEAR_M).
    pub(super) clear_m: f64,
}

/// What a heat sink chills the cells it freezes to (K).
pub(super) const CHILL_K: f64 = 300.0;

/// A passage is through once its front has gone this far with no hot
/// liquid across it.
pub(super) const CLEAR_M: f64 = 6.0;

/// A pool is small (one bot cools it all from one spot) if it has at most
/// this many cells, all within its tentacles' reach.
pub(super) const SMALL_POOL_CELLS: usize = 1500;

impl Sim {
    /// Look round the working flocks for open pools of magma: hot liquid
    /// cells with open space over them. Small ones are pools to cool; wide
    /// ones under open space become deep jobs (a passage down through them,
    /// as many as fit, 40 m apart).
    pub(super) fn scan_molten(&mut self) {
        let mut found: Vec<(i64, i64)> = Vec::new();
        let sites = self.live_sites();
        let step = (sites.len() / 64).max(1);
        let reach = self.eco.units.heatsink.reach_m;
        for &i in sites.iter().step_by(step) {
            let (hx, hy) = self.site_pos(i);
            let a = hy.atan2(hx);
            let g = self.ground_at(a, true);
            let c0 = cell_of((a.cos() * g, a.sin() * g));
            for dy in (-60i64..=60).step_by(2) {
                for dx in (-60i64..=60).step_by(2) {
                    let c = (c0.0 + dx, c0.1 + dy);
                    let m = self.world.material(c.0, c.1);
                    if m == VACUUM || self.units.freeze_to.get(m as usize).copied().flatten().is_none() {
                        continue;
                    }
                    let up = unit_of(mid(c));
                    let above = cell_of((mid(c).0 + up.0 * 1.2, mid(c).1 + up.1 * 1.2));
                    if self.world.material(above.0, above.1) != VACUUM {
                        continue;
                    }
                    if found.iter().any(|f| (f.0 - c.0).pow(2) + (f.1 - c.1).pow(2) < 30 * 30) {
                        continue;
                    }
                    if self.units.heat_jobs.iter().any(|j| dist2(j.at, mid(c)) < 40.0 * 40.0) {
                        continue;
                    }
                    found.push(c);
                    if found.len() >= 64 {
                        break;
                    }
                }
            }
        }
        // Small pools to cool; wide ones get deep jobs.
        let mut pools = Vec::new();
        for c in found {
            if self.pool_small(c, reach) {
                pools.push(c);
            } else {
                // Its spot: walked up from the pool's surface to open space
                // (two clear cells), so a bot hovering there is clear of it.
                let p = mid(c);
                let up = unit_of(p);
                let mut h = 0.5;
                let mut clear = 0;
                while h < 30.0 && clear < 2 {
                    let q = cell_of((p.0 + up.0 * h, p.1 + up.1 * h));
                    clear = if self.world.material(q.0, q.1) == VACUUM { clear + 1 } else { 0 };
                    h += 0.5;
                }
                if clear < 2 {
                    continue;
                }
                let at = (p.0 + up.0 * (h - 1.0), p.1 + up.1 * (h - 1.0));
                let top = (p.0 + up.0 * (h + 8.0), p.1 + up.1 * (h + 8.0));
                let id = self.units.new_id();
                // Its "shaft": the open space straight down to the pool.
                self.units.heat_jobs.push(HeatJob { id, at, down: (-up.0, -up.1), shaft: vec![top], taken: false, through: None, used: false, wrecks: 0 });
            }
        }
        self.units.molten = pools;
    }

    /// Whether the pool of hot liquid at `c` is small: at most
    /// SMALL_POOL_CELLS cells, all within `reach` of it.
    fn pool_small(&self, c: (i64, i64), reach: f64) -> bool {
        let hot = |q: (i64, i64)| {
            let m = self.world.material(q.0, q.1);
            m != VACUUM && self.units.freeze_to.get(m as usize).copied().flatten().is_some()
        };
        let mut seen = CellSet::default();
        let mut queue = std::collections::VecDeque::new();
        seen.insert(c);
        queue.push_back(c);
        let r2 = reach * reach;
        while let Some(q) = queue.pop_front() {
            if ((q.0 - c.0).pow(2) + (q.1 - c.1).pow(2)) as f64 > r2 {
                return false;
            }
            if seen.len() > SMALL_POOL_CELLS {
                return false;
            }
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (q.0 + d.0, q.1 + d.1);
                if !seen.contains(&n) && hot(n) {
                    seen.insert(n);
                    queue.push_back(n);
                }
            }
        }
        true
    }

    /// Work waiting for heat sinks: small pools no bot is on, and deep jobs
    /// no bot has taken.
    pub(super) fn heat_work_waiting(&self) -> usize {
        let pools = self.units.molten.iter().filter(|c| !self.units.bots.iter().any(|b| b.job.is_none() && (b.spot.0 - c.0).pow(2) + (b.spot.1 - c.1).pow(2) < 40 * 40)).count();
        let jobs = self.units.heat_jobs.iter().filter(|j| !j.taken && j.through.is_none()).count();
        pools + jobs
    }

    pub(super) fn launch_bot(&mut self) {
        let id = self.units.new_id();
        let mut b = HeatBot { id, state: HeatState::Fly, pos: (0.0, 0.0), prev: (0.0, 0.0), moved_at: self.tick, spot: (0, 0), hover: (0.0, 0.0), reach: 2.0, life: f64::INFINITY, idle_s: 0.0, frozen_kg: 0.0, tips: Vec::new(), job: None, path: Vec::new(), along: 0, front_m: 0.0, dug_m: 0.0, bank: 0.0, clear_m: 0.0 };
        if !self.next_heat_job(&mut b) {
            return;
        }
        let start = self.mother_over(b.hover.1.atan2(b.hover.0));
        b.pos = start;
        b.prev = start;
        self.units.bots.push(b);
    }

    /// Give bot `b` its next job: a deep job first (deep jobs are what let
    /// the swarm through), else the nearest small pool. False: none.
    fn next_heat_job(&mut self, b: &mut HeatBot) -> bool {
        let from = if b.pos == (0.0, 0.0) { self.mother_pos() } else { b.pos };
        let job = self.units.heat_jobs.iter().enumerate().filter(|(_, j)| !j.taken && j.through.is_none()).min_by(|x, y| dist2(x.1.at, from).total_cmp(&dist2(y.1.at, from))).map(|(k, _)| k);
        if let Some(k) = job {
            let j = &mut self.units.heat_jobs[k];
            j.taken = true;
            b.job = Some(j.id);
            b.path = j.shaft.clone();
            // (It stops 6 m short of the cap: over the work, not on it.)
            b.path.push((j.at.0 - j.down.0 * 6.0, j.at.1 - j.down.1 * 6.0));
            b.along = 0;
            b.front_m = 0.0;
            b.dug_m = 0.0;
            b.bank = 0.0;
            b.clear_m = 0.0;
            b.spot = cell_of(j.at);
            b.hover = b.path[0];
            b.state = HeatState::Fly;
            b.reach = 2.0;
            b.idle_s = 0.0;
            return true;
        }
        let busy = |c: &(i64, i64), bots: &Vec<HeatBot>| bots.iter().any(|o| o.id != b.id && o.job.is_none() && (o.spot.0 - c.0).pow(2) + (o.spot.1 - c.1).pow(2) < 40 * 40);
        let pool = self.units.molten.iter().copied().filter(|c| !busy(c, &self.units.bots)).min_by(|x, y| dist2(mid(*x), from).total_cmp(&dist2(mid(*y), from)));
        if let Some(spot) = pool {
            let up = unit_of(mid(spot));
            b.job = None;
            b.path.clear();
            b.spot = spot;
            b.hover = (mid(spot).0 + up.0 * 7.0, mid(spot).1 + up.1 * 7.0);
            b.state = HeatState::Fly;
            b.reach = 2.0;
            b.idle_s = 0.0;
            return true;
        }
        false
    }

    /// Hot liquid at `p` or just under it.
    fn hot_at(&self, p: (f64, f64)) -> bool {
        let down = unit_of(p);
        [0.0, 1.0].iter().any(|k| {
            let c = cell_of((p.0 - down.0 * k, p.1 - down.1 * k));
            let m = self.world.material(c.0, c.1);
            m != VACUUM && self.units.freeze_to.get(m as usize).copied().flatten().is_some()
        })
    }

    /// Cells' worth of freezing a cell of `m` takes: 1 at its freezing
    /// point, more the hotter it is (Benjamin: heat only sets how long a
    /// cell takes).
    fn freeze_cost(&self, c: (i64, i64), m: MatId) -> f64 {
        let fk = self.world.planet.mats.get(m).freezes_at_k.unwrap_or(1000.0);
        let t = self.sandbox.temps.get(&c).copied().unwrap_or(fk + 300.0);
        (1.0 + (t - fk).max(0.0) / 600.0).min(4.0)
    }

    /// Freeze cell `c` (hot liquid of `m`) to what it freezes to; kg frozen.
    fn freeze_cell(&mut self, c: (i64, i64), m: MatId) -> f64 {
        let Some(to) = self.units.freeze_to.get(m as usize).copied().flatten() else { return 0.0 };
        if self.world.change_cell(c.0, c.1, to).is_none() {
            return 0.0;
        }
        self.apply_sandbox(vec![Change { cell: c, from: m, to }]);
        let kg = self.world.planet.mats.get(m).density;
        // Chilled through (the bot has pulled its heat out) and left at rest:
        // the sandbox would otherwise follow every frozen cell's heat beside
        // the magma, thousands of them (on the lava world that took the
        // heat simulation from 5 to 400 ms a game second), and the walls
        // would melt back.
        self.sandbox.temps.insert(c, CHILL_K);
        self.units.frozen_kg += kg;
        kg
    }

    pub(super) fn bots_work(&mut self) {
        if self.units.bots.is_empty() {
            return;
        }
        let hs = self.eco.units.heatsink.clone();
        let mut gone: Vec<(u32, (f64, f64), bool)> = Vec::new();
        for k in 0..self.units.bots.len() {
            let mut b = self.units.bots[k].clone();
            b.prev = b.pos;
            b.moved_at = self.tick;
            // Magma reaching its body wrecks it, as any craft.
            {
                let c = cell_of(b.pos);
                let m = self.world.material(c.0, c.1);
                if b.state != HeatState::Fly && m != VACUUM && self.units.freeze_to.get(m as usize).copied().flatten().is_some() {
                    gone.push((b.id, b.pos, true));
                    self.units.bots[k] = b;
                    continue;
                }
            }
            // Where it is going has filled with magma (a pool risen since the
            // job was found): the job is stale; it gives it up and takes
            // the next (never flying into magma).
            let going = match b.state {
                HeatState::Fly | HeatState::Leave => Some(b.hover),
                HeatState::Down => b.path.get(b.along).copied(),
                _ => None,
            };
            if let Some(p) = going.filter(|_| b.job.is_some()) {
                if self.hot_at(p) {
                    let id = b.job.unwrap_or(0);
                    if let Some(j) = self.units.heat_jobs.iter_mut().find(|j| j.id == id) {
                        j.wrecks = 2;
                    }
                    self.units.heat_jobs.retain(|j| j.wrecks < 2 || j.through.is_some());
                    self.units.stale_jobs += 1;
                    if !self.next_heat_job(&mut b) {
                        gone.push((b.id, b.pos, false));
                    }
                    self.units.bots[k] = b;
                    continue;
                }
            }
            match b.state {
                HeatState::Fly | HeatState::Leave => {
                    let mut p = b.pos;
                    let there = self.fly_over(&mut p, b.hover, hs.speed, 40.0);
                    b.pos = p;
                    if there {
                        b.state = if b.job.is_some() { HeatState::Down } else { HeatState::Work };
                        b.along = 0;
                    }
                }
                HeatState::Down => {
                    // Down the shaft's line to the cap.
                    let mut step = hs.speed * 0.5;
                    while step > 0.0 {
                        let Some(&to) = b.path.get(b.along) else { break };
                        let d = dist2(b.pos, to).sqrt();
                        if d > step {
                            approach(&mut b.pos, to, step);
                            break;
                        }
                        b.pos = to;
                        step -= d;
                        b.along += 1;
                    }
                    if b.along >= b.path.len() {
                        b.state = HeatState::Work;
                    }
                }
                HeatState::Work => {
                    b.reach = (b.reach + hs.grow_m_per_s).min(hs.reach_m);
                    if b.job.is_some() {
                        self.bot_passage(&mut b, &hs);
                    } else {
                        self.bot_pool(&mut b, &hs);
                    }
                    if b.idle_s > 10.0 {
                        // Done here: the next job, or it powers down.
                        if !self.next_heat_job(&mut b) {
                            gone.push((b.id, b.pos, false));
                        }
                    }
                }
            }
            self.units.bots[k] = b;
        }
        for (id, at, wrecked) in gone {
            self.units.bots.retain(|b| b.id != id);
            if wrecked {
                self.units.bots_lost += 1;
                self.explode(at, 12, 10.0);
            } else {
                // Powered down: left as scrap for the cutters to reclaim.
                self.units.bots_retired += 1;
                for _ in 0..6 {
                    if let Some(c) = self.bodies.wreck(&mut self.world, at.0, at.1) {
                        self.scrap_landed(c);
                    }
                }
            }
            // A deep job it was on goes back to the list (one that has
            // wrecked two bots is given up).
            let bots = &self.units.bots;
            for j in self.units.heat_jobs.iter_mut() {
                if j.taken && j.through.is_none() && !bots.iter().any(|b| b.job == Some(j.id)) {
                    j.taken = false;
                    if wrecked {
                        j.wrecks += 1;
                    }
                }
            }
            self.units.heat_jobs.retain(|j| j.wrecks < 2 || j.through.is_some());
        }
    }

    /// A second cooling a small pool: the hot cells nearest its tentacles
    /// (downward first), as many as it can freeze a second.
    fn bot_pool(&mut self, b: &mut HeatBot, hs: &crate::economy::HeatSinkSpec) {
        let ri = b.reach.ceil() as i64;
        let down = unit_of(mid(b.spot));
        let mut cands: Vec<(f64, (i64, i64), MatId)> = Vec::new();
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d2 = (dx * dx + dy * dy) as f64;
                if d2 > b.reach * b.reach {
                    continue;
                }
                let c = (b.spot.0 + dx, b.spot.1 + dy);
                let m = self.world.material(c.0, c.1);
                if self.units.freeze_to.get(m as usize).copied().flatten().is_none() {
                    continue;
                }
                let out = (dx as f64 * down.0 + dy as f64 * down.1).max(0.0);
                cands.push((d2.sqrt() + 0.5 * out, c, m));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        // Still growing its tentacles out: not done until they are full length.
        if cands.is_empty() && b.reach >= hs.reach_m {
            b.idle_s += 1.0;
        } else {
            b.idle_s = 0.0;
        }
        b.tips.clear();
        b.bank += hs.freeze_per_s as f64;
        for &(_, c, m) in &cands {
            let cost = self.freeze_cost(c, m);
            if b.bank < cost {
                break;
            }
            b.bank -= cost;
            b.frozen_kg += self.freeze_cell(c, m);
            if b.tips.len() < 6 {
                b.tips.push(c);
            }
        }
        b.bank = b.bank.min(hs.freeze_per_s as f64 * 4.0);
    }

    /// A second on a deep job: freeze the passage on down past the cap (as
    /// wide as a tunneler, with walls `wall_m` thick either side), its crew
    /// digging out the middle behind the freezing; through once there is
    /// solid ground under the magma.
    fn bot_passage(&mut self, b: &mut HeatBot, hs: &crate::economy::HeatSinkSpec) {
        let Some(k) = self.units.heat_jobs.iter().position(|j| Some(j.id) == b.job) else {
            b.idle_s = 99.0;
            return;
        };
        let (at, down) = (self.units.heat_jobs[k].at, self.units.heat_jobs[k].down);
        let side = (-down.1, down.0);
        let inner = self.eco.units.tunneler.bore_m / 2.0;
        let outer = inner + hs.wall_m;
        b.tips.clear();
        b.bank += hs.freeze_per_s as f64;
        // Freezing: the slab at the front, the whole width (walls too), cell
        // by cell; the front moves on once a slab holds no hot liquid.
        // (Two rows a metre, half a metre apart across: a slanting passage
        // leaves no gaps in its walls for the magma to run through.)
        let mut moved = 0;
        // (Once through the layer the front stops; the crew catches up.)
        while moved < 4 && b.clear_m < CLEAR_M {
            let (mut left, mut hot) = (0, 0);
            for sub in [0.0, 0.5] {
                let mut s = -outer;
                while s <= outer {
                    let along = b.front_m + sub;
                    let p = (at.0 + down.0 * along + side.0 * s, at.1 + down.1 * along + side.1 * s);
                    let c = cell_of(p);
                    s += 0.5;
                    let m = self.world.material(c.0, c.1);
                    if self.units.freeze_to.get(m as usize).copied().flatten().is_none() {
                        continue;
                    }
                    hot += 1;
                    let cost = self.freeze_cost(c, m);
                    if b.bank < cost {
                        left += 1;
                        continue;
                    }
                    b.bank -= cost;
                    b.frozen_kg += self.freeze_cell(c, m);
                    if b.tips.len() < 6 {
                        b.tips.push(c);
                    }
                }
            }
            if left > 0 {
                break;
            }
            // (Clear metres count only past the tunneler's cap, which froze
            // the first few metres already.)
            let past_cap = b.front_m > self.eco.units.tunneler.bore_m / 2.0 + self.eco.units.tunneler.cap_m + 2.0;
            b.clear_m = if hot == 0 && past_cap { b.clear_m + 1.0 } else { 0.0 };
            b.front_m += 1.0;
            moved += 1;
            if b.front_m > hs.passage_max_m || b.clear_m >= CLEAR_M {
                break;
            }
        }
        b.bank = b.bank.min(hs.freeze_per_s as f64 * 4.0);
        // Its crew digs the middle out behind the freezing front (never the
        // walls), as fast as its cutters can carry it off.
        let mut can = hs.crew as f64 * hs.crew_kg_per_s;
        while b.dug_m + 1.0 < b.front_m && can > 0.0 {
            let mut kg_row = 0.0;
            for sub in [0.25, 0.75] {
            let mut s = -inner;
            while s <= inner {
                let p = (at.0 + down.0 * (b.dug_m + sub) + side.0 * s, at.1 + down.1 * (b.dug_m + sub) + side.1 * s);
                let c = cell_of(p);
                s += 0.5;
                let m = self.world.material(c.0, c.1);
                if m == VACUUM || self.world.planet.mats.get(m).state != State::Solid {
                    continue;
                }
                let Some(lifted) = self.world.lift_cell(c.0, c.1) else { continue };
                self.progress.remove(&c);
                self.reserved.remove(&c);
                self.open_cell(c);
                self.cell_mined(c);
                self.credit(lifted, 1.0);
                kg_row += self.world.planet.mats.get(lifted).density;
            }
            }
            can -= kg_row.max(1.0);
            b.dug_m += 1.0;
        }
        // It hovers 6 m over the dug face (over the cap, at first), never
        // into magma.
        let to = (at.0 + down.0 * (b.dug_m - 6.0), at.1 + down.1 * (b.dug_m - 6.0));
        let mut next = b.pos;
        approach(&mut next, to, hs.speed * 0.25);
        let nc = cell_of(next);
        let nm = self.world.material(nc.0, nc.1);
        if nm == VACUUM || self.units.freeze_to.get(nm as usize).copied().flatten().is_none() {
            b.pos = next;
        }
        // Through: the front has gone CLEAR_M with no hot liquid across it
        // (under the layer), and the crew has caught up.
        if (b.clear_m >= CLEAR_M && b.dug_m + 2.0 >= b.front_m) || b.front_m > hs.passage_max_m {
            let bottom = (at.0 + down.0 * b.dug_m, at.1 + down.1 * b.dug_m);
            let j = &mut self.units.heat_jobs[k];
            let mut shaft = j.shaft.clone();
            shaft.push(j.at);
            shaft.push(bottom);
            j.shaft = shaft;
            j.through = Some(bottom);
            b.idle_s = 99.0;
            self.units.passages += 1;
            self.director_log_push(format!("a heat sink has frozen a passage {:.0} m through the magma: tunnelers may go on down", b.front_m));
        }
    }
}
