//! tutorials: play the tutorial asteroids with the scripted idle player and
//! compare three ways of pacing the second one.
//!
//!   A  prestige: 5 km asteroid, with perks bought from asteroid 1's Seed Mass
//!   B  smaller:  3 km asteroid, no perks
//!   C  shallower: 5 km asteroid launching at 25% consumed, no perks
//!   D  same size: another 2 km asteroid, with perks
//!
//! Usage: tutorials [--seeds 3] [--limit-min 240] [--only ABCD] [--first 0] [--data data] [--out out]

use dft_sim::economy::Economy;
use dft_sim::planet::Planet;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn mins(s: f64) -> String {
    format!("{}:{:02}", (s / 60.0) as u64, (s % 60.0) as u64)
}

fn body(data: &Path, name: &str, seed_offset: u64) -> Planet {
    let p = dft_sim::load_body(data, name).unwrap();
    let mut spec = p.spec.clone();
    spec.seed += seed_offset;
    Planet::new(spec, p.mats.clone()).unwrap()
}

struct Run {
    ready: Option<f64>,
    launch: Option<f64>,
    peak: u64,
    buys: Vec<String>,
    seed_mass: f64,
    wall: f64,
}

fn play(data: &Path, name: &str, k: u64, eco: &Economy, perks: &[String], limit_min: f64) -> Run {
    let t = std::time::Instant::now();
    let mut sim = Sim::with_perks(World::new(body(data, name, k)), eco.clone(), perks);
    sim.run(limit_min * 60.0);
    Run {
        ready: sim.ready_at,
        launch: sim.launched_at,
        peak: sim.samples.iter().map(|s| s.harvesters as u64 + s.packet).max().unwrap_or(0),
        buys: sim.purchases.iter().map(|(t, p)| format!("{} {p}", mins(*t))).collect(),
        seed_mass: sim.seed_mass,
        wall: t.elapsed().as_secs_f64(),
    }
}

/// Spend Seed Mass on perks in the order they're listed, skipping any that don't fit.
fn buy_perks(eco: &Economy, mut seed_mass: f64) -> Vec<String> {
    let mut out = Vec::new();
    for p in &eco.perk {
        if seed_mass >= p.cost {
            seed_mass -= p.cost;
            out.push(p.name.clone());
        }
    }
    out
}

fn line(rep: &mut String, label: &str, r: &Run) {
    writeln!(
        rep,
        "  {label:<26} ready {:>6}  launch {:>6}  peak {:>6} drones  {:>2} purchases  Seed Mass {:>5.0}  ({:.1}s)",
        r.ready.map(mins).unwrap_or_else(|| "-".into()),
        r.launch.map(mins).unwrap_or_else(|| "none".into()),
        r.peak,
        r.buys.len(),
        r.seed_mass,
        r.wall
    )
    .unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut data, mut out, mut seeds, mut limit) = (PathBuf::from("data"), PathBuf::from("out"), 3u64, 240.0);
    let mut only = String::from("ABCD");
    let mut first = 0u64;
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--data" => data = PathBuf::from(v),
            "--out" => out = PathBuf::from(v),
            "--seeds" => seeds = v.parse().unwrap(),
            "--limit-min" => limit = v.parse().unwrap(),
            "--only" => only = v,
            "--first" => first = v.parse().unwrap(),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    std::fs::create_dir_all(&out).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut shallow = eco.clone();
    shallow.launch.threshold = 0.25;
    let mut rep = String::new();
    writeln!(rep, "Tutorial asteroids, idle player: checks in every {:.0} min, buys up to {} things, launches at the next check-in after it's allowed.", eco.player.check_in_every_s / 60.0, eco.player.buys_per_check_in).unwrap();
    writeln!(rep, "Targets: asteroid 1 about 60 min, asteroid 2 about 30 min, under 10,000 drones, a few purchases.\n").unwrap();
    for k in first..first + seeds {
        writeln!(rep, "Seed set {}", k + 1).unwrap();
        let a1 = play(&data, "tutorial-1-2km", k, &eco, &[], limit);
        line(&mut rep, "Asteroid 1 (2 km)", &a1);
        for b in &a1.buys {
            writeln!(rep, "      {b}").unwrap();
        }
        let perks = buy_perks(&eco, a1.seed_mass);
        writeln!(rep, "      perks bought with its Seed Mass: {}", if perks.is_empty() { "none".into() } else { perks.join(", ") }).unwrap();
        if only.contains('A') {
            let a = play(&data, "rubble-asteroid", k, &eco, &perks, limit);
            line(&mut rep, "A: 5 km with perks", &a);
        }
        if only.contains('B') {
            let b = play(&data, "tutorial-2-3km", k, &eco, &[], limit);
            line(&mut rep, "B: 3 km, no perks", &b);
        }
        if only.contains('C') {
            let c = play(&data, "rubble-asteroid", k, &shallow, &[], limit);
            line(&mut rep, "C: 5 km, launch at 25%", &c);
        }
        if only.contains('D') {
            let d = play(&data, "tutorial-1-2km", k + 100, &eco, &perks, limit);
            line(&mut rep, "D: another 2 km, perks", &d);
            for b in &d.buys {
                writeln!(rep, "      {b}").unwrap();
            }
        }
        writeln!(rep).unwrap();
    }
    print!("{rep}");
    std::fs::write(out.join("tutorials-report.txt"), rep).unwrap();
}
