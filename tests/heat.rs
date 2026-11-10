//! Drones' heat: drinking lava, working hot and cold ground, eating seas.
use dft_sim::economy::Economy;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn eco() -> Economy {
    Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap()
}

fn run(body: &str, minutes: f64) -> Sim {
    let mut s = Sim::new(World::new(dft_sim::load_body(&data(), body).unwrap()), eco());
    s.free_drones = Some(3000);
    s.run(minutes * 60.0);
    println!(
        "{body} {minutes} min: {} cutters, {} lost to heat, {} sips of molten rock, {} starved, {} died in all, {:.2}% eaten",
        s.drone_count(),
        s.burned,
        s.lava_swallowed,
        s.starved,
        s.deaths,
        s.consumed_share() * 100.0
    );
    s
}

#[test]
fn lava_is_drunk_and_melts_the_reckless() {
    let s = run("lava-world", 15.0);
    assert!(s.lava_swallowed > 0, "cutters should drink lava");
    assert!(s.burned > 0, "some cutters should melt");
    assert!(s.drone_count() > 1000);
}

#[test]
fn seas_are_eaten() {
    let s = run("ocean-world", 15.0);
    assert!(s.consumed_share() > 0.0);
    let mats = &s.world.planet.mats;
    let liquid: f64 = s.world.ledger.mined.iter().enumerate().filter(|(i, _)| mats.get(*i as u16).state == dft_sim::materials::State::Liquid).map(|(_, k)| *k).sum();
    println!("  liquid eaten: {:.0} t", liquid / 1000.0);
    assert!(liquid > 0.0, "the sea should be eaten");
}

#[test]
fn cold_worlds_still_work() {
    let s = run("methane-world", 15.0);
    assert!(s.consumed_share() > 0.0);
}

/// Heat is worked out on many threads at once: the run must come out the
/// same on one thread as on several (temperatures, what changed, who died),
/// and clearing out the dead drones on the way must not change it either.
#[test]
fn lava_world_same_on_any_thread_count() {
    let run = |threads: usize| {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
            let base = dft_sim::load_body(&data(), "lava-world").unwrap();
            let mut spec = base.spec.clone();
            spec.radius_m = 5000.0;
            let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
            let mut s = Sim::new(World::new(p), eco());
            s.free_drones = Some(10000);
            s.run(8.0 * 60.0);
            let mut temps: Vec<((i64, i64), u64)> = s.sandbox.temps.iter().map(|(c, t)| (*c, t.to_bits())).collect();
            temps.sort();
            let h = temps.iter().fold(0u64, |a, (c, t)| a.wrapping_mul(31).wrapping_add(c.0 as u64 ^ (c.1 as u64).rotate_left(17) ^ t));
            println!(
                "{threads} threads: {} temps (hash {h:x}), {} stepped, {} deaths, {} compactions, {} drones listed, {:.4}% eaten",
                temps.len(),
                s.sandbox.stats.cells_stepped,
                s.deaths,
                s.compactions,
                s.drone_count(),
                s.consumed_share() * 100.0
            );
            (h, temps.len(), s.sandbox.stats.cells_stepped, s.deaths, s.world.ledger.mined_kg().to_bits(), s.burned)
        })
    };
    let one = run(1);
    let many = run(3);
    assert_eq!(one, many, "the run changed with the number of threads");
}

/// Clearing the dead out of the drone list renumbers the living. The
/// numbers seed small timing offsets, so the run differs from one that never
/// clears them, but it must still come out the same every time.
#[test]
fn clearing_the_dead_keeps_runs_the_same() {
    let run = || {
        let base = dft_sim::load_body(&data(), "lava-world").unwrap();
        let mut spec = base.spec.clone();
        spec.radius_m = 5000.0;
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let mut s = Sim::new(World::new(p), eco());
        s.compact_min = 1000;
        s.free_drones = Some(2000);
        s.run(25.0 * 60.0);
        println!("{} compactions, {} deaths, {} alive, {:.5}% eaten", s.compactions, s.deaths, s.drone_count(), s.consumed_share() * 100.0);
        assert!(s.drone_count() > 1900, "the swarm should keep its numbers");
        (s.compactions, s.deaths, s.drone_count(), s.world.ledger.mined_kg().to_bits(), s.burned, s.sandbox.stats.cells_stepped)
    };
    let a = run();
    assert!(a.0 > 0, "the dead should have been cleared at least once");
    assert_eq!(a, run());
}
