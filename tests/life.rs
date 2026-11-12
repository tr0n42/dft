//! The plants' life cycle (L1e): a patch of forest cut down grows back,
//! felled tissue dies and rots to humus, the books still balance, and it all
//! survives a save.

use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn eco() -> Economy {
    Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap()
}

/// A living world, `km` across, with life on as in the game.
fn living(km: f64) -> dft_sim::planet::Planet {
    let text = std::fs::read_to_string(data().join("bodies").join("living-world.toml")).unwrap();
    let text = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
    let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
    spec.life = Some(1.0);
    let mats = dft_sim::materials::MaterialTable::load(&data().join("materials.toml").to_string_lossy()).unwrap();
    dft_sim::planet::Planet::new(spec, mats).unwrap()
}

/// Living plant cells in a strip of the surface around angle `a` (radians).
fn plant_cells(w: &World, a: f64, half_m: f64) -> Vec<(i64, i64)> {
    let (ux, uy) = (a.cos(), a.sin());
    let s = w.planet.surface_radius(ux, uy);
    let mut out = Vec::new();
    let (tx, ty) = (-uy, ux);
    for along in (-half_m as i64)..=(half_m as i64) {
        for up in -6..60 {
            let r = s + up as f64;
            let x = (ux * r + tx * along as f64).floor() as i64;
            let y = (uy * r + ty * along as f64).floor() as i64;
            let m = w.material(x, y);
            if m != VACUUM && w.planet.mats.get(m).name.starts_with("flora ") && !out.contains(&(x, y)) {
                out.push((x, y));
            }
        }
    }
    out
}

fn planted_near(s: &Sim, a: f64) -> usize {
    plant_cells(&s.world, a, 40.0).len()
}

#[test]
fn a_cut_forest_grows_back_and_the_felled_wood_rots() {
    let planet = std::sync::Arc::new(living(3.0));
    let mut s = Sim::new(World::shared(planet.clone()), eco());
    assert!(s.life.is_some(), "a living world has a life cycle");
    // Find a well-grown stretch away from where the swarm lands.
    let (mx, my) = s.mother_pos();
    let home = my.atan2(mx);
    let away = |a: f64| ((a - home).sin().abs() + (1.0 - (a - home).cos())) > 1.5;
    let a = (0..64).map(|i| i as f64 * std::f64::consts::TAU / 64.0).filter(|&a| away(a)).max_by_key(|&a| plant_cells(&s.world, a, 40.0).len()).unwrap();
    let dug = |s: &Sim| {
        let (ux, uy) = (a.cos(), a.sin());
        let g = s.world.planet.surface_radius(ux, uy);
        (-40..=40).filter(|k| { let (tx, ty) = (-uy, ux); let r = g - 4.0; s.world.material((ux * r + tx * *k as f64).floor() as i64, (uy * r + ty * *k as f64).floor() as i64) == VACUUM }).count()
    };
    let before = planted_near(&s, a);
    println!("plant cells in the strip: {before}");
    assert!(before > 50, "the strip has plants");
    // Cut it all down. A few cells are lifted and set down again as if a
    // trunk fell, so they die and rot.
    let cells = plant_cells(&s.world, a, 40.0);
    for (i, &(x, y)) in cells.iter().enumerate() {
        if i % 10 == 0 {
            if let Some(m) = s.world.lift_cell(x, y) {
                s.world.land_cell(x, y, m);
                // (Landing it again in the same place: the world notes it
                // as cut off, which is all that counts here.)
            }
        } else {
            s.world.mine_cell(x, y);
        }
    }
    let after_cut = planted_near(&s, a);
    println!("after the cut: {after_cut}");
    let mut last = 0.0;
    for t in (300..=2700).step_by(300) {
        s.run(t as f64);
        last = t as f64;
        let l = s.life.as_ref().unwrap();
        println!("  t {:>5.0} s: {} plant cells in the strip ({} dug 4 m down) · {:?}", s.time(), planted_near(&s, a), dug(&s), l.stats);
    }
    let _ = last;
    let st = s.life.as_ref().unwrap().stats.clone();
    assert!(st.regrown > 0, "places grew back");
    assert!(st.sprouted > 0, "plants sprouted");
    assert!(st.cells_grown > 0, "and grew");
    assert!(st.cells_died > 0, "felled tissue died");
    assert!(st.cells_rotted > 0, "and rotted");
    assert!(planted_near(&s, a) > after_cut, "the strip is greener than just after the cut");
    let grown: f64 = s.world.ledger.grown.iter().sum();
    println!("grown, net: {grown:.0} kg");
    let imbalance = s.world.ledger.imbalance(&s.world.remaining_kg());
    let total: f64 = s.world.ledger.initial.iter().sum();
    assert!(imbalance < total * 1e-6, "the books balance: off by {imbalance} kg");

    // A save keeps the life cycle.
    let mut w = dft_sim::save::W::default();
    s.write_save(&mut w);
    let l = Sim::read_save(&mut dft_sim::save::R(&w.0, 0), planet, eco()).unwrap();
    assert_eq!(l.life.as_ref().unwrap().counts(), s.life.as_ref().unwrap().counts());
    assert_eq!(l.life.as_ref().unwrap().stats.cells_grown, st.cells_grown);
    assert_eq!(l.world.ledger.grown, s.world.ledger.grown);
}

#[test]
fn lifeless_bodies_have_no_life_cycle() {
    let s = Sim::new(World::new(dft_sim::load_body(&data(), "tutorial-1-2km").unwrap()), eco());
    assert!(s.life.is_none());
}

/// The life cycle on its own (no swarm): a strip cut bare greens again,
/// creeping in from the living forest at its edges.
#[test]
fn a_bare_strip_grows_back_on_its_own() {
    let planet = std::sync::Arc::new(living(3.0));
    let mut w = World::shared(planet.clone());
    let mut life = dft_sim::life::Life::new(&w).unwrap();
    w.life_watch = life.watch();
    let mut sb = dft_sim::sandbox::Sandbox::new(&w);
    let a = (0..64).map(|i| i as f64 * std::f64::consts::TAU / 64.0).max_by_key(|&a| plant_cells(&w, a, 40.0).len()).unwrap();
    let cells = plant_cells(&w, a, 40.0);
    println!("before {}", cells.len());
    for &(x, y) in &cells {
        w.mine_cell(x, y);
    }
    println!("after cut {}", plant_cells(&w, a, 40.0).len());
    for sec in 1..=1800u64 {
        let tick = sec * 10;
        for _ in 0..10 { sb.step(&mut w); }
        life.work(&mut w, &mut sb, tick, 0.1, &|_, _| false);
        if sec % 150 == 0 {
            println!("t {sec}: {} · {:?} · {:?}", plant_cells(&w, a, 40.0).len(), life.counts(), life.stats);
        }
    }
    let now = plant_cells(&w, a, 40.0).len();
    assert!(now > 60, "life creeps back into the strip from its edges: {now} plant cells after 30 minutes");
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * w.ledger.initial.iter().sum::<f64>());
}

/// Where the forests are (for screenshots): the densest stretches round a 5 km living world.
#[test]
#[ignore]
fn where_forest() {
    let w = World::new(living(5.0));
    let mut v: Vec<(usize, f64)> = (0..256).map(|i| i as f64 * std::f64::consts::TAU / 256.0).map(|a| (plant_cells(&w, a, 40.0).len(), a)).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    for (n, a) in v.iter().take(5) {
        let s = w.planet.surface_radius(a.cos(), a.sin());
        println!("{n} cells at angle {a:.3}: view {:.0},{:.0}", a.cos() * (s + 20.0), a.sin() * (s + 20.0));
    }
}
