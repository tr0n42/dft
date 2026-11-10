use dft_sim::economy::Economy;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn sim(eco: Economy) -> Sim {
    Sim::new(World::new(dft_sim::load_body(&data(), "tutorial-1-2km").unwrap()), eco)
}

/// Play a sim to `minutes` of game time, printing a progress line every 10
/// game minutes so a long test shows it is working.
fn run_with_progress(s: &mut Sim, label: &str, minutes: f64) {
    let t = std::time::Instant::now();
    let mut m = 10.0;
    while m <= minutes && s.launched_at.is_none() {
        let before = s.tick;
        s.run(m * 60.0);
        if s.tick == before {
            break;
        }
        let last = s.samples.last().unwrap();
        println!(
            "  [{label}] {:>3.0} min of play: {:>5} drones, {:>4.1}% consumed{} ({:.0} s so far)",
            s.time() / 60.0,
            last.harvesters as u64 + last.packet,
            last.consumed * 100.0,
            if s.ready_at.is_some() { ", ready to launch" } else { "" },
            t.elapsed().as_secs_f64()
        );
        m += 10.0;
    }
    s.run(minutes * 60.0);
}

/// Print a run's fingerprint: the same numbers on every machine means the
/// simulation is fully deterministic across them.
fn fingerprint(name: &str, h: u64) {
    println!("  fingerprint {name}: {h:016x}");
}

fn eco() -> Economy {
    Economy::load(&data().join("economy.toml").to_string_lossy()).unwrap()
}

#[test]
fn same_start_same_run() {
    let mut a = sim(eco());
    let mut b = sim(eco());
    a.run(240.0);
    b.run(240.0);
    fingerprint("same_start_same_run", a.state_hash());
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn thread_count_never_changes_the_run() {
    let run = |threads: usize| {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
            let mut s = sim(eco());
            s.run(240.0);
            s.state_hash()
        })
    };
    let many = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(2);
    let one = run(1);
    fingerprint("thread_count_never_changes_the_run", one);
    assert_eq!(one, run(many));
}

#[test]
fn books_balance_and_mass_is_accounted_for() {
    let mut s = sim(eco());
    s.run(600.0);
    let w = &s.world;
    let initial: f64 = w.ledger.initial.iter().sum();
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
    // The swarm's Mass is exactly what it was paid, plus remains, minus spending.
    let expect = s.delivered_credited + s.recovered_mass - s.spent_mass;
    assert!((s.mass - expect).abs() < 1e-6 * s.delivered_credited.max(1.0), "{} vs {expect}", s.mass);
    // Energy never goes below zero.
    assert!(s.samples.iter().all(|x| x.energy >= 0.0));
}

#[test]
fn the_swarm_always_keeps_building() {
    // No energy from anything: only the guaranteed free Harvester keeps it going.
    let mut e = eco();
    e.seed_swarm.base_power = 0.0;
    e.seed_swarm.start_energy = 0.0;
    e.seed_swarm.processing_energy_per_tonne = 0.0;
    let mut s = sim(e);
    let start = s.harvesters_built;
    s.run(601.0);
    assert!(s.harvesters_built >= start + 10, "built {} in 10 minutes", s.harvesters_built - start);
}

#[test]
#[ignore = "tutorial timing: rebalanced in the economy pass (run with --ignored)"]
fn the_first_tutorial_takes_about_an_hour() {
    let mut s = sim(eco());
    run_with_progress(&mut s, "tutorial 1", 120.0);
    fingerprint("the_first_tutorial_takes_about_an_hour", s.state_hash());
    println!("  [tutorial 1] ready to launch at {:.1} min", s.ready_at.unwrap_or(f64::NAN) / 60.0);
    let ready = s.ready_at.expect("not ready to launch within 2 hours");
    assert!((45.0 * 60.0..=80.0 * 60.0).contains(&ready), "ready at {:.0} min", ready / 60.0);
    assert!(s.launched_at.is_some() && s.seed_mass > 0.0);
    let peak = s.samples.iter().map(|x| x.harvesters as u64 + x.packet).max().unwrap();
    // About 10k drones is a natural ceiling from rising build costs, not a hard cap.
    assert!(peak <= 10_500, "peak {peak} drones");
    // The orbiting swarm eats the body from all sides: what's left stays round.
    let peel = s.peel_score(4);
    assert!(peel < 0.3, "peel score {:.0}% (eaten lopsided)", peel * 100.0);
}

#[test]
#[ignore = "tutorial timing: rebalanced in the economy pass (run with --ignored)"]
fn prestige_makes_the_second_tutorial_faster() {
    let mut first = sim(eco());
    run_with_progress(&mut first, "prestige: asteroid 1", 120.0);
    let e = eco();
    let mut left = first.seed_mass;
    let perks: Vec<String> = e
        .perk
        .iter()
        .filter(|p| {
            let ok = left >= p.cost;
            if ok {
                left -= p.cost;
            }
            ok
        })
        .map(|p| p.name.clone())
        .collect();
    assert!(perks.iter().any(|p| p == "Mk II drones"));
    let mut planet = dft_sim::load_body(&data(), "tutorial-1-2km").unwrap();
    let mut spec = planet.spec.clone();
    spec.seed += 100;
    planet = dft_sim::planet::Planet::new(spec, planet.mats.clone()).unwrap();
    let mut second = Sim::with_perks(World::new(planet), e, &perks);
    run_with_progress(&mut second, "prestige: asteroid 2", 120.0);
    let (a, b) = (first.ready_at.unwrap(), second.ready_at.expect("second asteroid not ready"));
    println!("  [prestige] asteroid 1 ready at {:.1} min, asteroid 2 at {:.1} min", a / 60.0, b / 60.0);
    assert!(b < 0.7 * a, "second {:.0} min vs first {:.0} min", b / 60.0, a / 60.0);
}

#[test]
fn cooling_lava_keeps_the_books_balanced() {
    let mut w = World::new(dft_sim::load_body(&data(), "test-planetoid-3km").unwrap());
    let mats = w.planet.mats.clone();
    let magma = mats.id("magma").unwrap();
    let rock = mats.id("lava-rock").unwrap();
    // Find some magma: chambers sit in the outer mantle.
    let mut found = None;
    'search: for y in (-1500..1500).step_by(7) {
        for x in (-1500..1500).step_by(7) {
            if w.material(x, y) == magma {
                found = Some((x, y));
                break 'search;
            }
        }
    }
    let (x, y) = found.expect("the test planetoid has magma chambers");
    assert_eq!(w.change_cell(x, y, rock), Some(magma));
    assert_eq!(w.material(x, y), rock);
    // Matter moved from magma to rock; none was made or lost.
    let net: f64 = w.ledger.changed.iter().sum();
    assert!(net.abs() < 1e-9, "cooling changed the mass by {net} kg");
    assert!(w.ledger.changed[magma as usize] < 0.0 && w.ledger.changed[rock as usize] > 0.0);
    let initial: f64 = w.ledger.initial.iter().sum();
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
}

#[test]
fn harder_rock_takes_longer_to_dig() {
    let h = eco().harvester;
    let t: Vec<f64> = [1u8, 3, 4, 5, 7, 10].iter().map(|&x| h.dig_s_for(x)).collect();
    assert!(t.windows(2).all(|p| p[1] > p[0]), "{t:?}");
    assert!((h.dig_s_for(3u8) - h.dig_s).abs() < 1e-9);
}

#[test]
fn tunnelers_bore_down_and_cost_more_than_they_earn() {
    let w = World::new(dft_sim::load_body(&data(), "test-planetoid-3km").unwrap());
    let perks = vec!["Mk II drones".to_string()];
    let mut s = Sim::with_perks(w, eco(), &perks);
    s.tunneler_unlocked = true;
    s.run(25.0 * 60.0);
    assert!(s.tunnel_m >= 100, "only {} m bored", s.tunnel_m);
    // Most of what a Tunneler cuts is crushed to waste: it earns far less Mass
    // per tonne than it burns in energy.
    let t = &s.eco.tunneler;
    assert!(t.energy_per_tonne > 3.0 * s.eco.seed_swarm.processing_energy_per_tonne);
    assert!(t.spoil_recovery < 0.5);
    // Books still balance with shafts in the ground.
    let w = &s.world;
    let initial: f64 = w.ledger.initial.iter().sum();
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
    let expect = s.delivered_credited + s.recovered_mass - s.spent_mass;
    assert!((s.mass - expect).abs() < 1e-6 * s.delivered_credited.max(1.0));
}

#[test]
fn a_big_body_opens_one_landing_zone_at_a_time() {
    // An Earth-sized body isn't surveyed whole: the swarm starts on the ground
    // around its first landing point, sets up at once, and digs straight away.
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let t = std::time::Instant::now();
    let w = dft_sim::world::World::new(dft_sim::load_body(&data, "earth-test").unwrap());
    let eco = dft_sim::economy::Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let zone = eco.seed_swarm.zone_m;
    let mut s = dft_sim::swarm::Sim::new(w, eco);
    let setup = t.elapsed().as_secs_f64();
    let start = s.frontier_len();
    println!("  set up in {setup:.2} s with {start} dig-face cells");
    assert!(setup < 3.0);
    assert!(start > 0 && (start as f64) < zone * 2.0 * 3.0, "frontier {start} is bigger than one zone");
    s.run(60.0);
    let aboard: f64 = s.haulers.iter().map(|h| h.load).sum();
    println!("  after a minute: {} dig-face cells, {:.0} kg in haulers, {} landing points", s.frontier_len(), aboard, s.hubs.len());
    // (Haulers carry it up when full, so after a minute it is still aboard.)
    assert!(aboard + s.delivered_credited > 0.0);
    // (The mother swarm starts over one spot and spreads as it grows.)
    assert!(s.flock_count() >= 1);
}
