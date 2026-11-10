//! crustmap: the crust unrolled into a strip (angle across, height down), and
//! the whole disc, to judge a body's surface and upper layers at a glance.
//!
//! cargo run --release --bin crustmap -- <body> <out.png> [above_m] [below_m] [seed_offset]
use dft_sim::render;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (body, out) = (&a[0], &a[1]);
    let above: f64 = a.get(2).map_or(5000.0, |s| s.parse().unwrap());
    let below: f64 = a.get(3).map_or(6000.0, |s| s.parse().unwrap());
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut p = dft_sim::load_body(&data, body).unwrap();
    if let Some(k) = a.get(4) {
        let mut spec = p.spec.clone();
        spec.seed += k.parse::<u64>().unwrap() * 977;
        p = dft_sim::planet::Planet::new(spec, p.mats.clone()).unwrap();
    }
    let (w, h, disc) = (1800u32, 500u32, 500u32);
    let mut img = vec![0u8; (w * (h + disc) * 3) as usize];
    let r0 = p.radius;
    for col in 0..w {
        let ang = std::f64::consts::TAU * (col as f64 + 0.5) / w as f64;
        let (ux, uy) = (ang.sin(), ang.cos());
        for row in 0..h {
            let r = r0 + above - (row as f64 + 0.5) * (above + below) / h as f64;
            let (x, y) = ((ux * r).floor() as i64, (uy * r).floor() as i64);
            let m = p.material(x, y);
            let c = if m == 0 { render::space(&p, x, y) } else { render::colour(&p, m, x, y) };
            let i = ((row * w + col) * 3) as usize;
            img[i..i + 3].copy_from_slice(&c);
        }
    }
    let whole = render::whole(&p, disc);
    for y in 0..disc as usize {
        let dst = ((h as usize + y) * w as usize) * 3;
        img[dst..dst + disc as usize * 3].copy_from_slice(&whole[y * disc as usize * 3..(y + 1) * disc as usize * 3]);
    }
    render::write_png(out, w, h + disc, &img).unwrap();
}
