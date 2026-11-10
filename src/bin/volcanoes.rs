//! Cut-away views of the volcanoes on a body: for each one (biggest first),
//! a tile turned so the volcano points up, from above the summit down past
//! its chamber, and under it the same tile in ambient heat.
//!
//! cargo run --release --bin volcanoes -- <body> <out.png> [seeds] [per_seed]
use dft_sim::relief::Kind;
use dft_sim::render;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (body, out) = (&a[0], &a[1]);
    let seeds: u64 = a.get(2).map_or(3, |s| s.parse().unwrap());
    let per: usize = a.get(3).map_or(2, |s| s.parse().unwrap());
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, body).unwrap();
    let size = 300usize;
    let mut tiles: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    for k in 0..seeds {
        let mut spec = base.spec.clone();
        spec.seed += k * 977;
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let Some(rel) = p.relief.as_ref() else { continue };
        let mut vs: Vec<_> = rel.features.iter().filter(|f| f.kind == Kind::Volcano).collect();
        vs.sort_by(|a, b| (b.width * b.height).partial_cmp(&(a.width * a.height)).unwrap());
        for f in vs.into_iter().take(per) {
            let (ux, uy) = (f.ux, f.uy);
            let (tx, ty) = (-uy, ux);
            let top = p.surface_radius(ux, uy);
            let zoom: f64 = std::env::var("ZOOM").ok().map_or(1.0, |z| z.parse().unwrap());
            let span = 2.4 * f.width / zoom;
            let mpp = span / size as f64;
            // From a little above the summit down to well below the chamber.
            let r_top = top + 0.15 * span * zoom.min(1.0) + if zoom > 1.0 { 0.1 * span } else { 0.0 };
            let mut mat = vec![0u8; size * size * 3];
            let mut heat = vec![0u8; size * size * 3];
            for row in 0..size {
                let r = r_top - (row as f64 + 0.5) * mpp;
                for col in 0..size {
                    let s = (col as f64 + 0.5) * mpp - span / 2.0;
                    let (wx, wy) = ((ux * r + tx * s).floor() as i64, (uy * r + ty * s).floor() as i64);
                    let i = (row * size + col) * 3;
                    let c = render::colour(&p, p.material(wx, wy), wx, wy);
                    mat[i..i + 3].copy_from_slice(&c);
                    let t = p.ambient_k(wx, wy);
                    let q = ((t - 200.0) / 1400.0).clamp(0.0, 1.0);
                    heat[i..i + 3].copy_from_slice(&[(255.0 * q.sqrt()) as u8, (255.0 * q * q) as u8, (90.0 * (1.0 - q)) as u8]);
                }
            }
            if std::env::var("PROBE").is_ok() {
                for (fr, fc) in [(0.4, 0.3), (0.5, 0.4), (0.6, 0.6)] {
                    let r = r_top - fr * span;
                    let sx = fc * span - span / 2.0;
                    let (wx, wy) = ((ux * r + tx * sx).floor() as i64, (uy * r + ty * sx).floor() as i64);
                    eprintln!("  probe depth {:.0} m: {}", top - r, p.mats.get(p.material(wx, wy)).name);
                }
                for fc in [0.1, 0.3, 0.5, 0.7, 0.9] {
                    let sx = fc * span - span / 2.0;
                    let (vx, vy) = ((ux * top + tx * sx) / top, (uy * top + ty * sx) / top);
                    let l = (vx * vx + vy * vy).sqrt();
                    eprintln!("  col {fc}: table {:?}", rel.table_at(vx / l, vy / l));
                }
                for g in &rel.features {
                    let d = ((g.ux - ux) * (g.ux - ux) + (g.uy - uy) * (g.uy - uy)).sqrt() * top;
                    if d < 3.0 * g.width.max(f.width) && g.width > 0.0 {
                        eprintln!("  near: {:?} w {:.0} h {:.0} at {:.0} m fill {}", g.kind, g.width, g.height, d, g.fill_m);
                    }
                }
            }
            eprintln!(
                "seed {} {} volcano w {:.0} m h {:.0} m {}{}",
                p.spec.seed,
                if f.steep { "steep" } else { "shield" },
                f.width,
                f.height,
                if f.active { "ACTIVE" } else { "extinct" },
                if f.feed_r > 0.0 { " (hotspot)" } else { "" }
            );
            tiles.push((mat, heat));
        }
    }
    let cols = tiles.len().max(1);
    let w = cols * size;
    let mut img = vec![0u8; w * size * 2 * 3];
    for (k, (m, h)) in tiles.iter().enumerate() {
        for y in 0..size {
            let src = y * size * 3..(y + 1) * size * 3;
            let d1 = (y * w + k * size) * 3;
            img[d1..d1 + size * 3].copy_from_slice(&m[src.clone()]);
            let d2 = ((y + size) * w + k * size) * 3;
            img[d2..d2 + size * 3].copy_from_slice(&h[src]);
        }
    }
    render::write_png(out, w as u32, (size * 2) as u32, &img).unwrap();
}
