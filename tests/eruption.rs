//! Pressure: a hole bored into a buried layer of liquid that the weight of
//! the rock above would lift past the rim blasts it out; the blast draws
//! the liquid from the layer, weakens as it drains and stops; nothing is made.
use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

#[test]
fn a_tapped_magma_layer_blasts_out_then_goes_quiet() {
    let base = dft_sim::load_body(&data(), "lava-world").unwrap();
    let mut spec = base.spec.clone();
    spec.radius_m = 5000.0;
    let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
    let eco = Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap();
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(0);
    let magma = s.world.planet.mats.id("magma").unwrap();
    // Bore a 3 m shaft straight down at the far side from where the swarm lands.
    let top = s.world.planet.surface_radius(0.0, -1.0) as i64;
    let mut y = -(top + 2);
    let mut dug = 0;
    while s.world.material(0, y) != magma && dug < 2000 {
        for x in -1..=1 {
            if s.world.material(x, y) != VACUUM && s.world.material(x, y) != magma {
                s.world.mine_cell(x, y);
            }
            s.sandbox.wake_around((x, y));
        }
        y += 1;
        dug += 1;
    }
    assert_eq!(s.world.material(0, y), magma, "the shaft should reach the magma");
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let mut last = 0;
    // By the minute-9 mark the vent has nearly stopped: from then on only a
    // trickle (under 1% of all it blasted out) may follow. (A one-minute
    // pause used to count as quiet, but the vent can pause while its first
    // burst is still in the air and then add a tenth or so more as the shaft
    // clears, which that rule took for flaring up again; 27 Nov.)
    let mut at_9 = 0;
    for minute in 1..=12 {
        s.run(minute as f64 * 60.0);
        let e = s.sandbox.stats.erupted;
        println!("  minute {minute}: {e} cells blasted out, {} drops in the air", s.bodies.drops());
        if minute == 9 {
            at_9 = e;
        }
        last = e;
        assert!(s.world.ledger.imbalance(&s.world.remaining_kg()) < 1e-6 * initial, "the books stopped balancing");
    }
    assert!(last > 100, "the vent should have blasted out plenty, got {last}");
    assert!(last - at_9 < last / 100, "the vent should have gone quiet ({at_9} at minute 9, {last} at 12)");
    // Magma came down on the surface, outside the shaft.
    let mut outside = 0;
    for x in -400i64..=400 {
        if x.abs() <= 3 {
            continue;
        }
        for d in -30i64..=60 {
            let yy = -(top + d);
            if s.world.material(x, yy) == magma {
                outside += 1;
            }
        }
    }
    println!("  magma on the ground round the vent: {outside} cells");
    assert!(outside > 20, "what was blasted out should land round the vent");
}
