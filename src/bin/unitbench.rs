//! unitbench: "500k cutters with X". A fixed big-swarm run, with the unit
//! roster growing stage by stage as each unit is designed:
//!
//!   1  cutters + haulers
//!   2  + sappers
//!   3  + tunnelers
//!   4  + heat-sink bots
//!   5  + foresters and abductors
//!
//! It times each unit's own code (src/units/*.rs) and the platform's parts
//! separately, in ms of real time per second of game time, and appends one
//! row per stage to perf/history.csv (checked in), with the machine, the
//! commit, and the game's fingerprint. Each run prints the change against the
//! last row of the same stage on the same machine:
//!   - fingerprint the same: only speed changed (a pure optimisation);
//!   - fingerprint changed: behaviour changed too (expected when a unit is
//!     being reworked).
//!
//! Run: cargo run --release --bin unitbench -- [stages] [game_s]
//!   stages: "1" or "1-3" (default: 1 up to the stage in perf/stage.txt)
//!   game_s: game seconds per stage (default 1200)
//! Environment: BODY (default living-world), KM (default 35), DRONES
//! (default 500000), NOTE (a word for the history row), NOSAVE=1 (don't
//! write the history).

use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::{Sim, UNIT_NAMES};
use dft_sim::world::World;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

const STAGES: [&str; 5] = ["cutters + haulers", "+ sappers", "+ tunnelers", "+ heat-sink bots", "+ foresters, abductors"];

/// The specialists out at once in each stage: the economy's own `keep` for
/// each, as the director would order them.
fn roster(stage: usize, e: &Economy) -> Vec<(Unit, u32)> {
    let mut r = Vec::new();
    if stage >= 2 {
        r.push((Unit::Sapper, e.units.sapper.keep));
    }
    if stage >= 3 {
        r.push((Unit::Tunneler, e.units.tunneler.keep.max(1)));
    }
    if stage >= 4 {
        r.push((Unit::HeatSink, e.units.heatsink.keep));
    }
    if stage >= 5 {
        r.push((Unit::Forester, e.units.forester.keep));
        r.push((Unit::Abductor, e.units.abductor.keep));
    }
    r
}

/// Columns: each unit, then the platform's parts.
fn columns() -> Vec<String> {
    let mut c: Vec<String> = UNIT_NAMES.iter().map(|s| s.to_string()).collect();
    c.extend(["sand and heat", "rock bodies", "life", "platform other", "animals"].iter().map(|s| s.to_string()));
    c
}

/// ms of real time per game second in each column, since the last call.
fn costs(s: &mut Sim, game_s: f64) -> Vec<f64> {
    let total: f64 = s.prof[..4].iter().sum();
    let sand = s.prof2[4] + s.prof2[5];
    let bodies = s.prof[2];
    let life = s.prof2[10];
    let units: f64 = s.unit_prof.iter().sum();
    let animals = s.fauna_prof;
    let mut v: Vec<f64> = s.unit_prof.to_vec();
    v.push(sand);
    v.push(bodies);
    v.push(life);
    v.push(total - units - sand - bodies - life - animals);
    v.push(animals);
    s.fauna_prof = 0.0;
    s.prof = [0.0; 8];
    s.prof2 = [0.0; 12];
    s.unit_prof = [0.0; 7];
    v.iter().map(|x| x / game_s.max(1e-9) * 1000.0).collect()
}

fn git(args: &[&str]) -> String {
    std::process::Command::new("git").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("data");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let top: usize = std::fs::read_to_string(root.join("perf").join("stage.txt")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(1);
    let (a, b) = match args.first() {
        Some(s) if s.contains('-') => {
            let mut p = s.split('-').map(|x| x.parse::<usize>().unwrap_or(1));
            (p.next().unwrap_or(1), p.next().unwrap_or(top))
        }
        Some(s) => {
            let n = s.parse().unwrap_or(top);
            (n, n)
        }
        None => (1, top),
    };
    let until: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1200.0);
    let body = std::env::var("BODY").unwrap_or("living-world".into());
    let km: f64 = std::env::var("KM").ok().and_then(|s| s.parse().ok()).unwrap_or(35.0);
    let drones: u64 = std::env::var("DRONES").ok().and_then(|s| s.parse().ok()).unwrap_or(500_000);
    let machine = std::env::var("COMPUTERNAME").unwrap_or("?".into());
    let commit = {
        let c = git(&["rev-parse", "--short", "HEAD"]);
        let dirty = !git(&["status", "--porcelain", "--", "src", "data"]).is_empty();
        if dirty { format!("{c}+changes") } else { c }
    };
    let note = std::env::var("NOTE").unwrap_or_default().replace(',', ";");
    let cols = columns();
    let hist = root.join("perf").join("history.csv");
    let old = std::fs::read_to_string(&hist).unwrap_or_default();
    println!("500k cutters with X: {body} {km} km, {drones} drones, {until:.0} game s a stage, on {machine}, commit {commit}");

    for stage in a.max(1)..=b.min(STAGES.len()) {
        let text = std::fs::read_to_string(data.join("bodies").join(format!("{body}.toml"))).unwrap();
        let text = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
        let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
        if spec.life.is_none() {
            spec.life = Some(1.0);
        }
        let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
        let planet = dft_sim::planet::Planet::new(spec, mats).unwrap();
        let mut e = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
        e.seed_swarm.start_harvesters = drones as u32;
        let team = roster(stage, &e);
        // Only this stage's units exist: no bays (and so no scans for work)
        // for the others.
        e.units.bay.retain(|b| matches!(b.unit, Unit::Cutter | Unit::Hauler) || team.iter().any(|(u, _)| *u == b.unit));
        let mut s = Sim::new(World::new(planet), e);
        s.free_drones = Some(drones);
        s.free_units = team.clone();
        // Zoomed out, as the player mostly is.
        s.set_attention(Some(None));
        let names: Vec<String> = team.iter().map(|(u, n)| format!("{n} {}", u.name())).collect();
        println!("\nstage {stage}: {} ({})", STAGES[stage - 1], if names.is_empty() { "no specialists".into() } else { names.join(", ") });
        let t0 = Instant::now();
        let mut sum = vec![0.0; cols.len()];
        let mut t = 0.0;
        let every = 200.0f64.min(until);
        while t < until {
            let w = Instant::now();
            let from = t;
            t = (t + every).min(until);
            s.run(t);
            let c = costs(&mut s, t - from);
            for (k, v) in c.iter().enumerate() {
                sum[k] += v * (t - from);
            }
            let (_, waiting, out, home) = s.phase_counts();
            let top: Vec<String> = cols.iter().zip(&c).filter(|(_, v)| **v >= 0.5).map(|(n, v)| format!("{n} {v:.0}")).collect();
            println!(
                "  t {t:5.0}: {:6.1} s per game min · {:6} working · ms per game s: {}",
                w.elapsed().as_secs_f64() / ((t - from) / 60.0),
                waiting + out + home,
                top.join(", ")
            );
        }
        let real = t0.elapsed().as_secs_f64();
        let avg: Vec<f64> = sum.iter().map(|v| v / until).collect();
        let hash = format!("{:016x}", s.state_hash());
        println!("  done: {real:.1} s real for {until:.0} game s; fingerprint {hash}");

        // Against the last run of this stage on this machine.
        let header = format!("date,machine,commit,stage,body,km,drones,game_s,real_s,fingerprint,note,{}", cols.join(","));
        let prev = old.lines().filter(|l| {
            let f: Vec<&str> = l.split(',').collect();
            f.len() > 10 && f[1] == machine && f[3] == stage.to_string() && f[4] == body && f[5] == format!("{km}") && f[6] == drones.to_string() && f[7] == format!("{until:.0}")
        }).last();
        if let Some(p) = prev {
            let f: Vec<&str> = p.split(',').collect();
            let pr: f64 = f[8].parse().unwrap_or(0.0);
            println!("  against {} ({}): real {:.1} s -> {:.1} s ({:+.0}%), {}", f[0], f[2], pr, real, (real / pr.max(1e-9) - 1.0) * 100.0, if f[9] == hash { "same fingerprint: speed only" } else { "fingerprint changed: behaviour changed" });
            for (k, n) in cols.iter().enumerate() {
                let was: f64 = f.get(11 + k).and_then(|x| x.parse().ok()).unwrap_or(0.0);
                if was >= 0.5 || avg[k] >= 0.5 {
                    println!("    {n:16} {was:8.1} -> {:8.1} ms per game s ({:+.1})", avg[k], avg[k] - was);
                }
            }
        } else {
            println!("  (first run of this stage here)");
            for (k, n) in cols.iter().enumerate() {
                if avg[k] >= 0.05 {
                    println!("    {n:16} {:8.1} ms per game s", avg[k]);
                }
            }
        }
        if std::env::var("NOSAVE").is_err() {
            std::fs::create_dir_all(root.join("perf")).unwrap();
            let fresh = !hist.exists();
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&hist).unwrap();
            if fresh {
                writeln!(f, "{header}").unwrap();
            }
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let vals: Vec<String> = avg.iter().map(|v| format!("{v:.2}")).collect();
            writeln!(f, "{},{machine},{commit},{stage},{body},{km},{drones},{until:.0},{real:.1},{hash},{note},{}", day(now), vals.join(",")).unwrap();
        }
    }
}

/// YYYY-MM-DD HH:MM (UTC) from Unix seconds.
fn day(secs: u64) -> String {
    let d = (secs / 86400) as i64;
    let (hh, mm) = ((secs % 86400) / 3600, (secs % 3600) / 60);
    // Civil from days (Howard Hinnant's algorithm).
    let z = d + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{}-{:02}-{:02} {:02}:{:02}", if m <= 2 { y + 1 } else { y }, m, dd, hh, mm)
}
