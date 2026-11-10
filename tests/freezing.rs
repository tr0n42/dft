//! Freezing only as heat flows: cold ice freezes a little water against it;
//! plenty of warmer water melts a little ice instead.
use dft_sim::materials::VACUUM;
use dft_sim::sandbox::Sandbox;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// An Earth-like world: the ground near the surface is about 288 K.
const BODY: &str = "living-world";

/// A sealed box in solid rock deep down: `ice` cells of ice at `ice_k` on the
/// left, `water` cells of water at `water_k` on the right. Run, then count
/// (ice, water).
fn tank(ice: i64, ice_k: f64, water: i64, water_k: f64, secs: f64) -> (usize, usize) {
    let mut w = World::new(dft_sim::load_body(&data(), BODY).unwrap());
    let (ice_id, water_id) = (w.planet.mats.id("ice").unwrap(), w.planet.mats.id("water").unwrap());
    // A little under the surface, where the ground itself is cold.
    let y0 = (w.planet.surface_radius(0.0, 1.0) - 40.0) as i64;
    let h = 6i64;
    let (wi, ww) = ((ice + h - 1) / h, (water + h - 1) / h);
    let mut cells = Vec::new();
    for x in 0..wi + ww {
        for y in 0..h {
            let c = (x, y0 + y);
            w.mine_cell(c.0, c.1);
            cells.push(c);
        }
    }
    let mut sb = Sandbox::new(&w);
    let (mut ni, mut nw) = (0, 0);
    for &(x, y) in &cells {
        if x < wi && ni < ice {
            w.deposit_cell(x, y, ice_id);
            sb.temps.insert((x, y), ice_k);
            ni += 1;
        } else if x >= wi && nw < water {
            w.deposit_cell(x, y, water_id);
            sb.temps.insert((x, y), water_k);
            nw += 1;
        }
        sb.wake((x, y));
    }
    for _ in 0..(secs * w.planet.mats.physics.ticks_per_s) as usize {
        sb.step(&mut w);
    }
    let count = |id| cells.iter().filter(|c| w.material(c.0, c.1) == id).count();
    let _ = VACUUM;
    (count(ice_id), count(water_id))
}

#[test]
fn deep_cold_ice_freezes_some_water() {
    let (ice, water) = tank(24, 30.0, 6, 275.0, 20.0);
    println!("24 ice at 30 K + 6 water at 275 K, 20 s: {ice} ice, {water} water");
    assert!(ice > 24, "very cold ice should freeze some of the water against it");
}

#[test]
fn plenty_of_water_melts_a_little_ice() {
    let (ice, water) = tank(2, 260.0, 30, 300.0, 600.0);
    println!("2 ice at 260 K + 30 water at 300 K: {ice} ice, {water} water");
    assert!(ice < 2, "enough warmer water should melt the ice");
}
