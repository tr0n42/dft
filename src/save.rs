//! Saves: the world's changes and books, run-length encoded, then deflate-compressed.
//!
//! A save holds only what the generator can't recreate: every node entry, the
//! dense leaves, and the ledger. The body itself is rebuilt from its spec and
//! seed, and a fingerprint makes sure a save is only loaded onto the same body.
//!
//! Layout: "DFTS" | version u32 | raw length u64 | deflate(raw).

use crate::ledger::Ledger;
use crate::noise::hash;
use crate::planet::Planet;
use crate::totals::LEAF;
use crate::world::{Entry, State, World};

const MAGIC: &[u8; 4] = b"DFTS";
const VERSION: u32 = 6; // 6: the ledger's grown column (life)

/// Identifies a body: same spec and seed, same fingerprint.
pub fn fingerprint(p: &Planet) -> u64 {
    let text = format!("{:?}", p.spec);
    let mut h = p.spec.seed;
    for b in text.bytes() {
        h = hash(h, b as i64, 0, 0xF1);
    }
    for m in &p.mats.list {
        h = hash(h, m.density.to_bits() as i64, m.name.len() as i64, 0xF2);
    }
    h
}

/// A plain little-endian writer (shared by the game's own save sections).
#[derive(Default)]
pub struct W(pub Vec<u8>);
impl W {
    pub fn i64(&mut self, v: i64) {
        self.u64(v as u64);
    }
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn bytes(&mut self, v: &[u8]) {
        self.u64(v.len() as u64);
        self.0.extend_from_slice(v);
    }
    pub fn text(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }
    pub fn opt_f64(&mut self, v: Option<f64>) {
        self.u8(v.is_some() as u8);
        self.f64(v.unwrap_or(0.0));
    }
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    pub fn f64s(&mut self, v: &[f64]) {
        self.u32(v.len() as u32);
        for x in v {
            self.f64(*x);
        }
    }
}

/// Reads what `W` wrote.
pub struct R<'a>(pub &'a [u8], pub usize);
impl<'a> R<'a> {
    pub fn i64(&mut self) -> Result<i64, String> {
        Ok(self.u64()? as i64)
    }
    pub fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.u64()? as usize;
        self.take(n)
    }
    pub fn text(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| "corrupt save: bad text".to_string())
    }
    pub fn opt_f64(&mut self) -> Result<Option<f64>, String> {
        let some = self.u8()? != 0;
        let v = self.f64()?;
        Ok(some.then_some(v))
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.1 + n > self.0.len() {
            return Err("save file is truncated".into());
        }
        let s = &self.0[self.1..self.1 + n];
        self.1 += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_bits(self.u64()?))
    }
    pub fn f64s(&mut self) -> Result<Vec<f64>, String> {
        let n = self.u32()? as usize;
        (0..n).map(|_| self.f64()).collect()
    }
}

/// Bytes before compression and the save itself.
pub fn save(w: &World) -> (usize, Vec<u8>) {
    let mut o = W(Vec::new());
    write_world(&mut o, w);
    o.f64s(&w.ledger.grown);
    let raw = o.0;
    let out = pack(MAGIC, VERSION, &raw);
    (raw.len(), out)
}

/// Compress raw save bytes behind a magic word and version:
/// magic | version u32 | raw length u64 | deflate(raw).
pub fn pack(magic: &[u8; 4], version: u32, raw: &[u8]) -> Vec<u8> {
    let packed = miniz_oxide::deflate::compress_to_vec(raw, 6);
    let mut out = Vec::with_capacity(packed.len() + 16);
    out.extend_from_slice(magic);
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&(raw.len() as u64).to_le_bytes());
    out.extend_from_slice(&packed);
    out
}

/// Undo `pack`, checking the magic word and version.
pub fn unpack(magic: &[u8; 4], version: u32, bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() < 16 || &bytes[0..4] != magic {
        return Err("not a Dark Forest Theory save".into());
    }
    let v = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if v != version {
        return Err(format!("save version {v} is not supported (this build reads {version})"));
    }
    miniz_oxide::inflate::decompress_to_vec(&bytes[16..]).map_err(|e| format!("corrupt save: {e:?}"))
}

/// The world's changes and books, uncompressed.
pub fn write_world(o: &mut W, w: &World) {
    o.u64(fingerprint(&w.planet));
    let l = &w.ledger;
    o.f64s(&l.initial);
    o.f64s(&l.mined);
    o.f64s(&l.reconciled);
    o.f64s(&l.changed);
    o.f64s(&l.vented);
    o.f64s(&l.aloft);
    o.f64s(&l.deposited);
    o.f64(l.credited);
    o.f64(l.bonus);
    o.f64(l.generated);
    o.u64(l.cell_ops);
    o.u64(l.node_ops);

    let mut keys: Vec<&u64> = w.nodes.keys().collect();
    keys.sort();
    o.u64(keys.len() as u64);
    for k in keys {
        let e = &w.nodes[k];
        o.u64(*k);
        // Totals as (material, cells) pairs, skipping zeros.
        let nz: Vec<(usize, f64)> = e.totals.iter().copied().enumerate().filter(|(_, c)| *c != 0.0).collect();
        o.u16(nz.len() as u16);
        for (m, c) in nz {
            o.u16(m as u16);
            o.f64(c);
        }
        match &e.state {
            State::Untouched => o.u8(1),
            State::Split => o.u8(2),
            State::Empty => o.u8(3),
            State::Uniform(m) => {
                o.u8(4);
                o.u16(*m);
            }
            State::Dense(cells) => {
                o.u8(5);
                // Run-length encoding: (material, run length) pairs.
                let mut runs: Vec<(crate::materials::MatId, u16)> = Vec::new();
                for &c in cells.iter() {
                    match runs.last_mut() {
                        Some((m, n)) if *m == c && *n < u16::MAX => *n += 1,
                        _ => runs.push((c, 1)),
                    }
                }
                o.u16(runs.len() as u16);
                for (m, n) in runs {
                    o.u16(m);
                    o.u16(n);
                }
            }
        }
    }
}

/// Rebuild a world from a save and the same body.
pub fn load(bytes: &[u8], planet: Planet) -> Result<World, String> {
    let raw = unpack(MAGIC, VERSION, bytes)?;
    let mut r = R(&raw, 0);
    let mut w = read_world(&mut r, std::sync::Arc::new(planet))?;
    w.ledger.grown = r.f64s()?;
    Ok(w)
}

/// Read what `write_world` wrote, onto the same body.
pub fn read_world(r: &mut R, planet: std::sync::Arc<Planet>) -> Result<World, String> {
    if r.u64()? != fingerprint(&planet) {
        return Err("this save belongs to a different body".into());
    }
    let nm = planet.mats.len();
    let ledger = Ledger {
        initial: r.f64s()?,
        mined: r.f64s()?,
        reconciled: r.f64s()?,
        changed: r.f64s()?,
        vented: r.f64s()?,
        aloft: r.f64s()?,
        deposited: r.f64s()?,
        // (Kept after the world by whoever saves it: older saves have none.)
        grown: vec![0.0; nm],
        credited: r.f64()?,
        bonus: r.f64()?,
        generated: r.f64()?,
        cell_ops: r.u64()?,
        node_ops: r.u64()?,
    };
    let count = r.u64()?;
    let mut nodes = crate::world::NodeMap::with_capacity_and_hasher(count as usize, Default::default());
    for _ in 0..count {
        let k = r.u64()?;
        let mut totals = vec![0.0; nm];
        for _ in 0..r.u16()? {
            let m = r.u16()? as usize;
            totals[m] = r.f64()?;
        }
        let state = match r.u8()? {
            1 => State::Untouched,
            2 => State::Split,
            3 => State::Empty,
            4 => State::Uniform(r.u16()?),
            5 => {
                let n = r.u16()?;
                let mut cells = Vec::with_capacity((LEAF * LEAF) as usize);
                for _ in 0..n {
                    let m = r.u16()?;
                    let len = r.u16()?;
                    cells.extend(std::iter::repeat_n(m, len as usize));
                }
                if cells.len() != (LEAF * LEAF) as usize {
                    return Err("corrupt save: a leaf has the wrong size".into());
                }
                State::Dense(cells.into_boxed_slice())
            }
            t => return Err(format!("corrupt save: unknown node state {t}")),
        };
        nodes.insert(k, Entry { state, totals });
    }
    let tree = crate::totals::Tree::for_planet(&planet);
    Ok(World { planet, tree, nodes, ledger, track: None, life_watch: Vec::new(), life_notes: Default::default(), looks: Default::default() })
}
