//! A big blast on the ground blows out a crater and throws the ground about,
//! and nothing is made or lost: what was blown out is all still on the body
//! (back on the ground, or in the air on its way).
use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

#[test]
fn a_big_blast_digs_a_crater_and_keeps_the_books() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "barren-world").unwrap();
    let mut spec = base.spec.clone();
    spec.radius_m = 3000.0;
    let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(0);
    // On the ground, on the far side from where the swarm lands.
    let mut y = -(s.world.planet.max_radius as i64 + 2);
    while s.world.material(0, y) == VACUUM {
        y += 1;
    }
    let at = (0.5, y as f64 + 2.0);
    let solid_before = (-15..=15).flat_map(|dx: i64| (-15..=15).map(move |dy: i64| (dx, dy))).filter(|&(dx, dy)| s.world.material(dx, at.1 as i64 + dy) != VACUUM).count();
    let total = |s: &Sim| -> f64 { (0..s.world.planet.mats.len()).map(|m| s.world.ledger.initial[m]).sum() };
    let before = total(&s);
    s.big_boom(at, 40.0, 20);
    let solid_after = (-15..=15).flat_map(|dx: i64| (-15..=15).map(move |dy: i64| (dx, dy))).filter(|&(dx, dy)| s.world.material(dx, at.1 as i64 + dy) != VACUUM).count();
    assert!(solid_after + 150 < solid_before, "a crater: {solid_before} solid cells before, {solid_after} after");
    for _ in 0..600 {
        s.step();
    }
    assert!((total(&s) - before).abs() < 1e-6);
}
