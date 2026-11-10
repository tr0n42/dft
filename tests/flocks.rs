//! Cutters and haulers: the flock model.
use dft_sim::economy::Economy;
use dft_sim::swarm::{HaulerState, Sim};
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn eco() -> Economy {
    Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap()
}

fn report(s: &Sim, label: &str) {
    let states = |st: HaulerState| s.haulers.iter().filter(|h| h.state == st).count();
    println!(
        "[{label}] {:.0} min: mother {:.0}% of the way round at {:.0} m ({:.0} t returned), {} flocks ({} splits, {} merges, {} sent to help, {} haulers retired), {} explosions; {} cutters, {} haulers (working {}, lifting {}, unloading {}, returning {}), lost {} haulers, {} starved, {} burned, {} crushed, {} died in all; {:.2}% eaten, {:.0} t delivered, {:.0} t burned as fuel",
        s.time() / 60.0,
        s.mother.half / std::f64::consts::PI * 100.0,
        s.mother.radius,
        s.mother.returned_kg / 1000.0,
        s.flock_count(),
        s.splits,
        s.merges,
        s.helpers_sent,
        s.haulers_retired,
        s.explosions,
        s.drone_count(),
        s.haulers.len(),
        states(HaulerState::Working),
        states(HaulerState::Lifting),
        states(HaulerState::Unloading),
        states(HaulerState::Returning),
        s.haulers_lost,
        s.starved,
        s.burned,
        s.crushed,
        s.deaths,
        s.consumed_share() * 100.0,
        s.delivered_credited / 1000.0,
        s.fuel_kg / 1000.0
    );
}

#[test]
fn haulers_fill_lift_and_come_back() {
    let planet = dft_sim::load_body(&data(), "rubble-asteroid").unwrap();
    let mut s = Sim::new(World::new(planet), eco());
    s.free_drones = Some(3000);
    for m in [2.0, 5.0, 10.0, 20.0] {
        s.run(m * 60.0);
        report(&s, "free 3000");
    }
    assert!(!s.haulers.is_empty());
    assert!(s.delivered_credited > 0.0, "nothing reached the mother swarm");
    assert!(s.fuel_kg > 0.0, "cutters burn cargo for energy");
    assert!(s.drone_count() > 2000);
    // Mass is exactly what it was paid, plus remains, minus spending.
    let expect = s.delivered_credited + s.recovered_mass - s.spent_mass;
    assert!((s.mass - expect).abs() < 1e-6 * s.delivered_credited.max(1.0));
}

#[test]
fn the_economy_runs_on_haulers() {
    let planet = dft_sim::load_body(&data(), "tutorial-1-2km").unwrap();
    let mut s = Sim::new(World::new(planet), eco());
    for m in [5.0, 10.0, 20.0, 40.0] {
        s.run(m * 60.0);
        report(&s, "economy");
    }
    assert!(s.delivered_credited > 0.0);
    // Mass is exactly what it was paid, plus remains, minus spending.
    let expect = s.delivered_credited + s.recovered_mass - s.spent_mass;
    assert!((s.mass - expect).abs() < 1e-6 * s.delivered_credited.max(1.0));
}

/// The swarm director keeps haulers coming to each flock so cutters with full
/// holds don't pile up waiting, and says why it built each.
#[test]
fn the_director_keeps_cutters_from_waiting() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let w = dft_sim::world::World::new(dft_sim::load_body(&data, "rubble-asteroid").unwrap());
    let eco = dft_sim::economy::Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut s = dft_sim::swarm::Sim::new(w, eco);
    s.free_drones = Some(20000);
    let (mut sum, mut n) = (0.0, 0);
    for k in 1..=60 {
        s.run(k as f64 * 10.0);
        if k > 12 {
            sum += s.waiting_count() as f64;
            n += 1;
        }
    }
    let mean = sum / n as f64;
    println!("  waiting on average {mean:.0} of {} cutters; {} haulers built; last calls:", s.drone_count(), s.haulers_built);
    for l in &s.director_log {
        println!("    {l}");
    }
    assert!(s.haulers_built > 5, "the director should have built haulers");
    // (With the pool nearly every drone works, so hauling, not rock, sets
    // the pace: a share of cutters always stands waiting while a full hauler
    // is away and the next is on its way. Before the pool most drones sat
    // idle instead and almost none waited.)
    assert!(mean < 0.2 * s.drone_count() as f64, "too many cutters left waiting: {mean:.0}");
    assert!(!s.director_log.is_empty());
}
