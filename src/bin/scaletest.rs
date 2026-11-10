//! Scale test: load a body and measure what it costs.
//! cargo run --release --bin scaletest -- <body>
use dft_sim::render;
use std::path::Path;
use std::time::Instant;

fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS")).map(|l| l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0))
        .unwrap_or(0.0)
}

fn main() {
    let name = std::env::args().nth(1).unwrap();
    let data = std::env::var("DATA").map(std::path::PathBuf::from).unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("data"));
    let m0 = rss_mb();
    let t = Instant::now();
    let mut p = dft_sim::load_body(&data, &name).unwrap();
    // GLAMOUR=g: run with the glamour pass at this setting.
    // GLAMOUR=g: the glamour pass at this setting; RADIUS_KM=r: resized.
    let glam = std::env::var("GLAMOUR").ok().and_then(|v| v.parse::<f64>().ok());
    let rad = std::env::var("RADIUS_KM").ok().and_then(|v| v.parse::<f64>().ok());
    if glam.is_some() || rad.is_some() {
        let mut spec = p.spec.clone();
        if glam.is_some() {
            spec.glamour = glam;
        }
        if let Some(r) = rad {
            spec.radius_m = r * 1000.0;
        }
        p = dft_sim::planet::Planet::new(spec, p.mats.clone()).unwrap();
    }
    println!("{name}: radius {:.0} km", p.radius / 1000.0);
    println!("  load:              {:>8.0} ms, memory +{:.0} MB", t.elapsed().as_secs_f64() * 1000.0, rss_mb() - m0);
    if let Some(s) = &p.strata {
        println!("  strata:            {} bands x {} directions", s.bands.len(), s.directions());
    }
    if let Some(i) = &p.intrusions {
        let (a, b) = i.counts();
        println!("  intrusions:        {a} plutons, {b} dikes and veins");
    }
    // Per-cell cost at different depths: 200k random cells in each band.
    for (label, lo, hi) in [("crust (top 30 km)", 0.995, 1.0), ("upper mantle", 0.9, 0.99), ("lower mantle", 0.6, 0.85), ("core", 0.0, 0.5)] {
        let mut h = 0u64;
        let n = 200_000;
        let t = Instant::now();
        for k in 0..n {
            let f = lo + (hi - lo) * dft_sim::noise::unit(dft_sim::noise::hash(7, k, 0, 1));
            let (ux, uy) = dft_sim::geology::random_dir(11, k);
            let r = f * p.radius;
            let m = p.material((ux * r) as i64, (uy * r) as i64);
            h = h.wrapping_add(m as u64);
        }
        println!("  per cell, {label:<18} {:>6.0} ns   ({h})", t.elapsed().as_secs_f64() * 1e9 / n as f64);
    }
    let t = Instant::now();
    let img = render::whole(&p, 1000);
    println!("  whole-planet 1000 px render (all threads): {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    render::write_png(&format!("{}/../scale-{name}.png", std::env::temp_dir().display()), 1000, 1000, &img).ok();
    if std::env::var("SKIP_SWARM").is_ok() {
        // Profile mode: hammer per-cell generation in the upper mantle.
        let t = Instant::now();
        let mut h = 0u64;
        for k in 0..3_000_000i64 {
            let f = 0.6 + 0.39 * dft_sim::noise::unit(dft_sim::noise::hash(9, k, 0, 1));
            let (ux, uy) = dft_sim::geology::random_dir(13, k);
            let r = f * p.radius;
            h = h.wrapping_add(p.material((ux * r) as i64, (uy * r) as i64) as u64);
        }
        println!("  profile loop: {:.1} s ({h})", t.elapsed().as_secs_f64());
        return;
    }
    let t = Instant::now();
    let w = dft_sim::world::World::new(p);
    println!("  world store set-up: {:.0} ms, memory now {:.0} MB", t.elapsed().as_secs_f64() * 1000.0, rss_mb());
    let eco = dft_sim::economy::Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let t = Instant::now();
    let mut s = dft_sim::swarm::Sim::new(w, eco);
    println!("  swarm set-up:       {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    if std::env::var("TRACE").is_ok() {
        let n_chunks: usize = std::env::var("TRACE").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
        for k in 0..n_chunks {
            let t = Instant::now();
            for _ in 0..50 {
                s.step();
            }
            println!("  ticks {:>5}: {:>7.1} ms per 50, awake {}, frontier {}, bodies {}, prof {:?}", (k + 1) * 50, t.elapsed().as_secs_f64() * 1000.0, s.sandbox.awake(), s.frontier_len(), s.bodies.count(), s.prof.map(|v| (v * 10.0).round() / 10.0));
        }
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        for c in s.sandbox.awake_cells() {
            *by.entry(s.world.planet.mats.get(s.world.material(c.0, c.1)).name.clone()).or_default() += 1;
        }
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.1));
        println!("  awake by material: {:?}", &v[..v.len().min(8)]);
        // Where the awake powder is: depth below the generated surface.
        let mut depths: std::collections::BTreeMap<i64, usize> = Default::default();
        for c in s.sandbox.awake_cells() {
            let m = s.world.material(c.0, c.1);
            if s.world.planet.mats.get(m).state != dft_sim::materials::State::Powder && std::env::var("DEPTH_OF").map_or(true, |n| s.world.planet.mats.get(m).name != n) { continue; }
            let (fx, fy) = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
            let r = (fx * fx + fy * fy).sqrt();
            let d = s.world.planet.surface_radius(fx / r, fy / r) - r;
            *depths.entry((d / 20.0).floor() as i64 * 20).or_default() += 1;
        }
        println!("  awake powder by depth (m): {:?}", depths);
        if let Ok(name) = std::env::var("DEPTH_OF") {
            // Where those cells are: inside the sea or not, by angle, and a
            // picture around the biggest bunch.
            let pl = &s.world.planet;
            let (mut sea, mut not) = (0, 0);
            let mut ang: std::collections::BTreeMap<i64, usize> = Default::default();
            let mut pts = Vec::new();
            for c in s.sandbox.awake_cells() {
                if pl.mats.get(s.world.material(c.0, c.1)).name != name { continue; }
                if pl.in_sea(c.0, c.1) { sea += 1 } else { not += 1 }
                *ang.entry(((c.0 as f64).atan2(c.1 as f64).to_degrees()).floor() as i64).or_default() += 1;
                pts.push(c);
            }
            println!("  {name}: {sea} in the sea, {not} not; by angle: {:?}", ang);
            if let (Ok(path), Some((&a, _))) = (std::env::var("DUMP"), ang.iter().max_by_key(|x| x.1)) {
                let near: Vec<_> = pts.iter().filter(|c| ((c.0 as f64).atan2(c.1 as f64).to_degrees()).floor() as i64 == a).collect();
                let (cx, cy) = (near.iter().map(|c| c.0).sum::<i64>() / near.len() as i64, near.iter().map(|c| c.1).sum::<i64>() / near.len() as i64);
                let (w, h) = (900i64, 600i64);
                let mut img = vec![0u8; (w * h * 3) as usize];
                // Upright: rotate so the local up points up the image.
                let rr = ((cx * cx + cy * cy) as f64).sqrt();
                let (ux, uy) = (cx as f64 / rr, cy as f64 / rr);
                for row in 0..h {
                    for col in 0..w {
                        let (dx, dy) = ((col - w / 2) as f64, (h / 3 - row) as f64);
                        let (x, y) = ((cx as f64 + dx * uy + dy * ux).floor() as i64, (cy as f64 - dx * ux + dy * uy).floor() as i64);
                        let m = s.world.material(x, y);
                        let col3 = if m == 0 { [10, 10, 20] } else { dft_sim::render::colour(pl, m, x, y) };
                        let i = ((row * w + col) * 3) as usize;
                        img[i..i + 3].copy_from_slice(&col3);
                    }
                }
                for c in &pts {
                    let (dx, dy) = ((c.0 - cx) as f64, (c.1 - cy) as f64);
                    let (col, row) = ((dx * uy - dy * ux) as i64 + w / 2, h / 3 - (dx * ux + dy * uy) as i64);
                    if col >= 0 && col < w && row >= 0 && row < h { let i = ((row * w + col) * 3) as usize; img[i..i + 3].copy_from_slice(&[255, 0, 255]); }
                }
                dft_sim::render::write_png(&path, w as u32, h as u32, &img).unwrap();
            }
        }
        let cells = s.bodies.cells();
        println!("  bodies: {} cells in the air; stats {:?}", cells.len(), s.bodies.stats);
        println!("  sandbox: {:?}", s.sandbox.stats);
        return;
    }
    let t = Instant::now();
    let play: f64 = std::env::var("PLAY_S").ok().and_then(|v| v.parse().ok()).unwrap_or(600.0);
    s.run(play);
    let fm: Vec<(String, usize)> = s.frontier_mats().into_iter().map(|(m, n)| (s.world.planet.mats.get(m).name.clone(), n)).collect();
    println!("  dig face: {fm:?}");
    println!("  swarm: {:.0} kg delivered, {} built, {} deaths ({} burned, {} crushed), energy {:.0}, last sample {:?}", s.delivered_credited, s.harvesters_built, s.deaths, s.burned, s.crushed, s.energy, s.samples.last());
    println!("  first {play:.0} s of play: {:.1} s, {} drones, memory {:.0} MB", t.elapsed().as_secs_f64(), s.drone_count(), rss_mb());
    // 32-bit float resolution at the surface (what the rock physics uses).
    let r32 = p_radius_f32(s.world.planet.radius);
    println!("  f32 spacing at the surface: {r32} m");
}

fn p_radius_f32(r: f64) -> f32 {
    let x = r as f32;
    f32::from_bits(x.to_bits() + 1) - x
}
