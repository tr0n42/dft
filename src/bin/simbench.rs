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
        s.grab_adrift = std::env::var("NOGRAB").is_err();
        s.sandbox.far_settle = std::env::var("NOSETTLE").is_err();
        // As the game zoomed out: loose matter moves nowhere ("all": everywhere).
        if args.get(4).map(|a| a.as_str()) != Some("all") {
            s.set_attention(Some(None));
            if std::env::var("NOFOCUS").is_err() {
                s.set_focus(Some(None));
            }
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
        // LOOK=n: then zoom in on landing point n (as the game would) and
        // run on 3 seconds: its cutters should be caught up.
        if let Some(n) = std::env::var("LOOK").ok().and_then(|v| v.parse::<usize>().ok()) {
            let mut hp = Vec::new();
            s.hub_points(&mut hp);
            if let Some(h) = hp.chunks(3).filter(|c| c[2] == 0.0).nth(n) {
                let (hx, hy) = (h[0] as i64, h[1] as i64);
                let r = [hx - 300, hx + 300, hy - 200, hy + 200];
                s.set_attention(Some(Some(r)));
                s.set_focus(Some(Some([r[0] - 300, r[1] + 300, r[2] - 200, r[3] + 200])));
                let before = s.caught;
                s.run(until + 3.0);
                println!("    looked at landing point {n}: {} cutters caught up (came home from a trip worked out at once)", s.caught - before);
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
        for l in s.fleet_lines() {
            println!("    {l}");
        }
        // MAP=n: the ground round landing point n as text (# rock, . open).
        if let Some(n) = std::env::var("MAP").ok().and_then(|v| v.parse::<usize>().ok()) {
            let mut hp = Vec::new();
            s.hub_points(&mut hp);
            if let Some(h) = hp.chunks(3).filter(|c| c[2] == 0.0).nth(n) {
                let (hx, hy) = (h[0] as i64, h[1] as i64);
                println!("    ground round landing point {n} at ({hx}, {hy}), x across, y up:");
                for y in (hy - 70..hy + 30).rev() {
                    let row: String = (hx - 80..hx + 80).map(|x| if s.material(x, y) == dft_sim::materials::VACUUM { '.' } else { '#' }).collect();
                    println!("    {row}");
                }
            }
        }
        // Speckle: rock cells with three or four open sides, round every
        // landing point (a sieve-eaten face shows many).
        {
            let mut hp = Vec::new();
            s.hub_points(&mut hp);
            let (mut speck, mut solid) = (0u64, 0u64);
            let mut seen = std::collections::HashSet::new();
            for h in hp.chunks(3).filter(|c| c[2] == 0.0) {
                let (hx, hy) = (h[0] as i64, h[1] as i64);
                for y in hy - 200..hy + 200 {
                    for x in hx - 200..hx + 200 {
                        if !seen.insert((x, y)) || s.material(x, y) == dft_sim::materials::VACUUM {
                            continue;
                        }
                        solid += 1;
                        let open = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|d| s.material(x + d.0, y + d.1) == dft_sim::materials::VACUUM).count();
                        if open >= 3 {
                            speck += 1;
                        }
                    }
                }
            }
            println!("    speckle: {speck} rock cells with 3+ open sides of {solid} round the landing points");
        }
        let vented: f64 = s.world.ledger.vented.iter().sum();
        println!("    bodies in the air {} · lost to space (vented) {:.0} t", s.bodies.count(), vented / 1000.0);
        println!("    sandbox: {} cells dormant (waiting to be seen) · {} settled out of sight · {} moves in all", s.sandbox.dormant(), s.sandbox.stats.far_moves, s.sandbox.stats.moves);
        println!("    {}", STAGES.iter().zip(s.prof).map(|(n, v)| format!("{n} {:.2}", per(v) )).collect::<Vec<_>>().join(" · "));
    }
}
