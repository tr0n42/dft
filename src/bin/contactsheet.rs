//! A grid of seeds for one body: whole cross-sections in the top row and a
//! close-up of the surface (the top of the disc) under each, to judge variety.
//!
//! cargo run --release --bin contactsheet -- <body> <out.png> [seeds] [closeup_m]
use dft_sim::render;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (body, out) = (&a[0], &a[1]);
    let n: u64 = a.get(2).map_or(6, |s| s.parse().unwrap());
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, body).unwrap();
    let size = 360u32;
    // Close-up: `width` metres wide at the top of the body, at 1 px per cell
    // scaled to the tile width.
    let width: i64 = a.get(3).map_or(360, |s| s.parse().unwrap());
    let (cols, rows) = (n as u32, 2u32);
    let mut img = vec![0u8; (cols * size * rows * size * 3) as usize];
    let stride = (cols * size * 3) as usize;
    for k in 0..n {
        let mut spec = base.spec.clone();
        spec.seed += k * 977;
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let whole = render::whole(&p, size);
        // Close-up centred on the surface straight up.
        // Optional 5th argument: centre the close-up at this fraction of the radius.
        let top = match a.get(4) {
            Some(f) => (f.parse::<f64>().unwrap() * p.radius) as i64 - (size as i64 / (size as i64 / width).max(1)) / 5 + (size as i64 / (size as i64 / width).max(1)) / 2,
            None => p.surface_radius(0.0, 1.0) as i64,
        };
        let scale = (size as i64 / width).max(1) as u32;
        let cells = (size / scale) as i64;
        // With surface features, centre the close-up on the biggest one.
        let (cx, cy) = match p.relief.as_ref().and_then(|r| r.features.iter().filter(|f| f.width > 0.0).max_by(|a, b| (a.height * a.width).partial_cmp(&(b.height * b.width)).unwrap())) {
            Some(f) if a.get(4).is_none() => {
                let rs = p.surface_radius(f.ux, f.uy);
                ((f.ux * rs) as i64, (f.uy * rs) as i64 + cells / 5)
            }
            _ => (0, top + cells / 5),
        };
        // SPAN=<metres>: a wider close-up of the crust straight up, sampled.
        let (close, cw) = match std::env::var("SPAN").ok().and_then(|v| v.parse::<f64>().ok()) {
            Some(span) => {
                let mpp = span / size as f64;
                let top = p.surface_radius(0.0, 1.0) + 0.1 * span;
                let mut img = vec![0u8; (size * size * 3) as usize];
                for row in 0..size as usize {
                    for col in 0..size as usize {
                        let (wx, wy) = ((-span / 2.0 + (col as f64 + 0.5) * mpp).floor() as i64, (top - (row as f64 + 0.5) * mpp).floor() as i64);
                        let i = (row * size as usize + col) * 3;
                        img[i..i + 3].copy_from_slice(&render::colour(&p, p.material(wx, wy), wx, wy));
                    }
                }
                (img, size as usize)
            }
            None => (render::region(&p, cx - cells / 2, cy, cells as u32, cells as u32, scale), (cells as u32 * scale) as usize),
        };
        for y in 0..size as usize {
            let dst = y * stride + (k as usize * size as usize) * 3;
            img[dst..dst + size as usize * 3].copy_from_slice(&whole[y * size as usize * 3..(y + 1) * size as usize * 3]);
            if y < cw {
                let d2 = (y + size as usize) * stride + (k as usize * size as usize) * 3;
                img[d2..d2 + cw * 3].copy_from_slice(&close[y * cw * 3..(y + 1) * cw * 3]);
            }
        }
        eprintln!("{body} seed {} done", p.spec.seed);
    }
    render::write_png(out, cols * size, rows * size, &img).unwrap();
}
