//! samecheck: proves a refactor changed nothing. Runs a fixed set of games
//! (every unit kind at work, on and off screen, through a save and load)
//! and prints one fingerprint per game. Run it before and after a change
//! that should not alter play; the lines must match exactly.
//!
//! Run: cargo run --release --bin samecheck   (about 2 minutes)

use dft_sim::economy::Economy;
use dft_sim::save::{R, W};
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn planet(body: &str, km: f64, life: bool) -> Arc<dft_sim::planet::Planet> {
    let d = data();
    let text = std::fs::read_to_string(d.join("bodies").join(format!("{body}.toml"))).unwrap();
    let text = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
    let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
    if life && spec.life.is_none() {
        spec.life = Some(1.0);
    }
    let mats = dft_sim::materials::MaterialTable::load(&d.join("materials.toml").to_string_lossy()).unwrap();
    Arc::new(dft_sim::planet::Planet::new(spec, mats).unwrap())
}

fn eco() -> Economy {
    Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap()
}

fn line(name: &str, s: &Sim, t0: Instant) {
    let (blasts, cracked, frozen, plants, fires) = s.specialist_totals();
    let (caught, _, _) = s.fauna_totals();
    let animals = if s.animals_left() > 0 || caught > 0 { format!(" · animals {} caught {caught}", s.animals_left()) } else { String::new() };
    println!(
        "{name:34} hash {:016x} · drones {} · haulers {} · mined {:.0} t · blasts {blasts} cracked {cracked} · frozen {:.0} t · plants {:.0} t fires {fires} · tunnel {} m{animals} · ({:.1} s)",
        s.state_hash(),
        s.drone_count(),
        s.hauler_views().len(),
        s.world.ledger.mined_kg() / 1000.0,
        frozen / 1000.0,
        plants / 1000.0,
        s.tunnel_m,
        t0.elapsed().as_secs_f64()
    );
}

fn main() {
    // name, body, km, life, game seconds, free drones, watched everywhere
    let cases: [(&str, &str, f64, bool, f64, Option<u64>, bool); 6] = [
        ("living world, economy", "living-world", 5.0, true, 1800.0, None, false),
        ("living world, on screen", "living-world", 3.0, true, 600.0, None, true),
        ("lava world, economy", "lava-world", 3.0, false, 2400.0, None, false),
        ("living world, 500k free", "living-world", 35.0, true, 300.0, Some(500_000), false),
        ("active planetoid, economy", "active-planetoid", 3.0, false, 1200.0, None, false),
        // Every drone at work (added 13 Nov with the Dev planet).
        ("dev planet, economy", "dev-planet", 5.0, true, 1800.0, None, false),
    ];
    for (name, body, km, life, until, free, all) in cases {
        let t0 = Instant::now();
        let p = planet(body, km, life);
        let mut e = eco();
        if let Some(n) = free {
            e.seed_swarm.start_harvesters = n as u32;
        }
        let mut s = Sim::new(World::shared(p.clone()), e.clone());
        s.free_drones = free;
        if !all {
            s.set_attention(Some(None));
        }
        s.run(until);
        line(name, &s, t0);
        // Through a save and back, then on a little: both must agree.
        if free.is_none() {
            let t1 = Instant::now();
            let mut o = W::default();
            s.write_save(&mut o);
            let mut l = Sim::read_save(&mut R(&o.0, 0), p, e).unwrap();
            if !all {
                l.set_attention(Some(None));
                s.set_attention(Some(None));
            }
            s.run(until + 300.0);
            l.run(until + 300.0);
            line(&format!("{name}, +5 min"), &s, t1);
            line(&format!("{name}, loaded +5 min"), &l, t1);
        }
    }
}
