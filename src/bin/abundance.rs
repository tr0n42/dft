//! Element abundance of every body: how much of each element (ppm by mass)
//! there is in the whole body and in its outer skin, by sampling the
//! generator. Writes data/reference/abundance.csv and prints a comparison
//! with Earth's crust (data/reference/crust-ppm.toml).
//!
//! Run: cargo run --release --bin abundance [-- body ...] [--samples N] [--skin-m M]
use dft_sim::materials::MatId;
use dft_sim::noise::{cos_sin, hash, unit};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// ppm of each element (by table index) in a zone: `depth` picks the skin
/// (Some(metres)) or the whole body (None).
fn sample(p: &dft_sim::planet::Planet, n: u64, skin: Option<f64>) -> (Vec<f64>, Vec<f64>) {
    let els = &p.mats.elements;
    let ne_only = els.list.len();
    // Elements first, then each material's own mass.
    let ne = ne_only + p.mats.len();
    // Each material's element fractions, by index, once.
    let comp: Vec<Vec<(usize, f64)>> = (0..p.mats.len()).map(|m| p.mats.get(m as MatId).composition.iter().filter_map(|(s, f)| els.index(s).map(|i| (i, *f))).collect()).collect();
    let dens: Vec<f64> = (0..p.mats.len()).map(|m| p.mats.get(m as MatId).density).collect();
    let seed = p.spec.seed ^ 0xAB0;
    let r_max = p.max_radius;
    let kg = (0..n)
        .into_par_iter()
        .fold(
            || (vec![0.0f64; ne], Vec::with_capacity(4)),
            |(mut kg, mut odds), i| {
                let a = unit(hash(seed, i as i64, 0, 1)) * std::f64::consts::TAU;
                let (c, s) = cos_sin(a);
                let (x, y) = match skin {
                    None => {
                        let r = r_max * unit(hash(seed, i as i64, 1, 1)).sqrt();
                        (c * r, s * r)
                    }
                    Some(d) => {
                        let top = p.surface_radius(c, s);
                        let d = d.min(top);
                        // Area-uniform between top - d and top.
                        let (r0, r1) = (top - d, top);
                        let r = (r0 * r0 + (r1 * r1 - r0 * r0) * unit(hash(seed, i as i64, 2, 1))).sqrt();
                        (c * r, s * r)
                    }
                };
                p.material_odds(x.floor() as i64, y.floor() as i64, &mut odds);
                for &(m, w) in &odds {
                    let k = w * dens[m as usize];
                    for &(e, f) in &comp[m as usize] {
                        kg[e] += k * f;
                    }
                    kg[ne_only + m as usize] += k;
                }
                (kg, odds)
            },
        )
        .map(|(kg, _)| kg)
        .reduce(|| vec![0.0; ne], |mut a, b| {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
            a
        });
    let total: f64 = kg[..ne_only].iter().sum::<f64>().max(1e-30);
    let ppm: Vec<f64> = kg.iter().map(|k| k / total * 1e6).collect();
    (ppm[..ne_only].to_vec(), ppm[ne_only..].to_vec())
}

fn main() -> Result<(), String> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut n: u64 = 4_000_000;
    let mut skin_m = 500.0;
    let mut odds_seeds: u64 = 0;
    // --plain: no specialties, the generator's baseline.
    let mut plain = false;
    let mut names = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--samples" => {
                n = args[i + 1].parse().map_err(|_| "--samples takes a number")?;
                i += 1;
            }
            "--plain" => plain = true,
            "--odds" => {
                odds_seeds = args[i + 1].parse().map_err(|_| "--odds takes a number of seeds")?;
                i += 1;
            }
            "--skin-m" => {
                skin_m = args[i + 1].parse().map_err(|_| "--skin-m takes metres")?;
                i += 1;
            }
            b => names.push(b.to_string()),
        }
        i += 1;
    }
    let write = names.is_empty();
    if names.is_empty() {
        names = std::fs::read_dir(data.join("bodies")).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).map(|p| p.file_stem().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
    }
    if odds_seeds > 0 {
        // How often each specialty comes up on each kind of body, over many seeds.
        let mut out = String::from("body,specialty,share_of_seeds\n");
        for name in &names {
            let mut spec = dft_sim::spec::BodySpec::load(&data.join("bodies").join(format!("{name}.toml")).to_string_lossy())?;
            if spec.radius_m > 200_000.0 {
                continue;
            }
            let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy())?;
            let mut count: BTreeMap<String, u64> = BTreeMap::new();
            let mut none = 0;
            for s in 0..odds_seeds {
                spec.seed = 1_000_003 * (s + 1);
                let p = dft_sim::planet::Planet::new(spec.clone(), mats.clone())?;
                if p.specialties.is_empty() {
                    none += 1;
                }
                for r in &p.specialties {
                    *count.entry(r.name.clone()).or_default() += 1;
                }
            }
            println!("{name}: {:.0}% with none", none as f64 / odds_seeds as f64 * 100.0);
            for (k, v) in &count {
                println!("  {k:32} {:5.1}%", *v as f64 / odds_seeds as f64 * 100.0);
                writeln!(out, "{name},{k},{:.4}", *v as f64 / odds_seeds as f64).unwrap();
            }
        }
        std::fs::write(data.join("reference/specialty-odds.csv"), out).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let crust: BTreeMap<String, f64> = toml::from_str(&std::fs::read_to_string(data.join("reference/crust-ppm.toml")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut mat_csv = String::from("body,material,whole_ppm,skin_ppm\n");
    let mut spec_csv = String::from("body,specialty,jackpot,effects\n");
    let mut csv = String::from("body,radius_km,element,whole_ppm,skin_ppm,grade\n");
    let mut syms = Vec::new();
    let mut rows: Vec<(String, Vec<f64>, Vec<f64>, Vec<bool>)> = Vec::new();
    for name in &names {
        let p = {
            let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy())?;
            let mut spec = dft_sim::spec::BodySpec::load(&data.join("bodies").join(format!("{name}.toml")).to_string_lossy())?;
            spec.bias.off = plain;
            dft_sim::planet::Planet::new(spec, mats)?
        };
        let (whole, whole_m) = sample(&p, n, None);
        let (skin, skin_m_ppm) = sample(&p, n, Some(skin_m));
        for (m, (w, sk)) in whole_m.iter().zip(&skin_m_ppm).enumerate() {
            if *w > 0.0 || *sk > 0.0 {
                writeln!(mat_csv, "{name},{},{:.6e},{:.6e}", p.mats.get(m as MatId).name, w, sk).unwrap();
            }
        }
        for r in &p.specialties {
            eprintln!("{name}: {}{} — {}", if r.jackpot { "JACKPOT " } else { "" }, r.name, r.effects.join("; "));
            writeln!(spec_csv, "{name},{},{},\"{}\"", r.name, r.jackpot, r.effects.join("; ")).unwrap();
        }
        let grade = p.element_grades();
        let can: Vec<bool> = grade.iter().map(|g| *g > 0.0).collect();
        syms = p.mats.elements.list.iter().map(|e| e.symbol.clone()).collect();
        for (k, s) in syms.iter().enumerate() {
            writeln!(csv, "{name},{:.1},{s},{:.6e},{:.6e},{:.6e}", p.radius / 1000.0, whole[k], skin[k], grade[k]).unwrap();
        }
        eprintln!("{name}: done");
        rows.push((name.clone(), whole, skin, can));
    }
    if plain {
        std::fs::write(data.join("reference/abundance-plain.csv"), csv).map_err(|e| e.to_string())?;
        std::fs::write(data.join("reference/materials-plain.csv"), mat_csv).map_err(|e| e.to_string())?;
    } else if write {
        std::fs::write(data.join("reference/abundance.csv"), csv).map_err(|e| e.to_string())?;
        std::fs::write(data.join("reference/specialties.csv"), spec_csv).map_err(|e| e.to_string())?;
    }
    // The summary: per element, Earth's crust, the average over bodies (each
    // body counted once), and how many bodies have any.
    println!("{:<3} {:>12} {:>12} {:>12} {:>7}  richest", "el", "crust ppm", "avg whole", "avg skin", "bodies");
    for (k, s) in syms.iter().enumerate() {
        let nb = rows.len() as f64;
        let w: f64 = rows.iter().map(|r| r.1[k]).sum::<f64>() / nb;
        let sk: f64 = rows.iter().map(|r| r.2[k]).sum::<f64>() / nb;
        let on = rows.iter().filter(|r| r.3[k]).count();
        let best = rows.iter().max_by(|a, b| a.1[k].total_cmp(&b.1[k])).map(|r| format!("{} {:.3e}", r.0, r.1[k])).unwrap_or_default();
        println!("{s:<3} {:>12.3e} {:>12.3e} {:>12.3e} {:>4}/{:<2}  {best}", crust.get(s).copied().unwrap_or(0.0), w, sk, on, rows.len());
    }
    Ok(())
}
