//! sizesheet: body types at a range of sizes, to check that resizing keeps
//! each kind looking like itself.
//!
//! cargo run --release --bin sizesheet -- <out.png> <radius_km,...> <body> [body ...]
use dft_sim::render;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let out = &a[0];
    let radii: Vec<f64> = a[1].split(',').map(|x| x.parse::<f64>().unwrap() * 1000.0).collect();
    let bodies = &a[2..];
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
    let (size, strip) = (220u32, 60u32);
    let th = size + strip;
    let (w, h) = (size * radii.len() as u32, th * bodies.len() as u32);
    let mut img = vec![0u8; (w * h * 3) as usize];
    for (row, b) in bodies.iter().enumerate() {
        let text = std::fs::read_to_string(data.join("bodies").join(format!("{b}.toml"))).unwrap();
        for (col, &r) in radii.iter().enumerate() {
            let t = std::time::Instant::now();
            let resized = dft_sim::spec::resize_text(&text, r).unwrap();
            let spec = dft_sim::spec::BodySpec::parse(&resized).unwrap();
            let p = dft_sim::planet::Planet::new(spec, mats.clone()).unwrap();
            let d = render::whole(&p, size);
            let cells = |x: i64, y: i64| p.material(x, y);
            let below = (0.15 * p.radius).clamp(100.0, 20_000.0);
            let s = render::strip(&p, &cells, 0.0, std::f64::consts::TAU, p.max_radius, p.radius - below, size, strip);
            for y in 0..th as usize {
                let src: &[u8] = if y < size as usize { &d[y * size as usize * 3..(y + 1) * size as usize * 3] } else { &s[(y - size as usize) * size as usize * 3..(y - size as usize + 1) * size as usize * 3] };
                let dst = ((row * th as usize + y) * w as usize + col * size as usize) * 3;
                img[dst..dst + size as usize * 3].copy_from_slice(src);
            }
            eprintln!("{b} at {:.0} km: {:.0} ms", r / 1000.0, t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    render::write_png(out, w, h, &img).unwrap();
}
