//! Saving a game mid-play and loading it back.
use dft_sim::economy::Economy;
use dft_sim::save::{R, W};
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;
use std::sync::Arc;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn eco() -> Economy {
    Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap()
}

fn round_trip(body: &str, minutes: f64, free: Option<u64>) {
    let planet = Arc::new(dft_sim::load_body(&data(), body).unwrap());
    let mut s = Sim::new(World::shared(planet.clone()), eco());
    s.free_drones = free;
    s.run(minutes * 60.0);
    let t = std::time::Instant::now();
    let mut o = W::default();
    s.write_save(&mut o);
    let raw = o.0;
    let packed = dft_sim::save::pack(b"TEST", 1, &raw);
    let save_s = t.elapsed().as_secs_f64();
    let t = std::time::Instant::now();
    let back = dft_sim::save::unpack(b"TEST", 1, &packed).unwrap();
    let mut l = Sim::read_save(&mut R(&back, 0), planet, eco()).unwrap();
    l.free_drones = free;
    let load_s = t.elapsed().as_secs_f64();
    println!(
        "{body} at {minutes} min: {} drones, frontier {}, loose {}, save {:.1} MB raw / {:.2} MB packed in {:.2} s, load {:.2} s",
        s.drone_count(),
        s.frontier_len(),
        s.bodies.count(),
        raw.len() as f64 / 1e6,
        packed.len() as f64 / 1e6,
        save_s,
        load_s
    );
    assert_eq!(l.tick, s.tick);
    assert_eq!(l.drone_count(), s.drone_count());
    assert_eq!(l.hubs.len(), s.hubs.len());
    assert_eq!(l.energy, s.energy);
    assert_eq!(l.mass, s.mass);
    // The dig face comes back whole (cells that moved since are dropped).
    assert!(l.frontier_len() + 50 >= s.frontier_len(), "frontier {} vs {}", l.frontier_len(), s.frontier_len());
    if s.bodies.count() == 0 {
        assert_eq!(l.world.state_hash(), s.world.state_hash());
    }
    // And the loaded game carries on digging.
    let before = l.consumed_share();
    let t0 = l.time();
    l.run(t0 + 120.0);
    assert!(l.consumed_share() > before, "the loaded game should keep digging");
    assert!(l.drone_count() > 0);
}

#[test]
fn save_and_load_small_body() {
    round_trip("tutorial-1-2km", 20.0, None);
}

#[test]
fn save_and_load_free_swarm() {
    round_trip("rubble-asteroid", 5.0, Some(3000));
}
