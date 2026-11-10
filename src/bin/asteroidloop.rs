//! asteroidloop: play a body (default: tutorial asteroid 1) to launch with the scripted player,
//! headless, and report how the run went.
//!
//! Usage: asteroidloop [--body tutorial-1-2km] [--seeds 5] [--limit-min 180] [--data data] [--out out]
//! Writes out/asteroid-run-<seed>.csv (a sample every 10 s of game time),
//! out/asteroid-loop-report.txt and a picture of the first run at launch.

use dft_sim::economy::Economy;
use dft_sim::render;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

fn mins(s: f64) -> String {
    format!("{}:{:02}", (s / 60.0) as u64, (s % 60.0) as u64)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut data = PathBuf::from("data");
    let mut out = PathBuf::from("out");
    let mut seeds = 5u64;
    let mut limit_min = 180.0;
    let mut body_name = String::from("tutorial-1-2km");
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--data" => data = PathBuf::from(v),
            "--out" => out = PathBuf::from(v),
            "--seeds" => seeds = v.parse().unwrap(),
            "--limit-min" => limit_min = v.parse().unwrap(),
            "--body" => body_name = v,
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    std::fs::create_dir_all(&out).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut rep = String::new();
    writeln!(rep, "Asteroid loop: {body_name}, scripted idle player").unwrap();
    writeln!(rep, "launch at {:.0}% of the body consumed; time limit {limit_min:.0} min of game time\n", eco.launch.threshold * 100.0).unwrap();
    writeln!(rep, "{:<6} {:>9} {:>9} {:>9} {:>8} {:>7} {:>8} {:>10} {:>9}", "seed", "launch", "peak", "built", "deaths", "scouts", "refiners", "Seed Mass", "run time").unwrap();
    for k in 0..seeds {
        let mut planet = dft_sim::load_body(&data, &body_name).unwrap();
        let seed = planet.spec.seed + k;
        let mut spec = planet.spec.clone();
        spec.seed = seed;
        planet = dft_sim::planet::Planet::new(spec, planet.mats.clone()).unwrap();
        let t = Instant::now();
        let mut sim = Sim::new(World::new(planet), eco.clone());
        sim.run(limit_min * 60.0);
        let wall = t.elapsed().as_secs_f64();
        let peak = sim.samples.iter().map(|s| s.harvesters as u64 + s.packet).max().unwrap_or(0);
        let last = sim.samples.last().cloned().unwrap_or_default();
        writeln!(
            rep,
            "{:<6} {:>9} {:>9} {:>9} {:>8} {:>7} {:>8} {:>10.0} {:>8.1}s",
            seed,
            sim.launched_at.map(mins).unwrap_or_else(|| "none".into()),
            peak,
            sim.harvesters_built,
            sim.deaths,
            last.scouts,
            last.refiners,
            sim.seed_mass,
            wall
        )
        .unwrap();
        let mut csv = String::from("t_s,harvesters,scouts,refiners,on_trips,energy,mass_available_kg,credited_total_kg,consumed,mined_kg_per_s,avg_haul_m,hubs,brownout_share\n");
        for s in &sim.samples {
            writeln!(
                csv,
                "{:.0},{},{},{},{},{:.1},{:.0},{:.0},{:.5},{:.1},{:.1},{},{:.3}",
                s.t, s.harvesters, s.scouts, s.refiners, s.on_trips, s.energy, s.mass_available, s.credited_total, s.consumed, s.mass_rate, s.avg_haul_m, s.hubs, s.brownout_share
            )
            .unwrap();
        }
        std::fs::write(out.join(format!("asteroid-run-{seed}.csv")), csv).unwrap();
        if k == 0 {
            let first_hash = sim.state_hash();
            let imb = sim.world.ledger.imbalance(&sim.world.remaining_kg());
            let initial: f64 = sim.world.ledger.initial.iter().sum();
            writeln!(rep, "\nFirst run in detail (seed {seed}):").unwrap();
            writeln!(rep, "  books balance: {} (largest mismatch {imb:.2e} kg)", if imb < 1e-6 * initial { "yes" } else { "NO" }).unwrap();
            writeln!(rep, "  Mass delivered {:.0} t, spent {:.0} t, recovered from worn-out drones {:.0} t", sim.delivered_credited / 1e3, sim.spent_mass / 1e3, sim.recovered_mass / 1e3).unwrap();
            writeln!(rep, "  purchases: {} in all", sim.purchases.len()).unwrap();
            for (t, p) in sim.purchases.iter().filter(|(_, p)| p.starts_with("unlock")) {
                writeln!(rep, "    {:>6}  {p}", mins(*t)).unwrap();
            }
            for (i, u) in sim.eco.upgrade.iter().enumerate() {
                let lvl = sim.levels[i];
                writeln!(rep, "    {:<17} level {lvl:>3} at launch (x{:.1})", u.name, u.effect.powi(lvl as i32)).unwrap();
            }
            writeln!(rep, "  milestones:").unwrap();
            for target in [0.01, 0.05, 0.1, 0.25, 0.5] {
                if let Some(s) = sim.samples.iter().find(|s| s.consumed >= target) {
                    writeln!(rep, "    {:>4.0}% consumed at {:>6}, {} Harvesters", target * 100.0, mins(s.t), s.harvesters as u64 + s.packet).unwrap();
                }
            }
            writeln!(rep, "  peel score {:.0}% (share of what's left outside the ideal circle; 0% = eaten evenly from all sides)", sim.peel_score(4) * 100.0).unwrap();
            writeln!(rep, "  world and swarm hash: {first_hash:016x}").unwrap();
            let cells = |x: i64, y: i64| sim.world.material(x, y);
            let img = render::whole_with(&sim.world.planet, &cells, 1024);
            render::write_png(&out.join("asteroid-at-launch.png").to_string_lossy(), 1024, 1024, &img).unwrap();
        }
    }
    print!("{rep}");
    std::fs::write(out.join("asteroid-loop-report.txt"), rep).unwrap();
}
