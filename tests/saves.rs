//! Saving a game mid-play and loading it back.
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

fn round_trip(body: &str, minutes: f64, free: Option<u64>) {
    let planet = Arc::new(dft_sim::load_body(&data(), body).unwrap());
    let mut s = Sim::new(World::shared(planet.clone()), eco());
    s.free_drones = free;
    s.run(minutes * 60.0);
    let t = std::time::Instant::now();
    s.finish_blasts();
    let mut o = W::default();
    s.write_save(&mut o);
    let raw = o.0;
    let packed = dft_sim::save::pack(b"TEST", 1, &raw);
    let save_s = t.elapsed().as_secs_f64();
    let t = std::time::Instant::now();
    let back = dft_sim::save::unpack(b"TEST", 1, &packed).unwrap();
    let mut l = Sim::read_save(&mut R(&back, 0), planet, eco()).unwrap();
    l.free_drones = free;
    let load_s = t.elapsed().as_secs_f64();
    println!(
        "{body} at {minutes} min: {} drones, frontier {}, loose {}, save {:.1} MB raw / {:.2} MB packed in {:.2} s, load {:.2} s",
        s.drone_count(),
        s.frontier_len(),
        s.bodies.count(),
        raw.len() as f64 / 1e6,
        packed.len() as f64 / 1e6,
        save_s,
        load_s
    );
    assert_eq!(l.tick, s.tick);
    assert_eq!(l.drone_count(), s.drone_count());
    assert_eq!(l.hubs.len(), s.hubs.len());
    assert_eq!(l.energy, s.energy);
    assert_eq!(l.mass, s.mass);
    // The dig face comes back whole (cells that moved since are dropped).
    assert!(l.frontier_len() + 50 >= s.frontier_len(), "frontier {} vs {}", l.frontier_len(), s.frontier_len());
    if s.bodies.count() == 0 {
        assert_eq!(l.world.state_hash(), s.world.state_hash());
    }
    // And the loaded game carries on digging.
    let before = l.consumed_share();
    let t0 = l.time();
    l.run(t0 + 120.0);
    assert!(l.consumed_share() > before, "the loaded game should keep digging");
    assert!(l.drone_count() > 0);
}

#[test]
fn save_and_load_small_body() {
    round_trip("tutorial-1-2km", 20.0, None);
}

#[test]
fn save_and_load_free_swarm() {
    round_trip("rubble-asteroid", 5.0, Some(3000));
}

/// A save in the layout before version 18 (the loose and cracked cells kept
/// as lists of cells, not bit tiles) still loads, and comes back as a new
/// save does: the same cells loose and cracked, the same cells settling,
/// and saved again, the same bytes.
#[test]
fn a_save_from_the_version_before_still_loads() {
    let base = dft_sim::load_body(&data(), "barren-world").unwrap();
    let mut spec = base.spec.clone();
    spec.radius_m = 3000.0;
    let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
    let mut s = Sim::new(World::new(p), eco());
    s.free_drones = Some(0);
    s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), 10.0);
    for _ in 0..50 {
        s.step();
    }
    s.finish_blasts();
    let write = |s: &Sim, version: u32| {
        let mut o = W::default();
        s.write_save_as(&mut o, version);
        o.0
    };
    let (old_bytes, new_bytes) = (write(&s, 17), write(&s, 18));
    let planet = s.world.planet.clone();
    let read = |b: &[u8]| Sim::read_save(&mut R(b, 0), planet.clone(), eco()).unwrap();
    let (old, new) = (read(&old_bytes), read(&new_bytes));
    let cells = |s: &Sim| {
        let mut loose: Vec<(i64, i64)> = s.loose_cells().collect();
        let mut cracked: Vec<(i64, i64)> = s.cracked_cells().collect();
        loose.sort_unstable();
        cracked.sort_unstable();
        (loose, cracked)
    };
    let empty = |s: &Sim, cells: &[(i64, i64)]| cells.iter().filter(|c| s.world.material(c.0, c.1) == VACUUM).count();
    let want = cells(&s);
    assert!(want.0.len() > 1000 && want.1.len() > 1000, "the blast should leave rubble and cracks ({} loose, {} cracked)", want.0.len(), want.1.len());
    // What was thrown out, and the holes rubble settled out of, came off
    // both sets.
    assert_eq!((empty(&s, &want.0), empty(&s, &want.1)), (0, 0), "empty cells still loose, and cracked");
    assert_eq!(cells(&old), want);
    assert_eq!(cells(&new), want);
    assert_eq!(old.loose_count(), new.loose_count());
    assert_eq!(write(&old, 18), write(&new, 18));
    assert!(new_bytes.len() < old_bytes.len(), "bit tiles should be smaller: {} bytes against {}", new_bytes.len(), old_bytes.len());

    // Saves before version 18 kept cells dug away in both sets. Two such
    // cells, emptied behind the swarm's back: a version 17 save loses them
    // as it loads, and keeps the rest.
    let only = |a: &[(i64, i64)], b: &[(i64, i64)]| *a.iter().find(|c| b.binary_search(c).is_err()).unwrap();
    let gone = (only(&want.0, &want.1), only(&want.1, &want.0));
    assert!(s.world.lift_cell(gone.0 .0, gone.0 .1).is_some() && s.world.lift_cell(gone.1 .0, gone.1 .1).is_some());
    let old = read(&write(&s, 17));
    let (loose, cracked) = cells(&old);
    assert_eq!((empty(&old, &loose), empty(&old, &cracked)), (0, 0), "a version 17 save brought back empty cells, loose and cracked");
    let kept = |v: &[(i64, i64)], c| v.iter().copied().filter(|&q| q != c).collect::<Vec<_>>();
    assert_eq!(loose, kept(&want.0, gone.0));
    assert_eq!(cracked, kept(&want.1, gone.1));
    assert_eq!(old.loose_count().1, loose.len(), "every loose cell left settles");
}
