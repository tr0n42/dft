//! The Universal Drone Replicator: the specialists are built only where
//! their skills have work, and do it (sappers blow cracked pockets in the
//! rock, tunnelers bore shafts, heat-sink bots freeze magma, foresters take
//! plants), and they survive a save.

use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn eco() -> Economy {
    Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap()
}

/// A body from data/bodies, resized (km), with life on as in the game.
fn body(name: &str, km: f64) -> dft_sim::planet::Planet {
    let text = std::fs::read_to_string(data().join("bodies").join(format!("{name}.toml"))).unwrap();
    let text = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
    let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
    spec.life = Some(1.0);
    let mats = dft_sim::materials::MaterialTable::load(&data().join("materials.toml").to_string_lossy()).unwrap();
    dft_sim::planet::Planet::new(spec, mats).unwrap()
}

fn report(s: &Sim) {
    let (blasts, cracked, frozen, plants, fires) = s.specialist_totals();
    println!(
        "  t {:>5.0} s: {} cutters · out: {} sappers, {} tunnelers, {} heat-sink bots, {} foresters · blasts {blasts} ({cracked} cracked), shafts {} m, frozen {:.0} t, plants {:.1} t, fires {fires}",
        s.time(),
        s.harvesters_alive(),
        s.specialists_out(Unit::Sapper),
        s.specialists_out(Unit::Tunneler),
        s.specialists_out(Unit::HeatSink),
        s.specialists_out(Unit::Forester),
        s.tunnel_m,
        frozen / 1000.0,
        plants / 1000.0
    );
}

fn run(s: &mut Sim, until: f64) {
    for t in (60..=until as u64).step_by(60) {
        s.run(t as f64);
        if t % 300 == 0 {
            report(s);
        }
    }
    for l in s.production_lines() {
        println!("      {l}");
    }
}

#[test]
fn sappers_and_tunnelers_work_anywhere() {
    let mut s = Sim::new(World::new(dft_sim::load_body(&data(), "tutorial-1-2km").unwrap()), eco());
    // A lifeless rubble asteroid: no magma, no plants, so only those two.
    assert!(s.prod.bays_of(Unit::Sapper) > 0 && s.prod.bays_of(Unit::Tunneler) > 0);
    assert_eq!(s.prod.bays_of(Unit::HeatSink), 0);
    assert_eq!(s.prod.bays_of(Unit::Forester), 0);
    assert_eq!(s.prod.bays_of(Unit::Abductor), 0, "no body has animals");
    run(&mut s, 1200.0);
    let (blasts, cracked, ..) = s.specialist_totals();
    assert!(s.prod.built[Unit::Sapper.index()] > 0, "sappers were built");
    assert!(blasts > 0 && cracked > 0, "sappers went off and cracked the rock");
    assert!(s.prod.built[Unit::Tunneler.index()] > 0, "a tunneler was built");
    assert!(s.tunnel_m > 0, "and bored");
}

#[test]
fn heat_sink_bots_freeze_magma_on_a_lava_world() {
    let mut s = Sim::new(World::new(body("lava-world", 3.0)), eco());
    assert!(s.prod.bays_of(Unit::HeatSink) > 0);
    run(&mut s, 1200.0);
    let (_, _, frozen, ..) = s.specialist_totals();
    assert!(s.prod.built[Unit::HeatSink.index()] > 0, "heat-sink bots were built");
    assert!(frozen > 0.0, "and froze magma");
}

#[test]
fn foresters_take_plants_on_a_living_world() {
    let mut s = Sim::new(World::new(body("living-world", 3.0)), eco());
    assert!(s.prod.bays_of(Unit::Forester) > 0);
    run(&mut s, 1200.0);
    let (.., plants, _) = s.specialist_totals();
    assert!(s.prod.built[Unit::Forester.index()] > 0, "foresters were built");
    assert!(plants > 0.0, "and took plants");
}

#[test]
fn specialists_survive_a_save() {
    let planet = std::sync::Arc::new(dft_sim::load_body(&data(), "tutorial-1-2km").unwrap());
    let mut s = Sim::new(World::shared(planet.clone()), eco());
    s.run(600.0);
    let mut w = dft_sim::save::W::default();
    s.write_save(&mut w);
    let l = Sim::read_save(&mut dft_sim::save::R(&w.0, 0), planet, eco()).unwrap();
    for u in Unit::SPECIALISTS {
        assert_eq!(l.specialists_out(u), s.specialists_out(u), "{} out", u.name());
    }
    assert_eq!(l.specialist_totals().0, s.specialist_totals().0);
    assert_eq!(l.prod.built, s.prod.built);
}
