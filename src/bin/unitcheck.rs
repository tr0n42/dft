//! Which specialists the Universal Drone Replicator builds on each body:
//! every body in data/bodies, at its own size and shrunk to the test size
//! (5 km), with what decides it (magma, plants, animals) and the uranium the
//! body holds for sappers' charges.
//!
//! cargo run --release --bin unitcheck [-- <body> ...]
use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::Skills;
use std::path::Path;

fn main() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let min_k = eco.units.heatsink.min_freeze_k;
    let mut names: Vec<String> = std::env::args().skip(1).collect();
    if names.is_empty() {
        let mut v: Vec<String> = std::fs::read_dir(data.join("bodies")).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().trim_end_matches(".toml").to_string()).collect();
        v.sort();
        // (The Earth-size scale test takes minutes to build; it is checked
        // only at the test size.)
        names = v;
    }
    println!("{:<20} {:>9}  {:<6} {:<6} {:<6} {:>12}  builds", "body", "radius", "magma", "plants", "animal", "uranium");
    for name in names {
        for small in [false, true] {
            if !small && name == "earth-test" {
                continue;
            }
            let text = std::fs::read_to_string(data.join("bodies").join(format!("{name}.toml"))).unwrap();
            let text = if small { dft_sim::spec::resize_text(&text, 5000.0).unwrap() } else { text };
            let mut spec = match dft_sim::spec::BodySpec::parse(&text) {
                Ok(s) => s,
                Err(e) => {
                    println!("{name:<20} (can't read: {e})");
                    continue;
                }
            };
            // Life is on wherever a body's chemistry allows it, as in the game.
            if spec.life.is_none() {
                spec.life = Some(1.0);
            }
            let mats = match dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()) {
                Ok(m) => m,
                Err(e) => {
                    println!("{name:<20} (no materials: {e})");
                    continue;
                }
            };
            let planet = match dft_sim::planet::Planet::new(spec, mats) {
                Ok(p) => p,
                Err(e) => {
                    println!("{name:<20} (can't build: {e})");
                    continue;
                }
            };
            let r_km = planet.radius / 1000.0;
            let world = dft_sim::world::World::new(planet);
            let s = Skills::of(&world, min_k);
            let builds: Vec<&str> = Unit::SPECIALISTS.iter().filter(|u| s.useful(**u)).map(|u| u.name()).collect();
            let yn = |b: bool| if b { "yes" } else { "-" };
            println!("{:<20} {:>7.1} km  {:<6} {:<6} {:<6} {:>9.3e} kg  {}", name, r_km, yn(s.magma), yn(s.flora), yn(s.fauna), s.uranium_kg, builds.join(", "));
        }
    }
}
