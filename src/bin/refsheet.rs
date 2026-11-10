//! refsheet: the locked reference for each body: pictures and a spec sheet.
//!
//! cargo run --release --bin refsheet -- [--out reference] [--glamour g] [body ...]
//! Writes <out>/<body>/{disc,surface,dive,seeds}.png and sheet.md, and
//! <out>/README.md indexing them. Timings come from data/reference/timings.toml
//! if present (scaletest results).
use dft_sim::render;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = PathBuf::from("reference");
    let mut glamour: Option<f64> = None;
    let mut bodies = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                out = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            "--glamour" => {
                glamour = Some(args[i + 1].parse().unwrap());
                i += 1;
            }
            b => bodies.push(b.to_string()),
        }
        i += 1;
    }
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    if bodies.is_empty() {
        let mut v: Vec<String> = std::fs::read_dir(data.join("bodies")).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).map(|p| p.file_stem().unwrap().to_string_lossy().into_owned()).collect();
        v.sort();
        bodies = v;
    }
    let timings: BTreeMap<String, toml::Value> = std::fs::read_to_string(data.join("reference").join("timings.toml")).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default();
    let mut index = String::from("# Reference sheets\n\nThe realistic models, locked in. Each body: its spec sheet, the whole disc, the surface unrolled, a dive from the surface to the centre, and four other seeds.\n\n| Body | Radius | Fingerprint | 10 min of play |\n|---|---|---|---|\n");
    for b in &bodies {
        let mut p = dft_sim::load_body(&data, b).unwrap();
        if let Some(g) = glamour {
            let mut spec = p.spec.clone();
            spec.glamour = Some(g);
            p = dft_sim::planet::Planet::new(spec, p.mats.clone()).unwrap();
        }
        let dir = out.join(b);
        std::fs::create_dir_all(&dir).unwrap();
        let cells = |x: i64, y: i64| p.material(x, y);
        // The whole disc.
        render::write_png(&dir.join("disc.png").to_string_lossy(), 700, 700, &render::whole(&p, 700)).unwrap();
        // The surface, unrolled: from the highest peak down a fifth of the
        // radius (at most 12 km).
        let above = p.max_radius - p.radius + 200.0;
        let below = (0.2 * p.radius).clamp(400.0, 12_000.0);
        let tau = std::f64::consts::TAU;
        render::write_png(&dir.join("surface.png").to_string_lossy(), 1800, 400, &render::strip(&p, &cells, 0.0, tau, p.radius + above, p.radius - below, 1800, 400)).unwrap();
        // A dive: a wedge 3 radii wide... as wide as it is deep at mid-depth,
        // from the surface to the centre.
        let wedge = 0.6;
        render::write_png(&dir.join("dive.png").to_string_lossy(), 600, 1200, &render::strip(&p, &cells, 0.3, 0.3 + wedge, p.max_radius, 0.0, 600, 1200)).unwrap();
        // A close-up at player scale (2 px per metre) halfway to the centre.
        {
            let (w, h) = (450u32, 300u32);
            let rc = 0.55 * p.radius;
            let (x0, y0) = ((0.45 * rc) as i64 - w as i64 / 2, (0.89 * rc) as i64 + h as i64 / 2);
            render::write_png(&dir.join("closeup.png").to_string_lossy(), w * 2, h * 2, &render::region(&p, x0, y0, w, h, 2)).unwrap();
        }
        // Other seeds.
        let size = 300u32;
        let mut img = vec![0u8; (size * 4 * size * 3) as usize];
        for k in 0..4u64 {
            let mut spec = p.spec.clone();
            spec.seed += (k + 1) * 977;
            let q = dft_sim::planet::Planet::new(spec, p.mats.clone()).unwrap();
            let w = render::whole(&q, size);
            for y in 0..size as usize {
                let dst = (y * size as usize * 4 + k as usize * size as usize) * 3;
                img[dst..dst + size as usize * 3].copy_from_slice(&w[y * size as usize * 3..(y + 1) * size as usize * 3]);
            }
        }
        render::write_png(&dir.join("seeds.png").to_string_lossy(), size * 4, size, &img).unwrap();

        // The sheet.
        let (fp, seen) = dft_sim::reference::fingerprint(&p);
        let total: u32 = seen.iter().filter(|(n, _)| n.as_str() != "vacuum").map(|(_, v)| v).sum();
        let mut s = String::new();
        let _ = writeln!(s, "# {}\n\nArchetype `{}`, seed {}, radius {:.1} km, fingerprint `{fp:016x}`{}.\n", p.spec.name, p.spec.archetype, p.spec.seed, p.radius / 1000.0, glamour.map_or(String::new(), |g| format!(", glamour {g}")));
        let _ = writeln!(s, "![disc](disc.png)\n\n![surface](surface.png)\n\n![dive](dive.png)\n\n![closeup](closeup.png)\n\n![seeds](seeds.png)\n");
        let _ = writeln!(s, "## Layers\n\n| Layer | From | To | Material | Temperature |\n|---|---|---|---|---|");
        let mut inner = 0.0;
        for l in &p.spec.layer {
            let _ = writeln!(s, "| {} | {:.0} km | {:.1} km | {} | {:.0} K |", l.name, inner * p.radius / 1000.0, l.outer * p.radius / 1000.0, l.material, l.temperature_k);
            inner = l.outer;
        }
        let _ = writeln!(s, "\n## Whole body (share of samples)\n\n| Material | Share |\n|---|---|");
        let mut v: Vec<_> = seen.iter().filter(|(n, _)| n.as_str() != "vacuum").collect();
        v.sort_by_key(|x| std::cmp::Reverse(*x.1));
        for (n, c) in v.iter().take(14) {
            let _ = writeln!(s, "| {n} | {:.2}% |", 100.0 * **c as f64 / total as f64);
        }
        let _ = writeln!(s, "\n## By depth below the surface\n");
        let depths: Vec<f64> = [1.0, 10.0, 50.0, 200.0, 1000.0, 5000.0].iter().copied().filter(|d| *d < p.radius * 0.9).collect();
        for (d, m) in dft_sim::reference::depth_profile(&p, &depths, 2000) {
            let mut v: Vec<_> = m.into_iter().filter(|(n, _)| n != "vacuum").collect();
            v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let top: Vec<String> = v.iter().take(6).map(|(n, x)| format!("{n} {x:.0}%")).collect();
            let _ = writeln!(s, "- **{d} m:** {}", top.join(", "));
        }
        let distinct = v.len();
        let t = timings.get(b).map(|t| t.as_str().map(String::from).unwrap_or_else(|| t.to_string())).unwrap_or_else(|| "not measured".into());
        let _ = writeln!(s, "\n## Cost\n\n10 minutes of play: {t}\n\n{distinct} distinct materials seen in {} samples.", seen.values().sum::<u32>());
        std::fs::write(dir.join("sheet.md"), s).unwrap();
        let _ = writeln!(index, "| [{}]({b}/sheet.md) | {:.1} km | `{fp:016x}` | {t} |", p.spec.name, p.radius / 1000.0);
        eprintln!("{b} done");
    }
    std::fs::write(out.join("README.md"), index).unwrap();
}
