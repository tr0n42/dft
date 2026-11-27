//! The far field (src/units/far.rs): the swarm far from the camera worked a
//! sector of surface at a time, as a count of cutters eating its ground
//! from the top down.
use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
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

/// A free swarm on the dev planet with a camera zoomed out over it and room
/// for only a few cutters flown one by one: most of it goes far.
fn far_swarm(planet: Arc<dft_sim::planet::Planet>, secs: f64) -> Sim {
    let mut s = Sim::new(World::shared(planet.clone()), eco());
    s.free_drones = Some(60_000);
    s.set_attention(Some(None));
    s.set_focus(Some(None));
    let r = s.world.planet.max_radius as i64 + 500;
    s.set_view(Some(([-r, r, -r, r], 2.0 * r as f64 / 1000.0)));
    s.far.budget = 2_000.0;
    s.far.fixed = true;
    s.run(secs);
    s
}

#[test]
fn far_sectors_eat_their_ground_and_deliver_it() {
    let planet = Arc::new(dft_sim::load_body(&data(), "dev-planet").unwrap());
    let s = far_swarm(planet, 400.0);
    let far = s.far_cutters();
    println!("far {far} of {} cutters, {} flown; eaten {:.0} kt, mined {:.0} kt", s.harvesters_alive(), s.drone_count(), s.far.eaten_kg / 1e6, s.world.ledger.mined_kg() / 1e6);
    assert!(far > 40_000, "most of the swarm should be far: {far}");
    assert!(s.drone_count() <= 4_000, "few flown one by one: {}", s.drone_count());
    assert!(s.far.eaten_kg > 1e8, "far sectors should eat: {} kg", s.far.eaten_kg);
    // What they ate is booked as mined, and what is gone reads as empty:
    // above every cut sector's level, nothing.
    assert!(s.world.ledger.mined_kg() >= s.far.eaten_kg);
    let n = s.world.far.levels.len();
    let mut checked = 0;
    for k in 0..n {
        let l = s.world.far.levels[k] as f64;
        if !l.is_finite() {
            continue;
        }
        let a = std::f64::consts::TAU * (k as f64 + 0.5) / n as f64;
        for dr in [dft_sim::world::FAR_RAMP_M + 2.0, 100.0] {
            let r = l + dr;
            let (x, y) = ((a.cos() * r).floor() as i64, (a.sin() * r).floor() as i64);
            assert_eq!(s.world.material(x, y), VACUUM, "sector {k}: ground left {dr} m over its level");
        }
        checked += 1;
    }
    assert!(checked > 0, "some sectors should be cut");
}

#[test]
fn far_field_survives_a_save() {
    let planet = Arc::new(dft_sim::load_body(&data(), "dev-planet").unwrap());
    let mut s = far_swarm(planet.clone(), 300.0);
    s.finish_blasts();
    let mut o = W::default();
    s.write_save(&mut o);
    let mut l = Sim::read_save(&mut R(&o.0, 0), planet, eco()).unwrap();
    assert_eq!(l.world.far.levels, s.world.far.levels);
    assert_eq!(l.far.cutters, s.far.cutters);
    assert_eq!(l.far_cutters(), s.far_cutters());
    if s.bodies.count() == 0 {
        assert_eq!(l.world.state_hash(), s.world.state_hash());
    }
    // And it carries on eating.
    l.free_drones = s.free_drones;
    let r = l.world.planet.max_radius as i64 + 500;
    l.set_view(Some(([-r, r, -r, r], 2.0 * r as f64 / 1000.0)));
    l.far.budget = 2_000.0;
    l.far.fixed = true;
    let before = l.far.eaten_kg;
    let t = l.time();
    l.run(t + 60.0);
    assert!(l.far.eaten_kg > before, "the loaded far field should keep eating");
}

#[test]
fn coming_close_brings_the_swarm_down_as_flocks() {
    let planet = Arc::new(dft_sim::load_body(&data(), "dev-planet").unwrap());
    let mut s = far_swarm(planet, 300.0);
    let flown = s.drone_count();
    // The camera comes in close over where the swarm landed: the sectors
    // it shows come down as flocks, whatever the budget.
    let (x, y) = s.mother_pos_now();
    let (x, y) = (x as i64, y as i64);
    s.set_view(Some(([x - 800, x + 800, y - 450, y + 450], 1.0)));
    s.set_focus(Some(Some([x - 1600, x + 1600, y - 900, y + 900])));
    let t = s.time();
    s.run(t + 30.0);
    println!("flown {flown} -> {}, far {}", s.drone_count(), s.far_cutters());
    assert!(s.drone_count() > flown + 1_000, "cutters should come down where the camera looks: {flown} -> {}", s.drone_count());
    let k = s.world.far.sector_of(x as f64, y as f64);
    assert!(!s.far.is_far[k], "the sector on screen is worked as flocks");
}
