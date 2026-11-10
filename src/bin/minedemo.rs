//! minedemo: exercise the world store on both starter bodies and report.
//!
//! Asteroid: 40 tunnels dug cell by cell, 16 surface regions strip-mined at
//! once, and a 200 m cavern. Planetoid: a shaft from the surface to the Heart.
//! Each run saves, reloads, checks the books and renders the result.
//!
//! Usage: minedemo [--data data] [--out out]

use dft_sim::materials::VACUUM;
use dft_sim::noise::{hash, unit};
use dft_sim::render;
use dft_sim::save;
use dft_sim::world::World;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

fn kg_text(kg: f64) -> String {
    for (s, u) in [(1e12, "Gt"), (1e9, "Mt"), (1e6, "kt"), (1e3, "t")] {
        if kg.abs() >= s {
            return format!("{:.2} {u}", kg / s);
        }
    }
    format!("{kg:.0} kg")
}

fn bytes_text(b: usize) -> String {
    if b >= 1 << 20 {
        format!("{:.2} MB", b as f64 / (1 << 20) as f64)
    } else {
        format!("{:.1} KB", b as f64 / 1024.0)
    }
}

fn report_books(rep: &mut String, w: &World) {
    let l = &w.ledger;
    let mined = l.mined_kg();
    let recon: f64 = l.reconciled.iter().sum();
    writeln!(rep, "  mass mined: {} (credited {}, of which processing bonus {})", kg_text(mined), kg_text(l.credited), kg_text(l.bonus)).unwrap();
    let mut by: Vec<(usize, f64)> = l.mined.iter().copied().enumerate().filter(|(_, k)| *k > 0.0).collect();
    by.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (m, k) in by {
        writeln!(rep, "    {:<14} {:>11}", w.planet.mats.get(m as dft_sim::materials::MatId).name, kg_text(k)).unwrap();
    }
    let initial: f64 = l.initial.iter().sum();
    writeln!(rep, "  share of the body mined: {:.3}%", mined / initial * 100.0).unwrap();
    writeln!(
        rep,
        "  estimate corrections when regions were generated exactly: {} ({:+.3}% of the {} in those regions)",
        kg_text(recon),
        if l.generated > 0.0 { recon / l.generated * 100.0 } else { 0.0 },
        kg_text(l.generated)
    )
    .unwrap();
    let imb = l.imbalance(&w.remaining_kg());
    writeln!(rep, "  books balance: {} (largest mismatch {imb:.2e} kg)", if imb < 1e-6 * initial { "yes" } else { "NO" }).unwrap();
}

fn report_store(rep: &mut String, w: &World) {
    let (untouched, split, dense, empty, dense_bytes) = w.stats();
    writeln!(
        rep,
        "  store: {} entries ({untouched} untouched, {split} opened, {dense} exact leaves, {empty} empty); cell data {}",
        w.nodes.len(),
        bytes_text(dense_bytes)
    )
    .unwrap();
}

fn report_save(rep: &mut String, w: &World, data: &PathBuf, body: &str, out: &PathBuf) -> Result<(), String> {
    let t = Instant::now();
    let (raw, bytes) = save::save(w);
    let save_ms = t.elapsed().as_secs_f64() * 1e3;
    std::fs::write(out.join(format!("{body}.dfts")), &bytes).map_err(|e| e.to_string())?;
    let t = Instant::now();
    let back = save::load(&bytes, dft_sim::load_body(data, body)?)?;
    let load_ms = t.elapsed().as_secs_f64() * 1e3;
    let same = back.state_hash() == w.state_hash();
    writeln!(
        rep,
        "  save: {} uncompressed, {} on disk; saved in {save_ms:.0} ms, loaded in {load_ms:.0} ms; reload identical: {}",
        bytes_text(raw),
        bytes_text(bytes.len()),
        if same { "yes" } else { "NO" }
    )
    .unwrap();
    writeln!(rep, "  world hash: {:016x}", w.state_hash()).unwrap();
    Ok(())
}

fn asteroid(data: &PathBuf, out: &PathBuf, rep: &mut String) -> Result<(), String> {
    let body = "rubble-asteroid";
    let t0 = Instant::now();
    let mut w = World::new(dft_sim::load_body(data, body)?);
    writeln!(rep, "Rubble asteroid").unwrap();
    writeln!(rep, "  start: estimate of the whole body in {:.0} ms", t0.elapsed().as_secs_f64() * 1e3).unwrap();
    let r = w.planet.radius;

    // 40 tunnels, 3 m wide, from the surface 60% of the way to the centre.
    let t = Instant::now();
    let mut cells = 0u64;
    for k in 0..40u64 {
        let a = unit(hash(9, k as i64, 0, 1));
        // Direction without trig: walk the unit square's edge and normalise.
        let (dx, dy) = {
            let s = a * 8.0;
            let (x, y) = match s as u32 {
                0 | 1 => (1.0, s - 1.0),
                2 | 3 => (1.0 - (s - 3.0).max(-1.0).min(1.0), 1.0),
                4 | 5 => (-1.0, 5.0 - s),
                _ => (s - 7.0, -1.0),
            };
            let l: f64 = (x * x + y * y as f64).sqrt();
            (x / l, y / l)
        };
        let start = w.planet.surface_radius(dx, dy);
        let mut d = start;
        while d > start - 0.6 * r {
            let (cx, cy) = ((dx * d).floor() as i64, (dy * d).floor() as i64);
            for oy in -1..=1 {
                for ox in -1..=1 {
                    cells += 1;
                    w.mine_cell(cx + ox, cy + oy);
                }
            }
            d -= 1.0;
        }
    }
    let ms = t.elapsed().as_secs_f64() * 1e3;
    writeln!(rep, "  40 tunnels, cell by cell: {cells} cell operations in {ms:.0} ms ({:.2} us each)", ms * 1e3 / cells as f64).unwrap();

    // Strip-mine 16 surface regions, 512 m on a side, in one operation each.
    let t = Instant::now();
    let mut paid = 0.0;
    let mut regions = 0;
    let s = 512.0;
    for k in 0..64 {
        if regions == 16 {
            break;
        }
        let (gx, gy) = (k % 8, k / 8);
        let (x0, y0) = (w.tree.origin as f64 + 2048.0 + gx as f64 * s, w.tree.origin as f64 + 2048.0 + gy as f64 * s);
        let (cx, cy) = (x0 + s / 2.0, y0 + s / 2.0);
        let rr = (cx * cx + cy * cy).sqrt();
        if rr > 0.8 * r && rr < 1.15 * r {
            let n = w.node_at(3, cx as i64, cy as i64).unwrap();
            paid += w.mine_node(n).iter().sum::<f64>();
            regions += 1;
        }
    }
    let ms = t.elapsed().as_secs_f64() * 1e3;
    writeln!(rep, "  {regions} surface regions strip-mined whole: {} in {ms:.1} ms ({:.2} ms each)", kg_text(paid), ms / regions as f64).unwrap();

    // A 200 m cavern dug cell by cell, a third of the way down.
    let t = Instant::now();
    let (cx, cy) = (0i64, (0.3 * r) as i64);
    let mut n = 0u64;
    for y in cy - 100..=cy + 100 {
        for x in cx - 100..=cx + 100 {
            if (x - cx).pow(2) + (y - cy).pow(2) <= 100 * 100 {
                n += 1;
                w.mine_cell(x, y);
            }
        }
    }
    writeln!(rep, "  200 m cavern: {n} cell operations in {:.0} ms", t.elapsed().as_secs_f64() * 1e3).unwrap();

    report_books(rep, &w);
    report_store(rep, &w);
    report_save(rep, &w, data, body, out)?;

    let cells_fn = |x: i64, y: i64| w.material(x, y);
    let img = render::whole_with(&w.planet, &cells_fn, 1024);
    render::write_png(&out.join(format!("{body}-mined.png")).to_string_lossy(), 1024, 1024, &img)?;
    let img = render::region_with(&w.planet, &cells_fn, -256, cy + 192, 512, 384, 2);
    render::write_png(&out.join(format!("{body}-cavern.png")).to_string_lossy(), 1024, 768, &img)?;
    Ok(())
}

fn planetoid(data: &PathBuf, out: &PathBuf, rep: &mut String) -> Result<(), String> {
    let body = "active-planetoid";
    let t0 = Instant::now();
    let mut w = World::new(dft_sim::load_body(data, body)?);
    writeln!(rep, "\nActive planetoid").unwrap();
    writeln!(rep, "  start: estimate of the whole body in {:.0} ms", t0.elapsed().as_secs_f64() * 1e3).unwrap();
    let heart = w.planet.mats.id("heart")?;

    // A 3 m shaft straight down from the surface to the Heart.
    let t = Instant::now();
    let top = w.planet.surface_radius(0.0, 1.0).ceil() as i64;
    let mut ops = 0u64;
    let mut claimed = false;
    for y in (0..=top).rev() {
        for x in -1..=1 {
            ops += 1;
            if w.mine_cell(x, y) == Some(heart) {
                claimed = true;
            }
        }
    }
    let ms = t.elapsed().as_secs_f64() * 1e3;
    writeln!(rep, "  shaft to the centre, {} m deep: {ops} cell operations in {ms:.0} ms", top).unwrap();
    writeln!(rep, "  Heart claimed: {}", if claimed { "yes" } else { "NO" }).unwrap();
    assert_eq!(w.material(0, 0), VACUUM);

    report_books(rep, &w);
    report_store(rep, &w);
    report_save(rep, &w, data, body, out)?;

    let cells_fn = |x: i64, y: i64| w.material(x, y);
    let img = render::region_with(&w.planet, &cells_fn, -256, top + 60, 512, 384, 2);
    render::write_png(&out.join(format!("{body}-shaft-top.png")).to_string_lossy(), 1024, 768, &img)?;
    let img = render::region_with(&w.planet, &cells_fn, -64, 48, 128, 96, 8);
    render::write_png(&out.join(format!("{body}-shaft-heart.png")).to_string_lossy(), 1024, 768, &img)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut data = PathBuf::from("data");
    let mut out = PathBuf::from("out");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--data" => {
                data = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            "--out" => {
                out = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    std::fs::create_dir_all(&out).unwrap();
    let mut rep = String::new();
    let t = Instant::now();
    if let Err(e) = asteroid(&data, &out, &mut rep).and_then(|_| planetoid(&data, &out, &mut rep)) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    writeln!(rep, "\ntotal time: {:.1} s", t.elapsed().as_secs_f64()).unwrap();
    print!("{rep}");
    std::fs::write(out.join("minedemo-report.txt"), rep).unwrap();
}
