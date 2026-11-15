//! probe: what the generator gives straight down at one direction of a body
//! (for checking the ground the far field finds). Run: probe <body> <km> <x> <y>
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let body = a.first().cloned().unwrap_or("earth-test".into());
    let km: f64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(6371.0);
    let x: f64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let y: f64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let text = std::fs::read_to_string(data.join("bodies").join(format!("{body}.toml"))).unwrap();
    let text = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
    let spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
    let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
    let p = dft_sim::planet::Planet::new(spec, mats).unwrap();
    let r = (x * x + y * y).sqrt();
    let (ux, uy) = (x / r, y / r);
    let s = p.surface_radius(ux, uy);
    println!("surface {s:.0}, hint {:.0}, max radius {:.0}", p.ground_hint(ux, uy), p.max_radius);
    let w = dft_sim::world::World::new(p);
    let mut d = -200.0;
    while d < 60000.0 {
        let rr = s - d;
        let (cx, cy) = ((ux * rr).floor() as i64, (uy * rr).floor() as i64);
        let m = w.material_bare(cx, cy);
        println!("  {d:6.0} m down: {} ({})", w.planet.mats.get(m).name, m);
        d += if d < 1000.0 { 100.0 } else { 1000.0 };
    }
}
