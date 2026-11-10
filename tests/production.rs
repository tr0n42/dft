//! Production: the fleet arrives with no haulers, builds them from its
//! allowance of refined elements, then builds cutters as there is work.

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

fn report(s: &Sim, label: &str) {
    println!("  [{label}] t {:.0} s: energy {:.0}, {} cutters, {} haulers, stockpile {:.1} t ({:.1} t refined)", s.time(), s.energy, s.harvesters_alive(), s.hauler_views().len(), s.stock.total() / 1000.0, s.stock.refined.iter().sum::<f64>() / 1000.0);
    for l in s.production_lines() {
        println!("      {l}");
    }
}

#[test]
fn the_fleet_builds_its_first_haulers_then_cutters() {
    let mut s = Sim::new(World::new(dft_sim::load_body(&data(), "tutorial-1-2km").unwrap()), eco());
    assert!(s.factory(), "the economy has a factory");
    // It arrives with its cutters and no haulers: they are on order.
    assert_eq!(s.hauler_views().len(), 0);
    assert!(s.harvesters_alive() >= 1000);
    let ordered = s.prod.orders.len();
    assert!(ordered > 0, "the first flocks' haulers are on order");
    report(&s, "arrival");
    let mut first = None;
    for t in (30..=600).step_by(30) {
        s.run(t as f64);
        if first.is_none() && !s.hauler_views().is_empty() {
            first = Some(t);
        }
        if t <= 300 || t % 120 == 0 {
            report(&s, "play");
        }
    }
    let first = first.expect("a hauler was built");
    println!("  first hauler out by {first} s; {ordered} on order at the start");
    assert!(first <= 120, "the first hauler takes no more than two minutes");
    assert!(s.prod.built[1] as usize >= ordered.min(4), "the first haulers were built");
    // And with haulers working, the stockpile fills and cutters are built.
    assert!(s.prod.built[0] > 0, "cutters were built");
    // Everything built came out of the stockpile: nothing is free.
    assert!(s.stock_spent > 0.0);
}

#[test]
fn production_survives_a_save() {
    let planet = std::sync::Arc::new(dft_sim::load_body(&data(), "tutorial-1-2km").unwrap());
    let mut s = Sim::new(World::shared(planet.clone()), eco());
    s.run(90.0);
    let mut w = dft_sim::save::W::default();
    s.write_save(&mut w);
    let l = Sim::read_save(&mut dft_sim::save::R(&w.0, 0), planet, eco()).unwrap();
    assert_eq!(l.prod.orders, s.prod.orders);
    assert_eq!(l.prod.built, s.prod.built);
    assert_eq!(l.parts.n, s.parts.n);
    assert_eq!(l.prod.bays.iter().filter(|b| b.job.is_some()).count(), s.prod.bays.iter().filter(|b| b.job.is_some()).count());
}
