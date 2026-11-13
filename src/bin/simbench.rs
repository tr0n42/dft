//! Times the simulation alone (no drawing): a body run to a game time as fast
//! as it goes, with features switched off one at a time, so a slowdown can be
//! pinned on what causes it. Prints real seconds and the sim's own stage
//! timings (ms per game second).
//!
//! Run: cargo run --release --bin simbench -- [body] [radius_km] [game_s]
//! (default living-world 5 1800)

use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;
use std::time::Instant;

const STAGES: [&str; 8] = ["drone events", "sand and heat", "rock bodies", "per-second work", "events: work search", "per-second: building", "per-second: packet etc", "per-second: orbit"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let body = args.first().cloned().unwrap_or("living-world".into());
    let km: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5.0);
    let until: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1800.0);
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let planet = || {
        let text = std::fs::read_to_string(data.join("bodies").join(format!("{body}.toml"))).unwrap();
        let text = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
        let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
        // Life on, as in the game (NOLIFE=1: off).
        if std::env::var("NOLIFE").is_ok() {
            spec.life = None;
        } else if spec.life.is_none() {
            spec.life = Some(1.0);
        }
        let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
        dft_sim::planet::Planet::new(spec, mats).unwrap()
    };
    let eco = || Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let no_bay = |e: &mut Economy, u: Unit| e.units.bay.retain(|b| b.unit != u);
    let cases: Vec<(&str, Box<dyn Fn(&mut Economy)>)> = vec![
        ("as it is", Box::new(|_| {})),
        ("no sappers", Box::new(move |e| no_bay(e, Unit::Sapper))),
        ("no foresters", Box::new(move |e| no_bay(e, Unit::Forester))),
        ("no heat-sink bots", Box::new(move |e| no_bay(e, Unit::HeatSink))),
        ("no tunnelers", Box::new(move |e| no_bay(e, Unit::Tunneler))),
        ("no replicator at all", Box::new(|e| e.units.universal_replicator = false)),
    ];
    let only: Option<String> = args.get(3).cloned();
    println!("{body} {km} km, to {until:.0} game s");
    for (name, f) in cases {
        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }
        let mut e = eco();
        f(&mut e);
        let mut s = Sim::new(World::new(planet()), e);
        // As the game zoomed out: loose matter moves nowhere ("all": everywhere).
        if args.get(4).map(|a| a.as_str()) != Some("all") {
            s.set_attention(Some(None));
        }
        let t0 = Instant::now();
        let verbose = std::env::var("VERBOSE").is_ok();
        let mut t = 0.0;
        let mut p0 = s.prof;
        let mut f0 = s.prof2;
        while t < until {
            t = (t + 120.0).min(until);
            let w = Instant::now();
            s.run(t);
            if verbose {
                let (st, q) = s.sand_load();
                let d: Vec<String> = (0..8).map(|i| format!("{:.2}", s.prof[i] - p0[i])).collect();
                p0 = s.prof;
                let f: Vec<String> = (0..12).map(|i| format!("{:.2}", s.prof2[i] - f0[i])).collect();
                f0 = s.prof2;
                println!("    t {t:5.0}: finer s [big {}, free {}, step {}, landed {}, sand {}, apply {}, blast tick {}, of it sapper_blast {}, scan_flora {}, walkers {}, life {}, specialists {}]", f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11]);
                println!("    t {t:5.0}: stages s [{}] · loose, settling {:?} · drones {}", d.join(", "), s.loose_count(), s.harvesters_alive());
                println!(
                    "    t {t:5.0}: {:5.2} s real · queues next {} warm {} slow {} far {} · hot cells {} · stepped {} burning {} burned {} moves {} reactions {} · blasts {}",
                    w.elapsed().as_secs_f64(), q[0], q[1], q[2], q[3], q[4], st.cells_stepped, st.burning, st.burned, st.moves, st.reactions, s.specialist_totals().0
                );
            }
        }
        let real = t0.elapsed().as_secs_f64();
        let per = |v: f64| v / until * 1000.0;
        let (blasts, _, frozen, plants, fires) = s.specialist_totals();
        println!(
            "{name:22} {real:6.1} s real · {} drones · blasts {blasts}, frozen {:.0} t, plants {:.0} t, fires {fires}",
            s.harvesters_alive(),
            frozen / 1000.0,
            plants / 1000.0
        );
        println!("    {}", STAGES.iter().zip(s.prof).map(|(n, v)| format!("{n} {:.2}", per(v) )).collect::<Vec<_>>().join(" · "));
    }
}
