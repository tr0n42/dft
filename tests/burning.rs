//! Fire: things burn when hot enough with air to burn with.
use dft_sim::sandbox::Sandbox;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The first cell of `name` found near the surface straight up from the
/// centre, scanning a strip of ground.
fn find(w: &World, name: &str) -> Option<(i64, i64)> {
    let id = w.planet.mats.id(name).ok()?;
    let top = w.planet.surface_radius(0.0, 1.0) as i64;
    for y in (top - 2000..top).rev() {
        for x in -3000..3000i64 {
            if w.material(x, y) == id {
                return Some((x, y));
            }
        }
    }
    None
}

/// Light a cell of `name` (opening the cell above it to the air) and let
/// the sandbox run; returns how many cells burned away.
fn light(body: &str, name: &str, oxygen: Option<f64>, secs: f64) -> Option<u64> {
    let mut p = dft_sim::load_body(&data(), body).unwrap();
    if let Some(o) = oxygen {
        let mut spec = p.spec.clone();
        spec.oxygen = o;
        p = dft_sim::planet::Planet::new(spec, p.mats.clone()).unwrap();
    }
    let mut w = World::new(p);
    let c = find(&w, name)?;

    // Open a little pocket around it so it has air.
    for (dx, dy) in [(0, 1), (1, 1), (-1, 1), (0, 2)] {
        w.mine_cell(c.0 + dx, c.1 + dy);
    }
    let mut sb = Sandbox::new(&w);
    sb.temps.insert(c, 1500.0);
    sb.wake(c);
    let ticks = (secs * w.planet.mats.physics.ticks_per_s) as usize;
    for i in 0..ticks {
        sb.step(&mut w);
    }
    println!("{body}: {name} lit, oxygen {}: {} burned away, {} burning-steps", w.planet.spec.oxygen, sb.stats.burned, sb.stats.burning);
    Some(sb.stats.burned)
}

#[test]
fn coal_burns_in_air() {
    let n = light("living-world", "coal", None, 300.0).expect("the living world should have coal near the surface");
    // (Here the seam is one cell thick under soil: the fire spreads along
    // it until loose soil falls into the pocket and smothers it.)
    assert!(n >= 1, "a lit coal seam open to the air should burn ({n})");
}

#[test]
fn nothing_burns_without_oxygen() {
    let n = light("living-world", "coal", Some(0.0), 120.0).unwrap();
    assert_eq!(n, 0);
}
