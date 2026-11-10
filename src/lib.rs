//! The Dark Forest Theory: simulation core.
//!
//! M1: the planet generator (data-driven materials and body specs, a deterministic
//! `material(x, y)` for any 1 m cell, quadtree totals, previews).
//! M2: the world store (sparse changes over the generator), mining, the ledger,
//! compressed saves and determinism hashes.
//! M3: the swarm and the asteroid loop (drones, Mass and Energy, upgrades, launch).

pub mod bodies;
pub mod deep;
pub mod economy;
pub mod elements;
pub mod flora;
pub mod flow;
pub mod recipes;
pub mod geology;
pub mod glamour;
pub mod ledger;
pub mod materials;
pub mod metal;
pub mod noise;
pub mod palette;
pub mod planet;
pub mod production;
pub mod reference;
pub mod relief;
pub mod render;
pub mod sandbox;
pub mod save;
pub mod spec;
pub mod specialty;
pub mod swarm;
pub mod texture;
pub mod totals;
pub mod world;

use std::path::Path;

/// Set how many threads the simulation's parallel work uses: two thirds of
/// the machine's (at least 2), leaving the rest for drawing and the system.
/// Call once, before anything runs in parallel. Results are the same on any
/// number of threads. Returns the number chosen.
pub fn use_sim_threads() -> usize {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(6);
    let k = (n * 2 / 3).max(2);
    let _ = rayon::ThreadPoolBuilder::new().num_threads(k).thread_name(|i| format!("sim-{i}")).build_global();
    rayon::current_num_threads()
}

/// Load the material table and one body spec from a data folder.
pub fn load_body(data_dir: &Path, body: &str) -> Result<planet::Planet, String> {
    let mats = materials::MaterialTable::load(&data_dir.join("materials.toml").to_string_lossy())?;
    let spec = spec::BodySpec::load(&data_dir.join("bodies").join(format!("{body}.toml")).to_string_lossy())?;
    planet::Planet::new(spec, mats)
}
