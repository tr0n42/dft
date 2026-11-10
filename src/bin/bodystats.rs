//! Compare bodies by what matters for pacing: total mass, empty space, the
//! work to dig it all, and value-weighted mass. Samples every 2nd cell.
//!
//! cargo run --release --bin bodystats -- <body> [<body> ...]
use dft_sim::economy::Economy;
use dft_sim::materials::{State, VACUUM};
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    for name in std::env::args().skip(1) {
        let t = std::time::Instant::now();
        let p = dft_sim::load_body(&data, &name).unwrap();
        let load_ms = t.elapsed().as_millis();
        let r = p.max_radius.ceil() as i64;
        let step = ((r / 1500).max(1)) as usize * 2;
        let (mut cells, mut void, mut kg, mut work, mut valued) = (0f64, 0f64, 0f64, 0f64, 0f64);
        let mut by: BTreeMap<String, f64> = BTreeMap::new();
        for y in (-r..=r).step_by(step) {
            for x in (-r..=r).step_by(step) {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let rr = (fx * fx + fy * fy).sqrt();
                if rr > p.surface_radius(fx / rr.max(1e-9), fy / rr.max(1e-9)) {
                    continue;
                }
                cells += 1.0;
                let m = p.material(x, y);
                let mat = p.mats.get(m);
                if m == VACUUM {
                    void += 1.0;
                    continue;
                }
                kg += mat.density;
                let h = if mat.state == State::Solid { mat.hardness as f64 } else { 1.0 };
                work += mat.density * eco.harvester.dig_s_for(h);
                valued += mat.density * mat.multiplier;
                *by.entry(mat.name.clone()).or_default() += mat.density;
            }
        }
        let s = (step * step) as f64;
        println!("{name} (loaded in {load_ms} ms)");
        println!("  inside the surface {:>10.3e} cells, empty {:>5.1}%", cells * s, void / cells * 100.0);
        println!("  mass               {:>10.3e} kg", kg * s);
        println!("  dig work (rel.)    {:>10.3e}   per kg {:.3}", work * s / 1e3, work / kg);
        println!("  value-weighted     {:>10.3e} kg   x{:.3}", valued * s, valued / kg);
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (n, k) in v.iter().take(40) {
            println!("    {n:<16} {:>6.2}%", k / kg * 100.0);
        }
    }
}
