use dft_sim::planet::Planet;
use std::time::Instant;
fn main() {
    let d = std::path::Path::new("data");
    let mats = dft_sim::materials::MaterialTable::load("data/materials.toml").unwrap();
    let mut spec = dft_sim::spec::BodySpec::load(&d.join("bodies/living-world.toml").to_string_lossy()).unwrap();
    spec.life = Some(1.0);
    let t = Instant::now();
    let p = Planet::new(spec, mats).unwrap();
    println!("build {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let s = p.surface_radius(0.0, 1.0);
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..200_000i64 { acc += p.base_material(i % 500, s as i64 - 20 + (i / 500) % 40) as u64; }
    println!("base_material {:.2} us", t.elapsed().as_secs_f64() * 1e6 / 200_000.0);
    let t = Instant::now();
    for i in 0..20_000 { let a = i as f64 * 1e-5; acc += p.surface_radius(a.sin(), a.cos()) as u64; }
    println!("surface_radius {:.2} us", t.elapsed().as_secs_f64() * 1e6 / 20_000.0);
    let n = 40;
    let t = Instant::now();
    for k in 0..n {
        let a = 0.3 + k as f64 * 0.004;
        let r = p.surface_radius(a.sin(), a.cos());
        let (x, y) = ((a.sin() * r) as i64, (a.cos() * r) as i64);
        acc += p.material(x, y) as u64;
    }
    println!("surface chunk (with 3x3 anchors + plants) {:.1} ms each", t.elapsed().as_secs_f64() * 1e3 / n as f64);
    let t = Instant::now();
    for k in 0..n {
        let a = 0.3 + k as f64 * 0.004;
        let r = p.surface_radius(a.sin(), a.cos()) - 600.0;
        let (x, y) = ((a.sin() * r) as i64, (a.cos() * r) as i64);
        acc += p.material(x, y) as u64;
    }
    println!("deep chunk {:.1} ms each", t.elapsed().as_secs_f64() * 1e3 / n as f64);
    println!("{acc}");
}
