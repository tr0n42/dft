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
//! write the history), NOFOCUS=1 (cutters flown cell by cell everywhere).

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
    if std::env::var("FINE").is_ok() {
        let per = |v: f64| v / game_s.max(1e-9) * 1000.0;
        println!("    fine ms/game s: events {:.1} (work search {:.1}) · sandbox {:.1} · bodies {:.1} · per-second {:.1} (packet etc {:.1}, building {:.1}, orbit {:.1}) · far trips {} near {} · events {} · searches {} dry {}",
            per(s.prof[0]), per(s.prof[4]), per(s.prof[1]), per(s.prof[2]), per(s.prof[3]), per(s.prof[6]), per(s.prof[5]), per(s.prof[7]), s.far_trips, s.near_trips, s.counts[0], s.counts[1], s.counts[2]);
        println!("    per-second parts ms/game s: fleet commander {:.1} · site safety {:.1} · waiting heat {:.1} · flow fields {:.1} · pool {:.1} · haulers {:.1} · fauna {:.1}", per(s.prof3[5]), per(s.prof3[1]), per(s.prof3[2]), per(s.prof3[3]), per(s.prof3[4]), per(s.unit_prof[1]), per(s.fauna_prof));
        s.prof3 = [0.0; 6];
        println!("    fleet commander ({} points moved, {} drones turned) ms/game s: catch-up {:.1} · points moved {:.1} · adrift {:.1} · drop-off points {:.1} · calls {:.1} · answers {:.1}", s.moved_docks, s.turned, per(s.prof4[0]), per(s.prof4[1]), per(s.prof4[2]), per(s.prof4[3]), per(s.prof4[4]), per(s.prof4[5]));
        s.prof4 = [0.0; 6];
        println!("    cuts ms/game s: mine cell {:.1} · open cell {:.1} · whole cut {:.1} · whole far trip {:.1} · cells mined {:.0}/game s", per(s.prof5[0]), per(s.prof5[1]), per(s.prof5[2]), per(s.prof5[3]), per(s.prof5[5]) / 1000.0);
        s.prof5 = [0.0; 6];
    }
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
        // (At most a million built at the start: the rest grow in, so a
        // big body's swarm doesn't start as hundreds of millions of single
        // drones.)
        e.seed_swarm.start_harvesters = drones.min(1_000_000) as u32;
        let team = roster(stage, &e);
        // Only this stage's units exist: no bays (and so no scans for work)
        // for the others.
        e.units.bay.retain(|b| matches!(b.unit, Unit::Cutter | Unit::Hauler) || team.iter().any(|(u, _)| *u == b.unit));
        let mut s = Sim::new(World::new(planet), e);
        s.free_drones = Some(drones);
        s.free_units = team.clone();
        s.sandbox.far_settle = std::env::var("NOSETTLE").is_err();
        // Zoomed out, as the player mostly is.
        s.set_attention(Some(None));
        // ...so cutters' trips are worked out at once (NOFOCUS=1: flown
        // cell by cell everywhere, as before 14 Nov).
        if std::env::var("NOFOCUS").is_err() {
            s.set_focus(Some(None));
        }
        // FAR=1: a camera zoomed out over the whole body (sectors beyond what
        // can be flown one by one go far, see units/far.rs); FAR=close: one
        // close up over where the swarm lands.
        match std::env::var("FAR").as_deref() {
            Ok("close") => {
                let (x, y) = s.mother_pos_now();
                let (x, y) = (x as i64, y as i64);
                s.set_view(Some(([x - 960, x + 960, y - 540, y + 540], 1.0)));
            }
            Ok(_) => {
                let r = s.world.planet.max_radius as i64 + 1000;
                s.set_view(Some(([-r, r, -r, r], 2.0 * r as f64 / 1000.0)));
            }
            Err(_) => {}
        }
        if let Some(b) = std::env::var("FARBUDGET").ok().and_then(|v| v.parse::<f64>().ok()) {
            s.far.budget = b.max(1e-9);
            s.far.fixed = true;
        }
        let names: Vec<String> = team.iter().map(|(u, n)| format!("{n} {}", u.name())).collect();
        println!("\nstage {stage}: {} ({})", STAGES[stage - 1], if names.is_empty() { "no specialists".into() } else { names.join(", ") });
        let t0 = Instant::now();
        let mut sum = vec![0.0; cols.len()];
        let mut t = 0.0;
        let every = 200.0f64.min(until);
        let mut mined0 = 0.0;
        while t < until {
            let w = Instant::now();
            let from = t;
            t = (t + every).min(until);
            // FARSWITCH=t: at game time t the camera comes in close (2 m a
            // pixel) over where the swarm landed, from zoomed out.
            if let Some(at) = std::env::var("FARSWITCH").ok().and_then(|v| v.parse::<f64>().ok()) {
                if from >= at && s.far.view.is_some_and(|v| v.1 > 2.0) {
                    // (FARTARGET=busy: over the far sector with the most cutters,
                    // as the game's camera script goes.)
                    let (x, y) = if std::env::var("FARTARGET").is_ok() {
                        let f = s.far_sectors();
                        let (a, l, _) = f.iter().copied().max_by_key(|x| x.2).unwrap_or((0.0, 0.0, 0));
                        let r = l.min(s.world.planet.max_radius) + 20.0;
                        (a.cos() * r, a.sin() * r)
                    } else {
                        s.mother_pos_now()
                    };
                    let (x, y) = (x as i64, y as i64);
                    s.set_view(Some(([x - 1600, x + 1600, y - 900, y + 900], 2.0)));
                    s.set_focus(Some(Some([x - 3200, x + 3200, y - 1800, y + 1800])));
                    println!("  (camera in close at {from:.0} s)");
                    let k = s.world.far.sector_of(x as f64, y as f64);
                    for i in 1..=20 {
                        s.run(from + i as f64);
                        let live: Vec<(usize, u32)> = (0..s.hubs.len()).filter(|&h| s.world.far.sector_of(s.hubs[h].x, s.hubs[h].y) == k && s.hub_loads()[h] > 0).map(|h| (h, s.hub_loads()[h])).collect();
                        let pz: Vec<u64> = s.passed.iter().map(|a| a.swap(0, std::sync::atomic::Ordering::Relaxed)).collect();
                        println!("    +{i} s: sector {k} far cutters {}, flocks with cutters {:?}, pool {}, flown {}, phases {:?}; searches {} empty {}; passed over (far, taken, unreachable, not worth, weighed) {:?}; frontier {}", s.far.cutters[k], &live[..live.len().min(6)], s.parked_count(), s.drone_count(), s.phase_counts(), s.counts[1], s.counts[2], pz, s.frontier_len());
                    }
                    let r = [x - 1600, x + 1600, y - 900, y + 900];
                    let near: Vec<(f64, f64, u32)> = s.hubs.iter().zip(s.hub_loads()).filter(|(h, n)| **n > 0 && h.x >= r[0] as f64 && h.x <= r[1] as f64 && h.y >= r[2] as f64 && h.y <= r[3] as f64).map(|(h, n)| (h.x, h.y, *n)).collect();
                    {
                        let k = s.world.far.sector_of(x as f64, y as f64);
                        let lv = s.world.far.levels[k];
                        let in_k: Vec<(f64, f64, u32)> = s.hubs.iter().zip(s.hub_loads()).filter(|(h, _)| s.world.far.sector_of(h.x, h.y) == k).map(|(h, n)| (h.x, h.y, *n)).collect();
                        println!("  sector {k}: level {lv} (camera at r {:.0}), far {}, must {}, far cutters {}, open {}, wet {}; flocks there {:?}", ((x * x + y * y) as f64).sqrt(), s.far.is_far[k], s.far.must[k], s.far.cutters[k], s.far.open[k], s.far.wet[k], &in_k[..in_k.len().min(8)]);
                    }
                    println!("  20 s later: {} flocks with cutters on screen, {} cutters; flown {} in all; mother at ({x}, {y}): {:?}", near.len(), near.iter().map(|f| f.2).sum::<u32>(), s.drone_count(), &near[..near.len().min(6)]);
                }
            }
            s.run(t);
            let c = costs(&mut s, t - from);
            for (k, v) in c.iter().enumerate() {
                sum[k] += v * (t - from);
            }
            let (_, waiting, out, home) = s.phase_counts();
            let mined = s.world.ledger.mined_kg();
            let per_t = w.elapsed().as_secs_f64() * 1e6 / ((mined - mined0) / 1000.0).max(1e-9);
            mined0 = mined;
            println!("    efficiency: {per_t:.2} us real per tonne mined");
            let top: Vec<String> = cols.iter().zip(&c).filter(|(_, v)| **v >= 0.5).map(|(n, v)| format!("{n} {v:.0}")).collect();
            println!(
                "  t {t:5.0}: {:6.1} s per game min · {:6} working ({} waiting) · {:.0} kt mined · ms per game s: {}",
                w.elapsed().as_secs_f64() / ((t - from) / 60.0),
                waiting + out + home + s.far_cutters() as usize,
                waiting,
                s.world.ledger.mined_kg() / 1e6,
                top.join(", ")
            );
        }
        let real = t0.elapsed().as_secs_f64();
        let avg: Vec<f64> = sum.iter().map(|v| v / until).collect();
        let hash = format!("{:016x}", s.state_hash());
        println!("  done: {real:.1} s real for {until:.0} game s; fingerprint {hash}");
        for l in s.fleet_lines() {
            println!("  {l}");
        }
        {
            let f = &s.far;
            println!("  far field: {} cutters in {} far sectors of {} ({} reached) · eaten {:.0} kt · folded {} flocks, unfolded {} sectors ({} cutters) · {:.1} s · budget {:.0} one by one, sim {:.0} ms per game s lately", f.total, f.is_far.iter().filter(|x| **x).count(), f.cutters.len(), f.open.iter().filter(|x| **x).count(), f.eaten_kg / 1e6, f.folded, f.unfolded, f.cutters_unfolded, f.prof, f.budget, f.cost_ms);
        }
        println!("  sizes: {:?}", s.sizes());
        let st = &s.sandbox.stats;
        if std::env::var("FAR").is_ok() {
            let n = s.far.is_far.len();
            let row: String = (0..64).map(|i| {
                let k = (i * n / 64).min(n.saturating_sub(1));
                let far = s.far.is_far.get(k).copied().unwrap_or(false);
                let wet = s.far.wet.get(k).copied().unwrap_or(false);
                if wet { 'w' } else if far { 'f' } else if s.far.open.get(k).copied().unwrap_or(false) { 'N' } else { '.' }
            }).collect();
            println!("  slices (f far, N near, w sea): {row}");
            println!("  out-of-sight moves by slice: {:?}", st.far_by_angle);
        }
        println!("  sandbox: stepped {} · moves {} · out of sight: tried {:?} (powder, liquid, gas), stuck {:?}, moved {}", st.cells_stepped, st.moves, st.far_tries, st.far_stuck, st.far_moves);
        let mut bm: Vec<(u64, String)> = st.far_by_mat.iter().enumerate().filter(|x| *x.1 > 0).map(|(m, &n)| (n, s.world.planet.mats.get(m as u16).name.clone())).collect();
        bm.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        println!("  sandbox s: gather {:.1} · heat {:.1} · cell by cell {:.1} (settling out of sight {:.1})", st.prof[0], st.prof[1], st.prof[2], st.prof[3]);
        println!("  out-of-sight moves by material: {:?} · sea refills {}", &bm[..bm.len().min(6)], st.far_sea);
        if let Ok(w) = s.world.planet.mats.id("water") {
            let l = &s.world.ledger;
            println!("  water: mined {:.0} kt · deposited {:.0} kt · of all mined {:.0} kt", l.mined[w as usize] / 1e6, l.deposited[w as usize] / 1e6, l.mined_kg() / 1e6);
        }

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
