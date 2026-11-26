//! The ground units as agreed with Benjamin on 25 Nov 2026
//! (docs/design/ground-units.md): sappers made to order where cutters
//! struggle, depth from yield, blasts that kill; tunnelers (openers) that
//! reach and cap magma; heat sinks that freeze a passage through it; and
//! tunnelers going on down from the passage.
//!
//! Free cutters (as the benchmarks run them) keep these quick.

use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// A test body with free cutters and only these specialists.
fn lab(body: &str, units: &[Unit], drones: u64) -> Sim {
    let mut e = Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap();
    e.units.bay.retain(|b| matches!(b.unit, Unit::Cutter | Unit::Hauler) || units.contains(&b.unit));
    e.seed_swarm.start_harvesters = drones as u32;
    let team: Vec<(Unit, u32)> = units
        .iter()
        .map(|&u| {
            (
                u,
                match u {
                    Unit::Sapper => e.units.sapper.keep,
                    Unit::Tunneler => e.units.tunneler.keep,
                    Unit::HeatSink => e.units.heatsink.keep,
                    _ => 2,
                },
            )
        })
        .collect();
    // (Life on, as the game has it wherever a body's chemistry allows.)
    let text = std::fs::read_to_string(data().join("bodies").join(format!("{body}.toml"))).unwrap();
    let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
    if spec.life.is_none() {
        spec.life = Some(1.0);
    }
    let mats = dft_sim::materials::MaterialTable::load(&data().join("materials.toml").to_string_lossy()).unwrap();
    let planet = dft_sim::planet::Planet::new(spec, mats).unwrap();
    let mut s = Sim::new(World::new(planet), e);
    s.free_drones = Some(drones);
    s.free_units = team;
    s
}

/// Benjamin: sappers are made to order, only where the cutters struggle
/// with hard ground; none on soft ground.
#[test]
fn sappers_wait_for_hard_ground() {
    let mut s = lab("tutorial-1-2km", &[Unit::Sapper], 5_000);
    s.run(600.0);
    let r = s.ground_report();
    println!("soft ground, 10 min: {r:?}");
    assert_eq!(r.blasts, 0, "no sapper where nothing is hard");
    assert_eq!(s.specialists_out(Unit::Sapper), 0);
}

/// On hard rock: sappers come where flocks struggle, no more often than
/// one every order_gap_s, go off at their yield's depth, loosen the ground
/// over them, and kill the cutters caught in it.
#[test]
fn sappers_go_off_deep_where_cutters_struggle() {
    let mut s = lab("test-sappers", &[Unit::Sapper], 20_000);
    s.run(900.0);
    let r = s.ground_report();
    println!("hard ground, 15 min: {r:?}");
    let gap = s.eco.units.sapper.order_gap_s;
    assert!(r.blasts >= 3, "sappers went off ({})", r.blasts);
    assert!(r.blasts as f64 <= 900.0 / gap + 1.0, "at most one every {gap} s ({})", r.blasts);
    assert!(r.blast_kills > 0, "blasts kill cutters");
    let (_, cracked, ..) = s.specialist_totals();
    // A 5 kt column loosens thousands of cells; the old blast about a thousand.
    assert!(cracked as f64 >= r.blasts as f64 * 2500.0, "{cracked} cells broken in {} blasts", r.blasts);
}

/// Tunnelers reach the magma layer and cap it; heat sinks come down the
/// shaft and freeze a passage through it; a tunneler goes on down past the
/// layer. Nothing is wrecked by magma, and it all survives a save.
#[test]
fn tunnelers_and_heat_sinks_open_the_magma_layer() {
    let mut s = lab("test-tunnelers", &[Unit::Tunneler, Unit::HeatSink], 20_000);
    let layer_top = s.world.planet.radius * 0.83;
    let mut below = false;
    for t in (120..=2400).step_by(120) {
        s.run(t as f64);
        let r = s.ground_report();
        println!("t {t}: {r:?}");
        // A tunneler under the magma layer (its head deeper than the layer's
        // bottom, about 90 m under its top).
        if s.unit_views().iter().any(|v| v.kind == Unit::Tunneler && (v.to.0.hypot(v.to.1)) < layer_top - 100.0) {
            below = true;
        }
        if t == 1200 {
            // A save in the middle of it all.
            let mut w = dft_sim::save::W::default();
            s.write_save(&mut w);
            let l = Sim::read_save(&mut dft_sim::save::R(&w.0, 0), s.world.planet.clone(), s.eco.clone()).unwrap();
            assert_eq!(l.ground_report(), s.ground_report(), "the ground units survive a save");
            // (The world as the swarm left it; a loaded game's own fingerprint
            // differs from the running one's, as it did before these units.)
            assert_eq!(l.world.state_hash(), s.world.state_hash());
        }
    }
    let r = s.ground_report();
    assert!(r.caps >= 1, "magma capped");
    assert!(r.passages >= 1, "a passage frozen through");
    assert!(below, "a tunneler went on down past the magma layer");
    assert_eq!(r.bots_lost, 0, "no heat sink wrecked");
}


/// Foresters and abductors fly past the swarm's edge and drop to work;
/// abductors catch some animals alive and laser the spitters; out of sight
/// a forester's fires burn on as math; it all survives a save.
#[test]
fn walkers_fly_out_and_work() {
    let mut s = lab("dev-planet", &[Unit::Forester, Unit::Abductor], 20_000);
    s.set_attention(Some(None));
    s.set_focus(Some(None));
    s.run(600.0);
    let (caught, _, _) = s.fauna_totals();
    let (.., plants, fires) = s.specialist_totals();
    let lines = s.specialist_lines().join("\n");
    println!("{lines}");
    assert!(plants > 0.0 && fires > 0, "foresters took plants and lit fires");
    assert!(caught > 0, "abductors caught animals alive");
    assert!(lines.contains("burst by lasers") && !lines.contains(" 0 burst by lasers"), "abductors burst some");
    assert!(lines.contains("plant cells burned as math") && !lines.contains("(0 plant cells burned as math)"), "fires burned out of sight");
    let mut w = dft_sim::save::W::default();
    s.write_save(&mut w);
    let l = Sim::read_save(&mut dft_sim::save::R(&w.0, 0), s.world.planet.clone(), s.eco.clone()).unwrap();
    // (The walkers and the animals; the stands of plants found are scanned
    // afresh every 30 s, and leaps were never saved.)
    let keep = |v: Vec<String>| v.into_iter().filter(|x| x.contains("walkers:") || x.contains("animals:")).collect::<Vec<_>>();
    assert_eq!(keep(l.specialist_lines()), keep(s.specialist_lines()), "the walkers survive a save");
}

/// A tunneler with nothing left to open backs up its shaft, finds nothing
/// anywhere, sinks back down and goes off where it is (no other tunneler
/// caught in it).
#[test]
fn a_tunneler_with_nothing_left_goes_off() {
    let mut s = lab("test-tunnelers", &[Unit::Tunneler], 5_000);
    s.free_units = vec![(Unit::Tunneler, 1)];
    s.run(240.0);
    assert_eq!(s.specialists_out(Unit::Tunneler), 1, "a tunneler is out");
    // Nothing is worth opening any more.
    s.eco.units.tunneler.min_score = f64::INFINITY;
    s.eco.units.tunneler.magma_score = 0.0;
    s.eco.units.tunneler.hard_score = 0.0;
    s.free_units.clear();
    for t in (300..=1200).step_by(60) {
        s.run(t as f64);
        if s.ground_report().tunneler_blasts > 0 {
            break;
        }
    }
    let r = s.ground_report();
    println!("{r:?}");
    assert_eq!(r.tunneler_blasts, 1, "it went off");
    assert_eq!(s.specialists_out(Unit::Tunneler), 0);
    assert!(r.blasts >= 1);
}
