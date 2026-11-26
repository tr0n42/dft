//! unitlab: one ground unit (or a mix) at work on a test body, reported
//! every game minute: what it is doing, what it has done, and what it costs
//! (ms of real time per game second, its own code and the rest).
//!
//! cargo run --release --bin unitlab -- <units> [game_s]
//!   units: comma-separated: sapper, tunneler, heatsink, forester, abductor
//!          (or "all", or "none" for cutters and haulers only)
//! Environment:
//!   BODY    test body (default dev-planet), KM (default its own size)
//!   DRONES  free cutters (default 20000)
//!   KEEP    specialists of each kind at most (default the economy's)
//!   SEEN    "all" (default: everything is on screen, drawn exactly) or
//!           "none" (everything off screen: the shortcuts)
//!   ECON=1  the real economy (factory, stockpile) instead of free drones
use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::{Sim, UNIT_NAMES};
use dft_sim::world::World;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let which = args.first().cloned().unwrap_or_else(|| "sapper".into());
    let until: f64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(900.0);
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let body = std::env::var("BODY").unwrap_or_else(|_| "dev-planet".into());
    let drones: u64 = std::env::var("DRONES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let text = std::fs::read_to_string(data.join("bodies").join(format!("{body}.toml"))).unwrap();
    let text = match std::env::var("KM").ok().and_then(|v| v.parse::<f64>().ok()) {
        Some(km) => dft_sim::spec::resize_text(&text, km * 1000.0).unwrap(),
        None => text,
    };
    let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
    if spec.life.is_none() {
        spec.life = Some(1.0);
    }
    let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
    let planet = dft_sim::planet::Planet::new(spec, mats).unwrap();
    let mut e = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let units: Vec<Unit> = match which.as_str() {
        "all" => Unit::SPECIALISTS.to_vec(),
        "none" => Vec::new(),
        w => w
            .split(',')
            .map(|n| match n.trim() {
                "sapper" => Unit::Sapper,
                "tunneler" => Unit::Tunneler,
                "heatsink" => Unit::HeatSink,
                "forester" => Unit::Forester,
                "abductor" => Unit::Abductor,
                o => panic!("no unit {o}"),
            })
            .collect(),
    };
    let keep: Option<u32> = std::env::var("KEEP").ok().and_then(|v| v.parse().ok());
    let econ = std::env::var("ECON").is_ok();
    e.units.bay.retain(|b| matches!(b.unit, Unit::Cutter | Unit::Hauler) || units.contains(&b.unit));
    if !econ {
        e.seed_swarm.start_harvesters = drones.min(1_000_000) as u32;
    }
    let team: Vec<(Unit, u32)> = units
        .iter()
        .map(|&u| {
            let k = keep.unwrap_or(match u {
                Unit::Sapper => e.units.sapper.keep,
                Unit::Tunneler => e.units.tunneler.keep,
                Unit::HeatSink => e.units.heatsink.keep,
                Unit::Forester => e.units.forester.keep,
                _ => e.units.abductor.keep,
            });
            (u, k)
        })
        .collect();
    let t_build = Instant::now();
    let mut s = Sim::new(World::new(planet), e);
    if !econ {
        s.free_drones = Some(drones);
        s.free_units = team.clone();
    } else {
        s.deploy_only = Some(units.clone());
    }
    // FAR=n: a camera zoomed out over the whole body, flying at most n
    // cutters one by one: the rest of the swarm works far sectors (math).
    if let Some(b) = std::env::var("FAR").ok().and_then(|v| v.parse::<f64>().ok()) {
        let r = s.world.planet.max_radius as i64 + 1000;
        s.set_view(Some(([-r, r, -r, r], 2.0 * r as f64 / 1000.0)));
        s.far.budget = b.max(1e-9);
        s.far.fixed = true;
    }
    if std::env::var("SEEN").as_deref() == Ok("none") {
        s.set_attention(Some(None));
        s.set_focus(Some(None));
    }
    println!("unitlab: {} on {body} ({:.1} km), {} game s, {}; built in {:.1} s", which, s.world.planet.radius / 1000.0, until, if econ { "the economy".to_string() } else { format!("{drones} free cutters") }, t_build.elapsed().as_secs_f64());
    let wall = Instant::now();
    let mut last = Instant::now();
    let mut prof0 = s.unit_prof;
    let (mut sand0, mut total0) = (0.0, 0.0);
    let mut t = 0.0;
    let mut logged: Vec<String> = Vec::new();
    while t < until {
        let step = 60.0f64.min(until - t);
        // SEEN=follow: a screen (1920 x 1080 m, 1 m a pixel) kept on the
        // first specialist, as a player watching it; only that is drawn
        // exactly, the rest approximated.
        if std::env::var("SEEN").as_deref() == Ok("follow") {
            let mut tt = t;
            while tt < t + step {
                tt = (tt + 10.0).min(t + step);
                if let Some(v) = s.unit_views().first() {
                    let (x, y) = (v.to.0 as i64, v.to.1 as i64);
                    let r = [x - 960, x + 960, y - 540, y + 540];
                    s.set_attention(Some(Some(r)));
                    s.set_focus(Some(Some([r[0] - 960, r[1] + 960, r[2] - 540, r[3] + 540])));
                } else {
                    s.set_attention(Some(None));
                    s.set_focus(Some(None));
                }
                s.run(tt);
            }
        }
        t += step;
        s.run(t);
        let real = last.elapsed().as_secs_f64();
        last = Instant::now();
        let per: Vec<String> = UNIT_NAMES
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let v = (s.unit_prof[i] - prof0[i]) / step * 1000.0;
                (v >= 0.05).then(|| format!("{n} {v:.1}"))
            })
            .collect();
        prof0 = s.unit_prof;
        let sand = (s.prof2[4] + s.prof2[5] - sand0) / step * 1000.0;
        sand0 = s.prof2[4] + s.prof2[5];
        let total = (s.prof.iter().take(4).sum::<f64>() - total0) / step * 1000.0;
        total0 = s.prof.iter().take(4).sum::<f64>();
        println!(
            "t {t:>5.0}: {:.2} s real/game min · {} cutters alive, {} died · {:.0} kt mined · ms/game s: all {total:.0}, sand and heat {sand:.1}, {}",
            real / (step / 60.0),
            s.harvesters_alive(),
            s.deaths,
            s.world.ledger.mined_kg() / 1e6,
            per.join(", ")
        );
        if s.far.any_far {
            println!("        far field: {} cutters in far sectors, {:.0} kt eaten there, {} far blasts", s.far_cutters(), s.far.eaten_kg / 1e6, s.far.blasts);
        }
        for l in s.specialist_lines().iter().skip(1) {
            println!("        {}", l.trim_start());
        }
        for l in s.director_log.iter() {
            if !logged.contains(l) {
                println!("        > {l}");
                logged.push(l.clone());
            }
        }
    }
    println!("done: {:.1} s real for {until:.0} game s; fingerprint {:016x}", wall.elapsed().as_secs_f64(), s.state_hash());
}
