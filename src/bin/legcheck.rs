//! legcheck: are drones flying through rock? Runs a body as the game
//! would and, every few game seconds, looks at every drone's flight leg
//! the way the game draws it (along its flock's flow field when the leg
//! starts or ends at a field's hub, else straight) and the way the fog
//! charts it (always straight), and counts the metres of solid rock each
//! crosses.
//!
//! cargo run --release --bin legcheck -- <units> [game_s]
//! Environment: BODY, KM, DRONES (as unitlab); VIEW=whole: a screen over
//! the whole body, as the game starts (the far field then works most of
//! the swarm as math); EVERY (s between looks, default 5).
use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::{Leg, Sim};
use dft_sim::world::World;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let which = args.first().cloned().unwrap_or_else(|| "none".into());
    let until: f64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(600.0);
    let every: f64 = std::env::var("EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0);
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
    e.units.bay.retain(|b| matches!(b.unit, Unit::Cutter | Unit::Hauler) || units.contains(&b.unit));
    e.seed_swarm.start_harvesters = drones.min(1_000_000) as u32;
    let team: Vec<(Unit, u32)> = units
        .iter()
        .map(|&u| {
            let k = match u {
                Unit::Sapper => e.units.sapper.keep,
                Unit::Tunneler => e.units.tunneler.keep,
                Unit::HeatSink => e.units.heatsink.keep,
                Unit::Forester => e.units.forester.keep,
                _ => e.units.abductor.keep,
            };
            (u, k)
        })
        .collect();
    let mut s = Sim::new(World::new(planet), e);
    s.free_drones = Some(drones);
    s.free_units = team;
    s.deploy_only = Some(units.clone());
    s.instant_build = true;
    if std::env::var("VIEW").as_deref() == Ok("whole") {
        // The game's first view: the whole body (and its air) on a 1080-pixel-high screen.
        let r = (s.world.planet.max_radius + s.world.planet.spec.atmosphere_m) * 1.06;
        let ri = r as i64;
        let mpp = 2.0 * r / 1080.0;
        s.set_view(Some(([-ri * 16 / 9, ri * 16 / 9, -ri, ri], mpp)));
        s.set_attention(Some(None));
        s.set_focus(Some(None));
    }
    println!("legcheck: {which} on {body} ({:.1} km), {drones} cutters, {until} game s", s.world.planet.radius / 1000.0);
    let mut legs: Vec<Leg> = Vec::new();
    let mut t = 0.0;
    let (mut worst_drawn, mut worst_fog) = (0.0f64, 0.0f64);
    while t < until {
        t += every;
        s.run(t);
        s.drone_legs(&mut legs);
        let fields: Vec<_> = s.fields.iter().flatten().cloned().collect();
        let now = s.time();
        // (Liquid is flown through, as the flow fields have it.)
        let solid = |x: f64, y: f64| {
            let m = s.world.material(x.floor() as i64, y.floor() as i64);
            m != dft_sim::materials::VACUUM && s.world.planet.mats.get(m).state != dft_sim::materials::State::Liquid
        };
        // Metres of rock along the straight line, its last 3 m each end left
        // out (a cutter's leg ends in the rock it cuts).
        let rock_m = |a: (f64, f64), b: (f64, f64)| -> (f64, f64) {
            let d = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let n = d.ceil() as i64;
            let mut m = 0.0;
            for i in 0..n {
                let k = (i as f64 + 0.5) / n as f64;
                let along = k * d;
                if along < 3.0 || along > d - 3.0 {
                    continue;
                }
                if solid(a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k) {
                    m += d / n as f64;
                }
            }
            (m, d)
        };
        let (mut flying, mut old_bad, mut new_bad) = (0, 0, 0);
        // Drawn through rock, by kind: a drop from the mother swarm, home
        // to a drop-off point, out from one, a hop from cut to cut.
        let (mut old_kinds, mut new_kinds) = ([0usize; 4], [0usize; 4]);
        let sky = s.world.planet.max_radius;
        let all_fields = &s.fields;
        // What the fog would pay: every flying drone's drawn place now.
        let tc = std::time::Instant::now();
        let mut sink = 0.0;
        for leg in &legs {
            let p = leg.drawn_at(now, all_fields, sky);
            sink += p.0;
        }
        let cost_us = tc.elapsed().as_secs_f64() * 1e6;
        let mut examples: Vec<(f64, String)> = Vec::new();
        for leg in &legs {
            if leg.state != 1 && leg.state != 2 {
                continue;
            }
            // Only legs under way now (finished ones sit at `to`).
            let d = ((leg.to.0 - leg.from.0).powi(2) + (leg.to.1 - leg.from.1).powi(2)).sqrt();
            if (now - leg.t0) * leg.speed >= d {
                continue;
            }
            flying += 1;
            let (rock, len) = rock_m(leg.from, leg.to);
            if rock <= 4.0 {
                continue;
            }
            let r0 = (leg.from.0.powi(2) + leg.from.1.powi(2)).sqrt();
            let kind = if r0 > sky { 0 } else if leg.state == 2 { 1 } else if leg.cloud & 1 == 1 { 2 } else { 3 };
            // The old drawing: along a field whose hub is within 8 m of
            // the leg's hub end, else straight.
            let hub_end = if leg.state == 1 { leg.from } else { leg.to };
            let other = if leg.state == 1 { leg.to } else { leg.from };
            let follows = fields.iter().any(|f| (f.hub.0 - hub_end.0).powi(2) + (f.hub.1 - hub_end.1).powi(2) <= 64.0 && f.dist_m(other.0, other.1).or_else(|| f.reach_m((other.0.floor() as i64, other.1.floor() as i64))).is_some());
            if !follows {
                old_bad += 1;
                old_kinds[kind] += 1;
                worst_drawn = worst_drawn.max(rock);
            }
            // The new: rock along the drawn way, a metre of flight at a
            // time, its first and last 3 m left out.
            let step = 1.0 / leg.speed.max(0.1);
            let mut prev = leg.drawn_at(leg.t0, all_fields, sky);
            let mut in_rock = 0.0;
            let mut way = 0.0;
            for i in 1..40_000 {
                let p = leg.drawn_at(leg.t0 + i as f64 * step, all_fields, sky);
                let seg = ((p.0 - prev.0).powi(2) + (p.1 - prev.1).powi(2)).sqrt();
                way += seg;
                let m = ((p.0 + prev.0) * 0.5, (p.1 + prev.1) * 0.5);
                let near_end = |e: (f64, f64)| (m.0 - e.0).powi(2) + (m.1 - e.1).powi(2) < 9.0;
                if !near_end(leg.from) && !near_end(leg.to) && solid(m.0, m.1) {
                    in_rock += seg;
                }
                prev = p;
                if p == leg.to {
                    break;
                }
            }
            if in_rock > 60.0 && std::env::var("DEBUG").is_ok() {
                // Where along its drawn way it is in rock: runs of rock, each
                // with its height, the body's first surface there, and what.
                let mut run: Option<(f64, f64, f64, String)> = None;
                if std::env::var("DEBUG").as_deref() == Ok("2") {
                    let mut w = 0.0;
                    let mut q = leg.drawn_at(leg.t0, all_fields, sky);
                    for i in 0..40_000 {
                        let p = leg.drawn_at(leg.t0 + i as f64 * step, all_fields, sky);
                        w += ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
                        q = p;
                        if i % 6 == 0 || p == leg.to {
                            let r = (p.0 * p.0 + p.1 * p.1).sqrt();
                            let m = s.world.material(p.0.floor() as i64, p.1.floor() as i64);
                            println!("      {w:6.0} m  ({:8.1}, {:8.1})  r {r:6.0}  {}{}", p.0, p.1, s.world.planet.mats.get(m).name, if solid(p.0, p.1) { "  ROCK" } else { "" });
                        }
                        if p == leg.to {
                            break;
                        }
                    }
                }
                let mut prev = leg.drawn_at(leg.t0, all_fields, sky);
                println!("    DEBUG leg flock {} state {} from {:?} to {:?}", leg.flock, leg.state, leg.from, leg.to);
                for i in 1..40_000 {
                    let p = leg.drawn_at(leg.t0 + i as f64 * step, all_fields, sky);
                    let r = (p.0 * p.0 + p.1 * p.1).sqrt();
                    let g = s.world.planet.surface_radius(p.0 / r, p.1 / r);
                    let rock = solid(p.0, p.1);
                    if rock {
                        let m = s.world.material(p.0.floor() as i64, p.1.floor() as i64);
                        let name = s.world.planet.mats.get(m).name.clone();
                        run = Some(match run { Some((a, _, gg, n)) => (a, r, gg, n), None => (r, r, g, name) });
                    } else if let Some((a, b, gg, n)) = run.take() {
                        println!("      rock run r {a:.0}..{b:.0}, first surface {gg:.0}, {n}, near ({:.0}, {:.0})", prev.0, prev.1);
                    }
                    prev = p;
                    if p == leg.to {
                        break;
                    }
                }
            }
            if in_rock > 4.0 {
                new_bad += 1;
                new_kinds[kind] += 1;
                worst_fog = worst_fog.max(in_rock);
                examples.push((
                    in_rock,
                    format!(
                        "kind {kind} state {} cloud {} flock {} · straight {:.0} m ({:.0} m rock) · drawn {:.0} m, {:.0} m through rock · from r {:.0} to r {:.0}",
                        leg.state,
                        leg.cloud,
                        if leg.flock == dft_sim::swarm::Leg::NO_FLOCK { -1 } else { leg.flock as i64 },
                        len,
                        rock,
                        way,
                        in_rock,
                        r0,
                        (leg.to.0.powi(2) + leg.to.1.powi(2)).sqrt()
                    ),
                ));
            }
        }
        examples.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!(
            "t {t:>5.0}: {flying} flying · through rock (>4 m): old drawing {old_bad} (drops {}, home {}, out {}, hops {}) · new drawing {new_bad} (drops {}, home {}, out {}, hops {}) · drawn places for all {} legs {:.0} us{}",
            old_kinds[0], old_kinds[1], old_kinds[2], old_kinds[3], new_kinds[0], new_kinds[1], new_kinds[2], new_kinds[3], legs.len(), cost_us, if sink.is_nan() { "!" } else { "" }
        );
        for (_, e) in examples.iter().take(3) {
            println!("        {e}");
        }
    }
    println!("done: worst through rock: old drawing {worst_drawn:.0} m, new drawing {worst_fog:.0} m");
}
