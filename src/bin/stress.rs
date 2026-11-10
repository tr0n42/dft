//! stress: push a body and a swarm past their usual sizes and watch the cost.
//!
//! cargo run --release --bin stress -- <body>
//! Settings (environment):
//!   RADIUS_KM=r   resize the body (layers scale with it; surface features keep their size)
//!   GLAMOUR=g     the glamour pass at this setting
//!   START=n       start with n harvesters already built
//!   CAP=n         simulate up to n drones one by one (the rest as a packet)
//!   PLAY_S=t      game seconds to play (default 600)
//!   EVERY_S=t     report every t game seconds (default 60)
//!   WALL_S=t      stop early after t seconds of real time (default 900)
//! Prints one line per report: game time, real time, real seconds per game
//! minute over the last stretch, drones, awake cells, dig face, memory.
use std::path::Path;
use std::time::Instant;

fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS")).map(|l| l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0))
        .unwrap_or(0.0)
}

fn env<T: std::str::FromStr>(k: &str) -> Option<T> {
    std::env::var(k).ok().and_then(|v| v.parse().ok())
}

fn main() {
    let name = std::env::args().nth(1).expect("body name");
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let t0 = Instant::now();
    let base = dft_sim::load_body(&data, &name).unwrap();
    let mut spec = base.spec.clone();
    if let Some(r) = env::<f64>("RADIUS_KM") {
        spec.radius_m = r * 1000.0;
    }
    if let Some(g) = env::<f64>("GLAMOUR") {
        spec.glamour = Some(g);
    }
    // LIFE=x: flora (0 to 2; the game uses 1).
    if let Some(l) = env::<f64>("LIFE") {
        spec.life = Some(l);
    }
    let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
    let radius = p.radius;
    let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let w = dft_sim::world::World::new(p);
    let mut eco = dft_sim::economy::Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    if let Some(n) = env::<u32>("START") {
        eco.seed_swarm.start_harvesters = n;
    }
    if let Some(n) = env::<u32>("CAP") {
        eco.individual_cap = n;
    }
    let mut s = dft_sim::swarm::Sim::new(w, eco);
    // FREE=n: economy off, the swarm grows to n drones (the game's test mode).
    s.free_drones = env::<u64>("FREE");
    // ATTENTION=x0,x1,y0,y1 (cells), or "none": where the sandbox moves
    // things, as the game sets it to what's on screen. Unset: everywhere.
    if let Ok(a) = std::env::var("ATTENTION") {
        let v: Vec<i64> = a.split(',').filter_map(|x| x.parse().ok()).collect();
        s.set_attention(Some(if v.len() == 4 { Some([v[0], v[1], v[2], v[3]]) } else { None }));
    }
    println!("{name}: radius {:.1} km, load {load_ms:.0} ms, set-up {:.0} ms, memory {:.0} MB, {} drones to start", radius / 1000.0, t0.elapsed().as_secs_f64() * 1000.0 - load_ms, rss_mb(), s.drone_count());
    let play: f64 = env("PLAY_S").unwrap_or(600.0);
    let every: f64 = env("EVERY_S").unwrap_or(60.0);
    let wall: f64 = env("WALL_S").unwrap_or(900.0);
    let start = Instant::now();
    let (mut last_t, mut last_w) = (0.0, 0.0);
    let mut next = every;
    // BOOST=n BOOST_AT=seconds: raise the free swarm to n then (as a player
    // turning the count up mid-game).
    let boost: Option<(u64, f64)> = env::<u64>("BOOST").map(|n| (n, env::<f64>("BOOST_AT").unwrap_or(0.0)));
    while s.time() < play && s.launched_at.is_none() {
        if let Some((n, at)) = boost {
            if s.time() >= at && s.free_drones != Some(n) {
                s.free_drones = Some(n);
            }
        }
        s.step();
        if s.time() >= next {
            let w = start.elapsed().as_secs_f64();
            println!(
                "  t {:>6.0} s  real {:>7.1} s  {:>6.2} s per game min  drones {:>6}  packet {:>7}  awake {:>7}  memory {:>5.0} MB",
                s.time(),
                w,
                (w - last_w) / ((s.time() - last_t) / 60.0),
                s.drone_count(),
                s.packet,
                s.sandbox.awake(),
                rss_mb()
            );
            let total: f64 = s.prof.iter().sum::<f64>().max(1e-9);
            println!("           time in: drone events {:.0}%, sandbox {:.0}%, rock bodies {:.0}%, per-second work {:.0}%", 100.0 * s.prof[0] / total, 100.0 * s.prof[1] / total, 100.0 * s.prof[2] / total, 100.0 * s.prof[3] / total);
            s.prof = [0.0; 8];
            let st = &s.sandbox.stats;
            println!("           flow fields: {} swept, {:.0} ms", s.sweeps, s.flow_s * 1000.0);
            s.flow_s = 0.0; s.sweeps = 0;
            println!("           drones idle/waiting/out/home {:?}, pool {}, sent from pool {}, flocks opened {}", s.phase_counts(), s.parked_count(), s.pool_sent, s.flocks_opened);
            let pz: Vec<u64> = s.passed.iter().map(|a| a.swap(0, std::sync::atomic::Ordering::Relaxed)).collect();
            println!("           cells passed over: too far {}, taken {}, unreachable {}, not worth it {}; weighed {}; recycled {}, deaths {}", pz[0], pz[1], pz[2], pz[3], pz[4], s.recycled, s.deaths);
            {
                let els = &s.world.planet.mats.elements;
                let mut top: Vec<(f64, &str)> = s.stock.kg.iter().enumerate().map(|(i, k)| (*k, els.list[i].symbol.as_str())).collect();
                top.sort_by(|a, b| b.0.total_cmp(&a.0));
                let t: Vec<String> = top.iter().take(8).map(|(k, e)| format!("{e} {:.1} t", k / 1000.0)).collect();
                println!("           stockpile {:.1} t ({}), in transit {:.1} t; energy {:.0}; cutters built for room {}, room {}", s.stock.total() / 1000.0, t.join(", "), s.transit.total / 1000.0, s.energy, s.cutters_built_for_room, s.last_room);
                let src: Vec<String> = s.eco.recipe.part.iter().map(|p| format!("{}: {}", p.name, s.stock.sources(els, p).into_iter().map(|(_, e)| e.unwrap_or("-".into())).collect::<Vec<_>>().join("+"))).collect();
                println!("           parts made from: {}", src.join(", "));
                let have: Vec<String> = s.eco.recipe.part.iter().zip(&s.parts.n).map(|(p, n)| format!("{} {n}", p.name)).collect();
                println!("           parts in stock: {}", have.join(", "));
            }
            println!("           haulers {} (built {}), cutters waiting {}, flocks {}", s.hauler_views().len(), s.haulers_built, s.waiting_count(), s.flock_count());
            println!("           loose bodies in the air {}, broken free {}, settled {}, shattered {}, crushed drones {}", s.bodies.count(), s.bodies.stats.broken_free, s.bodies.stats.settled, s.bodies.stats.shattered, s.crushed);
            println!("           sandbox queues {:?}, dormant {}; so far: stepped {}, moves {} (powder {}), phase changes {}, reactions {}", s.sandbox.queue_sizes(), s.sandbox.dormant(), st.cells_stepped, st.moves, st.moves_powder, st.phase_changes, st.reactions);
            last_t = s.time();
            last_w = w;
            next += every;
            if w > wall {
                println!("  stopped: {wall:.0} s of real time");
                break;
            }
        }
    }
    for l in &s.director_log {
        println!("  director: {l}");
    }
    println!("  vents: {} cells blasted out, {} drops in the air", s.sandbox.stats.erupted, s.bodies.drops());
    println!("  mined {:.0} t, searches {} ({} empty), events {}", s.world.ledger.mined_kg() / 1000.0, s.counts[1], s.counts[2], s.counts[0]);
    println!("  done: {:.0} s of play in {:.1} s, {} drones, consumed {:.4}%", s.time(), start.elapsed().as_secs_f64(), s.drone_count(), 100.0 * s.consumed_share());
}
