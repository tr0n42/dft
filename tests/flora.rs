//! Life L1: each body's flora, laid over the generator.
use dft_sim::flora::Niche;
use dft_sim::planet::Planet;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn body(name: &str, life: Option<f64>, glamour: f64) -> Planet {
    let d = data();
    let mats = dft_sim::materials::MaterialTable::load(&d.join("materials.toml").to_string_lossy()).unwrap();
    let mut spec = dft_sim::spec::BodySpec::load(&d.join("bodies").join(format!("{name}.toml")).to_string_lossy()).unwrap();
    spec.life = life;
    spec.glamour = Some(glamour);
    Planet::new(spec, mats).unwrap()
}

/// A soil spot on the body (an open cell just above living ground).
fn soil_spot(p: &Planet) -> (i64, i64) {
    let f = p.flora.as_ref().unwrap();
    let &(_, x, y) = f.spots.iter().find(|s| s.0 == Niche::Soil).expect("a soil spot");
    (x, y)
}

#[test]
fn bodies_without_life_are_unchanged() {
    let p = body("living-world", None, 0.4);
    assert!(p.flora.is_none());
    for i in -200..200 {
        let y = p.surface_radius(0.0, 1.0) as i64 + i;
        assert_eq!(p.material(i * 3, y), p.base_material(i * 3, y));
    }
}

#[test]
fn living_world_is_alive_and_lava_world_is_not() {
    let p = body("living-world", Some(1.0), 0.4);
    let f = p.flora.as_ref().unwrap();
    assert!(f.species.len() >= 10, "{} species", f.species.len());
    assert!(f.report.iter().any(|r| r.niche == Niche::Soil && r.alive));
    assert!(f.species.iter().any(|s| s.plan == dft_sim::flora::Plan::Tree));
    // Every species has its own materials, coloured in the palette.
    assert_eq!(p.palette.colours.len(), p.mats.len());
    for s in &f.species {
        for m in [s.stem_m, s.foliage_m, s.organ_m, s.root_m] {
            assert!(p.mats.get(m).name.starts_with("flora "));
        }
    }
    let lava = body("lava-world", Some(1.0), 0.4);
    assert!(lava.flora.as_ref().unwrap().species.is_empty());
}

#[test]
fn plants_grow_only_into_open_space_soil_and_ground_cover() {
    let p = body("living-world", Some(1.0), 0.4);
    let f = p.flora.as_ref().unwrap();
    let mut most = 0;
    for &(_, cx, cy) in f.spots.iter().filter(|s| s.0 == Niche::Soil).take(12) {
        let mut plant = 0;
        for y in cy - 100..cy + 100 {
            for x in cx - 150..cx + 150 {
                let (m, b) = (p.material(x, y), p.base_material(x, y));
                if m != b {
                    plant += 1;
                    assert!(p.mats.get(m).name.starts_with("flora "), "{} at {x},{y}", p.mats.get(m).name);
                }
            }
        }
        most = most.max(plant);
    }
    assert!(most > 1000, "at most {most} plant cells around a soil spot");
}

#[test]
fn flora_is_the_same_whatever_order_cells_are_asked_in() {
    let a = body("living-world", Some(1.0), 0.4);
    let b = body("living-world", Some(1.0), 0.4);
    let (cx, cy) = soil_spot(&a);
    let cells: Vec<(i64, i64)> = (cy - 140..cy + 140).flat_map(|y| (cx - 300..cx + 300).map(move |x| (x, y))).collect();
    let fwd: Vec<u16> = cells.iter().map(|&(x, y)| a.material(x, y)).collect();
    let mut back: Vec<u16> = cells.iter().rev().map(|&(x, y)| b.material(x, y)).collect();
    back.reverse();
    assert_eq!(fwd, back);
}

/// Every plant cell touches the ground or another plant cell edge to edge
/// (otherwise the world would see loose pieces and drop them).
#[test]
fn plants_are_joined_edge_to_edge() {
    let mut checked = 0;
    for (name, g) in [("living-world", 0.3), ("living-world", 1.0), ("ice-moon", 0.8), ("dry-world", 0.6)] {
    let p = body(name, Some(1.0), g);
    let f = p.flora.as_ref().unwrap();
    let spots: Vec<(i64, i64)> = Niche::ALL.iter().flat_map(|n| f.spots.iter().filter(move |s| s.0 == *n).take(3).map(|s| (s.1, s.2))).collect();
    for &(cx, cy) in &spots {
        let (x0, y0, w) = (cx - 150, cy - 150, 300i64);
        let mut grid = vec![0u16; (w * w) as usize];
        let mut plant = vec![false; (w * w) as usize];
        for j in 0..w {
            for i in 0..w {
                let m = p.material(x0 + i, y0 + j);
                grid[(j * w + i) as usize] = m;
                plant[(j * w + i) as usize] = m != p.base_material(x0 + i, y0 + j);
            }
        }
        // Flood from every non-plant solid cell through plant cells.
        let solid = |m: u16| m != 0 && p.mats.get(m).state != dft_sim::materials::State::Liquid;
        let mut reached = vec![false; (w * w) as usize];
        let mut stack: Vec<(i64, i64)> = Vec::new();
        for j in 0..w {
            for i in 0..w {
                let k = (j * w + i) as usize;
                if solid(grid[k]) && !plant[k] {
                    reached[k] = true;
                    stack.push((i, j));
                }
            }
        }
        while let Some((i, j)) = stack.pop() {
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i + di, j + dj);
                if a < 0 || b < 0 || a >= w || b >= w {
                    continue;
                }
                let k = (b * w + a) as usize;
                if !reached[k] && plant[k] {
                    reached[k] = true;
                    stack.push((a, b));
                }
            }
        }
        // Plants cut by the window's edge may look loose: only count cells well inside.
        for j in 40..w - 40 {
            for i in 40..w - 40 {
                let k = (j * w + i) as usize;
                if plant[k] {
                    checked += 1;
                    assert!(reached[k], "{name} glamour {g}: loose plant cell at {},{}", x0 + i, y0 + j);
                }
            }
        }
    }
    }
    assert!(checked > 1000, "only {checked} plant cells checked");
}
