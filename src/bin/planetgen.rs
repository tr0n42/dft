//! planetgen: build bodies from their specs, render previews and report totals.
//!
//! Usage: planetgen [--data data] [--out out] [--exact] [body ...]
//! With no body names, every spec in data/bodies is built.

use dft_sim::render;
use dft_sim::totals::{self, Node, Tree};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

const G: f64 = 6.674e-11;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut data = PathBuf::from("data");
    let mut out = PathBuf::from("out");
    let mut want_exact = false;
    let mut bodies = Vec::new();
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
            "--exact" => want_exact = true,
            b => bodies.push(b.to_string()),
        }
        i += 1;
    }
    if bodies.is_empty() {
        let mut names: Vec<String> = std::fs::read_dir(data.join("bodies"))
            .expect("data/bodies folder")
            .filter_map(|e| e.ok()?.path().file_stem()?.to_str().map(String::from))
            .collect();
        names.sort();
        bodies = names;
    }
    std::fs::create_dir_all(&out).unwrap();
    for b in &bodies {
        if let Err(e) = build(&data, &out, b, want_exact) {
            eprintln!("{b}: {e}");
            std::process::exit(1);
        }
    }
}

fn kg_text(kg: f64) -> String {
    let units = [(1e21, "Zt"), (1e18, "Et"), (1e15, "Pt"), (1e12, "Gt"), (1e9, "Mt"), (1e6, "kt"), (1e3, "t")];
    for (s, u) in units {
        if kg >= s {
            return format!("{:.2} {u}", kg / s);
        }
    }
    format!("{kg:.0} kg")
}

fn build(data: &PathBuf, out: &PathBuf, body: &str, want_exact: bool) -> Result<(), String> {
    let t0 = Instant::now();
    let p = dft_sim::load_body(data, body)?;
    let tree = Tree::for_planet(&p);
    let mut rep = String::new();
    writeln!(rep, "{} ({})", p.spec.name, p.spec.archetype).unwrap();
    writeln!(rep, "seed {}, radius {:.0} m, quadtree depth {} levels", p.spec.seed, p.radius, tree.depth()).unwrap();
    writeln!(rep, "load: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3).unwrap();

    // Previews.
    let t = Instant::now();
    let img = render::whole(&p, 1024);
    render::write_png(&out.join(format!("{body}.png")).to_string_lossy(), 1024, 1024, &img)?;
    writeln!(rep, "whole-body preview (1024 x 1024 samples): {:.0} ms", t.elapsed().as_secs_f64() * 1e3).unwrap();

    let top = p.surface_radius(0.0, 1.0).round() as i64;
    let t = Instant::now();
    let img = render::region(&p, -256, top + 60, 512, 384, 2);
    render::write_png(&out.join(format!("{body}-surface.png")).to_string_lossy(), 1024, 768, &img)?;
    writeln!(rep, "surface close-up (512 x 384 cells at 1 m): {:.0} ms", t.elapsed().as_secs_f64() * 1e3).unwrap();
    if p.spec.heart {
        let img = render::region(&p, -64, 48, 128, 96, 8);
        render::write_png(&out.join(format!("{body}-heart.png")).to_string_lossy(), 1024, 768, &img)?;
        // Crust and mantle boundary, along the +x axis.
        let r = (p.radius * 0.86) as i64;
        let img = render::region(&p, r - 256, 192, 512, 384, 2);
        render::write_png(&out.join(format!("{body}-crust-mantle.png")).to_string_lossy(), 1024, 768, &img)?;
    } else {
        let r = (p.radius * 0.5) as i64;
        let img = render::region(&p, r - 256, 192, 512, 384, 2);
        render::write_png(&out.join(format!("{body}-interior.png")).to_string_lossy(), 1024, 768, &img)?;
    }

    // Totals.
    let root = tree.root();
    let t = Instant::now();
    let est = totals::estimate(&p, &tree, root, 1024);
    let est_ms = t.elapsed().as_secs_f64() * 1e3;
    writeln!(rep, "\nwhole-body estimate (1,048,576 samples): {est_ms:.0} ms").unwrap();
    let mut final_totals = est.clone();
    let mut label = "estimated";
    if want_exact {
        let t = Instant::now();
        let ex = totals::exact(&p, &tree, root);
        let ex_ms = t.elapsed().as_secs_f64() * 1e3;
        let cells: f64 = ex.cells[1..].iter().sum();
        writeln!(rep, "whole-body exact count ({cells:.0} non-vacuum cells): {ex_ms:.0} ms").unwrap();
        let (me, mx) = (est.total_kg(&p.mats), ex.total_kg(&p.mats));
        writeln!(rep, "estimate vs exact, total mass: {:+.3}%", (me - mx) / mx * 100.0).unwrap();
        final_totals = ex;
        label = "exact";
    }

    let kg = final_totals.kg(&p.mats);
    let total: f64 = kg.iter().sum();
    let area: f64 = final_totals.cells[1..].iter().sum();
    let density = total / area.max(1.0);
    let g = 4.0 / 3.0 * std::f64::consts::PI * G * density * p.radius;
    writeln!(rep, "\nmass of the cross-section ({label}): {}", kg_text(total)).unwrap();
    writeln!(rep, "mean density {density:.0} kg/m3, surface gravity {g:.4} m/s2 ({:.4} g)", g / 9.81).unwrap();
    writeln!(rep, "\nmaterials by mass:").unwrap();
    let mut by: Vec<(usize, f64)> = kg.iter().copied().enumerate().filter(|(i, k)| *i != 0 && *k > 0.0).collect();
    by.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (i, k) in &by {
        writeln!(rep, "  {:<14} {:>12}  {:>7.3}%", p.mats.get(*i as dft_sim::materials::MatId).name, kg_text(*k), k / total * 100.0).unwrap();
    }
    writeln!(rep, "\nelements by mass:").unwrap();
    let mut els: Vec<(String, f64)> = final_totals.elements(&p.mats).into_iter().collect();
    els.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (e, k) in &els {
        writeln!(rep, "  {e:<3} {:>12}  {:>9.5}%", kg_text(*k), k / total * 100.0).unwrap();
    }

    // Parts sum to the whole: an exact parent equals the sum of its exact children.
    let probe = Node { level: 4.min(tree.root_level), ix: 0, iy: 0 };
    let probe = {
        // The node of that level containing the top of the surface.
        let s = probe.side();
        Node { level: probe.level, ix: (0 - tree.origin) / s, iy: (top - 10 - tree.origin) / s }
    };
    let parent = totals::exact(&p, &tree, probe);
    let mut sum = totals::Totals::zero(p.mats.len());
    for c in probe.children() {
        sum.add(&totals::exact(&p, &tree, c));
    }
    writeln!(
        rep,
        "\nparts-sum-to-whole check on a {} m surface node: {}",
        probe.side(),
        if parent == sum { "pass" } else { "FAIL" }
    )
    .unwrap();
    writeln!(rep, "total time: {:.1} s", t0.elapsed().as_secs_f64()).unwrap();

    print!("{rep}\n");
    std::fs::write(out.join(format!("{body}-report.txt")), rep).map_err(|e| e.to_string())?;
    Ok(())
}
