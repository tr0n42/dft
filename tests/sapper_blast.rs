//! A sapper's buried blast (the crater thrown out, the shock's cracks, the
//! loosened column) makes and loses nothing: what was blown out is all still
//! on the body (on the ground, or in the air on its way).
use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

fn sim() -> Sim {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "barren-world").unwrap();
    let mut spec = base.spec.clone();
    spec.radius_m = 3000.0;
    let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(0);
    s
}

#[test]
fn a_sapper_blast_throws_out_a_crater_and_keeps_the_books() {
    let mut s = sim();
    // Over the ground on the far side from where the swarm lands.
    let over = (0.5, -(s.world.planet.max_radius + 5.0));
    let mut y = over.1 as i64;
    while s.world.material(0, y) == VACUUM {
        y += 1;
    }
    let total = |s: &Sim| -> f64 { (0..s.world.planet.mats.len()).map(|m| s.world.ledger.initial[m]).sum() };
    let before = total(&s);
    s.test_blast(over, 10.0);
    for _ in 0..600 {
        s.step();
    }
    // A crater: the middle well down from where the ground was.
    let mut y2 = over.1 as i64;
    while s.world.material(0, y2) == VACUUM {
        y2 += 1;
    }
    assert!(y2 - y >= 6, "the ground over the blast is {} m lower (at least 6 wanted)", y2 - y);
    assert!((total(&s) - before).abs() < 1e-6);
}

/// How long each part of a blast takes as the yield grows (run by hand:
/// cargo test --release --test sapper_blast -- --ignored --nocapture).
#[test]
#[ignore]
fn blast_timings() {
    for kt in [10.0, 100.0, 1000.0, 3000.0, 10000.0] {
        let mut s = sim();
        let t = std::time::Instant::now();
        s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), kt);
        eprintln!("{kt} kt: {:.0} ms in all", t.elapsed().as_secs_f64() * 1000.0);
    }
}
