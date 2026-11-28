//! A sapper's buried blast (the crater thrown out, the shock's cracks, the
//! loosened column) makes and loses nothing: what was blown out is all still
//! on the body (on the ground, or in the air on its way).
use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
use dft_sim::save::{R, W};
use dft_sim::swarm::{CraterKind, Sim};
use dft_sim::world::World;
use std::path::Path;

fn sim() -> Sim {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "barren-world").unwrap();
    let mut spec = base.spec.clone();
    spec.radius_m = 3000.0;
    let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(0);
    s
}

/// The same body with its crust's rock made harder (hardness 8, where the
/// barren world's is 3 to 6): a blast holds only rock harder than 6
/// together in big pieces, so on the plain body it holds none.
fn hard_sim() -> Sim {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "barren-world").unwrap();
    let mut spec = base.spec.clone();
    spec.radius_m = 3000.0;
    // (The table as loaded, before the body changed it: `Planet::new` starts
    // from that.)
    let mut mats = base.mats.pristine.as_deref().cloned().unwrap_or_else(|| base.mats.clone());
    for name in ["anorthosite", "impact breccia", "basalt", "ilmenite", "troilite"] {
        let m = mats.id(name).unwrap();
        mats.list[m as usize].hardness = 8;
    }
    let p = dft_sim::planet::Planet::new(spec, mats).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(0);
    s
}

/// How far the mass booked as in the air is from what is really in the air
/// (kg, over all materials). The books' own balance can't see a piece that
/// went missing from the air without being set down or lost: it stays
/// booked as in the air, and the ground has that much less, so the two
/// cancel.
fn air_off(s: &Sim) -> f64 {
    let mats = &s.world.planet.mats;
    let mut real = vec![0.0; s.world.ledger.aloft.len()];
    for m in s.aloft_mats() {
        real[m as usize] += mats.get(m).density;
    }
    s.world.ledger.aloft.iter().zip(&real).map(|(a, b)| (a - b).abs()).sum()
}

/// A save written and loaded back, on the same body.
fn save_and_load(s: &mut Sim) -> Sim {
    s.finish_blasts();
    let mut o = W::default();
    s.write_save(&mut o);
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut l = Sim::read_save(&mut R(&o.0, 0), s.world.planet.clone(), eco).unwrap();
    l.free_drones = Some(0);
    l
}

#[test]
fn a_sapper_blast_throws_out_a_crater_and_keeps_the_books() {
    let mut s = sim();
    // Over the ground on the far side from where the swarm lands.
    let over = (0.5, -(s.world.planet.max_radius + 5.0));
    let mut y = over.1 as i64;
    while s.world.material(0, y) == VACUUM {
        y += 1;
    }
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    s.test_blast(over, 10.0);
    // The books balance while its front runs out (rock in the air counted
    // as aloft)...
    let mut t = 0;
    while s.blasts_pending() > 0 {
        s.step();
        t += 1;
        assert!(t < 600, "the blast ran its course within a minute");
        if t % 3 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    for _ in t..600 {
        s.step();
    }
    // A crater: the middle well down from where the ground was.
    let mut y2 = over.1 as i64;
    while s.world.material(0, y2) == VACUUM {
        y2 += 1;
    }
    assert!(y2 - y >= 6, "the ground over the blast is {} m lower (at least 6 wanted)", y2 - y);
    assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
    // ...and when one is finished at once part way, as a save does (a
    // second blast a few hundred metres along).
    let far = s.world.planet.max_radius + 5.0;
    s.test_blast((-far * 0.15f64.sin(), -far * 0.15f64.cos()), 10.0);
    for _ in 0..3 {
        s.step();
    }
    assert!(s.blasts_pending() > 0, "still going off three ticks in");
    s.finish_blasts();
    assert_eq!(s.blasts_pending(), 0);
    assert!(off(&s) < 1e-6 * initial, "finished at once: the books are off by {} kg", off(&s));
    for _ in 0..300 {
        s.step();
    }
    assert!(off(&s) < 1e-6 * initial, "after: the books are off by {} kg", off(&s));
    assert!(air_off(&s) < 1e-6 * initial, "after: the air is off by {} kg", air_off(&s));
}

/// A blast's events come by its own clock (docs/explosion-sequence.md): its
/// shock runs through the rock at the rock's real speed and reaches the
/// ground over a sapper's 10 kt charge within the first tick. From then its
/// crater's ground rises as a dome, slower than the ground over the charge
/// jumps (about 42 m/s here), and the ground round the crater is kicked up
/// and falls back. When the dome bursts (about 1.8 s on; a conventional
/// charge's about twice as soon) what is rising with it is thrown out in its
/// fan, faster, and its gas vents (a glow for a nuclear charge, fumes for a
/// conventional one). The screen is told when it went off and how fast its
/// shock runs, to draw it by, and the books balance throughout.
#[test]
fn a_blast_lifts_its_crater_as_a_dome_and_throws_it_when_the_dome_bursts() {
    for conventional in [false, true] {
        let mut s = sim();
        let initial: f64 = s.world.ledger.initial.iter().sum();
        let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
        let over = (0.5, -(s.world.planet.max_radius + 5.0));
        let mut y0 = over.1 as i64;
        while s.world.material(0, y0) == VACUUM {
            y0 += 1;
        }
        // The top of the ground over the charge.
        let top = move |s: &Sim| (-6..=6).flat_map(|x: i64| (0..6).map(move |d: i64| (x, y0 + d))).filter(|&(x, y)| s.world.material(x, y) != VACUUM).count();
        let before = top(&s);
        let depth = s.test_blast_depth(10.0);
        s.take_booms();
        s.test_blast_at(over, 10.0, depth, conventional);
        let tl = s.blast_timelines()[0];
        assert_eq!(tl.nuclear, !conventional);
        assert_eq!(tl.kind, CraterKind::Crater);
        assert!((tl.depth_m - depth).abs() < 3.0, "{} m deep (put {depth} m down)", tl.depth_m);
        assert!((1_000.0..=6_000.0).contains(&tl.shock_mps), "the shock runs at {} m/s", tl.shock_mps);
        assert!(tl.t_surface < s.eco.tick_s, "the shock reaches the ground {} s on", tl.t_surface);
        let full = if conventional { 0.45 } else { 0.85 } * 10f64.cbrt();
        assert!(tl.t_vent > 0.8 * full && tl.t_vent <= full + 1e-9, "the dome bursts {} s on ({full} s at most)", tl.t_vent);
        assert!((30.0..60.0).contains(&tl.spall_v0), "the ground over it jumps at {} m/s", tl.spall_v0);
        assert!(tl.spall_m > 2.0 * depth && tl.spall_m <= 3.0 * depth + 1e-9, "the ground jumps out to {} m either side", tl.spall_m);
        let booms = s.take_booms();
        assert_eq!(booms.len(), 1);
        let b = booms[0];
        assert!(b.buried && b.nuclear == !conventional);
        assert!((b.t - s.time()).abs() < 1e-9 && (b.shock_mps as f64 - tl.shock_mps).abs() < 1.0);
        assert!((b.t_surface as f64 - tl.t_surface).abs() < 1e-4 && (b.t_vent as f64 - tl.t_vent).abs() < 1e-4);
        let born = s.time();
        let (mut dome_v, mut hop_v, mut hopped, mut at_vent, mut rising) = (0.0f64, 0.0f64, 0, before, 0);
        let mut burst = None;
        for k in 1..=100 {
            s.step();
            // (As of the end of the tick, as the blast reckons it.)
            let age = k as f64 * s.eco.tick_s;
            let (_, dome, hop) = s.bodies.fastest();
            if age + 1e-9 < tl.t_vent {
                if age >= 0.6 {
                    assert!(s.last_crater.0 > 0, "{age:.1} s on: the crater's ground on its way");
                    assert!(s.bodies.rising() > 0, "{age:.1} s on: the crater's ground rising as a dome");
                }
                dome_v = dome_v.max(dome);
                hop_v = hop_v.max(hop);
                hopped = hopped.max(s.bodies.hopping());
                at_vent = top(&s);
                rising = s.bodies.rising();
            } else if burst.is_none() {
                // The dome burst this tick: what rose with it is thrown out,
                // not set down.
                burst = Some(age);
                assert_eq!(s.bodies.rising(), 0, "{age:.1} s on: the dome burst, nothing still rising with it");
                assert!(s.bodies.drops() as f64 >= 0.8 * rising as f64, "{age:.1} s on: {rising} pieces rising before the burst, {} in the air after", s.bodies.drops());
            }
            if k % 3 == 0 {
                assert!(off(&s) < 1e-6 * initial, "{age:.1} s on: the books are off by {} kg", off(&s));
                assert!(air_off(&s) < 1e-6 * initial, "{age:.1} s on: the air is off by {} kg", air_off(&s));
            }
        }
        // The dome rises at three quarters of the jump over the charge (see
        // DOME_SHARE), the middle fastest; the ground round it jumps slower.
        assert!(dome_v > 0.5 * tl.spall_v0 && dome_v <= 0.75 * tl.spall_v0 + 0.5, "the dome rose at {dome_v} m/s at most (the ground jumping at {})", tl.spall_v0);
        assert!(hopped > 0 && hop_v > 3.0 && hop_v <= tl.spall_v0, "the ground round the crater kicked up ({hopped} pieces at most, {hop_v} m/s at most)");
        assert!(at_vent + 15 < before, "the ground over the charge rose before the dome burst ({before} cells to {at_vent})");
        let burst = burst.expect("the dome burst");
        assert!(burst >= tl.t_vent && burst < tl.t_vent + s.eco.tick_s, "the dome burst {burst:.1} s on, its time {:.2} s", tl.t_vent);
        assert_eq!(s.vents.len(), 1, "its gas vented once");
        let v = s.vents[0];
        assert!(v.nuclear == !conventional && (v.t - born - tl.t_vent).abs() < 1e-9);
        assert!((v.x - over.0).abs() < 10.0 && ((v.y - y0 as f64).abs()) < 10.0, "it vents at the ground over the charge ({}, {}), the ground at {y0}", v.x, v.y);
        // Ten seconds on the kicked-up ground is down (but for the pieces
        // from the slope into this body's deep chasm, 80 to 100 m out, still
        // falling down it), and the crater is open.
        assert!(s.bodies.hopping() * 5 < hopped, "the kicked-up ground back down ({} of {hopped} still in the air)", s.bodies.hopping());
        assert!(top(&s) + 30 < before, "a crater: {before} cells over the charge before, {} after", top(&s));
        assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
    }
}

/// The ground round a crater that the shock kicks up falls back near where
/// it was within a few seconds (pulled back as on Earth: on this small body
/// it would otherwise hang there for minutes), as loose rubble a cutter
/// takes quickly, so the ground there ends up about as high as before; and
/// a save taken while it is in the air (or while the dome still rises) sets
/// it all down first, with the books balanced.
#[test]
fn the_ground_round_a_crater_is_kicked_up_and_falls_back() {
    let mut s = sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let over = (0.5, -(s.world.planet.max_radius + 5.0));
    let depth = s.test_blast_depth(10.0);
    // The ground's height (m from the centre, the first cell met walking
    // down from the sky), and its cell, at places along it `x` m from over
    // the charge, between about twice and three times the crater's
    // half-width out.
    let a0 = over.1.atan2(over.0);
    let top_r = s.world.planet.max_radius + 5.0;
    let ground = move |s: &Sim, x: i64| {
        let a = a0 + x as f64 / (top_r - 60.0);
        let mut r = top_r;
        loop {
            let c = ((a.cos() * r).floor() as i64, (a.sin() * r).floor() as i64);
            if s.world.material(c.0, c.1) != VACUUM || r < 1.0 {
                return (r.round() as i64, c);
            }
            r -= 0.5;
        }
    };
    let xs: Vec<i64> = [-120, -110, -100, 100, 110, 120].into();
    let before: Vec<(i64, (i64, i64))> = xs.iter().map(|&x| ground(&s, x)).collect();
    // Saved with it in the air and the dome still rising: all of it down,
    // or thrown out, and the books balance.
    {
        let mut s = sim();
        s.test_blast_at(over, 10.0, depth, false);
        for _ in 0..8 {
            s.step();
        }
        assert!(s.bodies.hopping() > 0 && s.bodies.rising() > 0, "in the air to save");
        let mut l = save_and_load(&mut s);
        assert_eq!(s.bodies.hopping(), 0, "the kicked-up ground set down for the save");
        assert_eq!(s.bodies.rising(), 0, "the dome burst for the save");
        assert!(off(&l) < 1e-6 * initial, "loaded: the books are off by {} kg", off(&l));
        assert!(air_off(&l) < 1e-6 * initial, "loaded: the air is off by {} kg", air_off(&l));
        for _ in 0..50 {
            l.step();
        }
        assert!(off(&l) < 1e-6 * initial, "loaded, after: the books are off by {} kg", off(&l));
        assert!(air_off(&l) < 1e-6 * initial, "loaded, after: the air is off by {} kg", air_off(&l));
    }
    s.test_blast_at(over, 10.0, depth, false);
    let tl = s.blast_timelines()[0];
    assert!(tl.spall_m > 130.0, "the ground jumps out to {} m", tl.spall_m);
    let mut most = 0;
    for _ in 0..10 {
        s.step();
        most = most.max(s.bodies.hopping());
    }
    assert!(most > 100, "the ground round the crater kicked up ({most} pieces)");
    // It comes down within a few seconds, near where it was.
    for t in 0..60 {
        s.step();
        if t % 5 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    assert!(s.bodies.hopping() * 5 < most, "the kicked-up ground back down, but for some still falling down this body's chasm 80 to 100 m out ({} of {most})", s.bodies.hopping());
    let loose: std::collections::BTreeSet<(i64, i64)> = s.loose_cells().collect();
    for (k, &x) in xs.iter().enumerate() {
        let (now, c) = ground(&s, x);
        assert!((now - before[k].0).abs() <= 4, "{x} m along: the ground was {} m from the centre, now {now}", before[k].0);
        let near = (-1..=1).any(|dx| (-1..=1).any(|dy| loose.contains(&(c.0 + dx, c.1 + dy))));
        assert!(near, "{x} m along: the ground there is loose rubble now");
    }
    assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
}

/// The ground a blast kicks up goes as its shock gets there, give or take
/// SPALL_CATCH_S: within half a second of a 10 kt sapper's charge going
/// off, the shock reaching the ground in its first tick. (Planned after the
/// crater, it went 0.4 s on, with nothing moving till then.) The cutters on
/// it are wrecked as it goes, not before.
#[test]
fn the_ground_jumps_as_the_shock_reaches_it() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "dev-planet").unwrap();
    let p = dft_sim::planet::Planet::new(base.spec.clone(), base.mats.clone()).unwrap();
    let mut eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    eco.seed_swarm.start_harvesters = 3000;
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(3000);
    s.run(40.0);
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    // Right under the thickest of the cutters at work.
    let mut legs = Vec::new();
    s.drone_legs(&mut legs);
    let now = s.time();
    let pts: Vec<(f64, f64)> = legs.iter().filter(|l| l.state <= 2).map(|l| l.at(now)).collect();
    let crowd = |p: (f64, f64)| pts.iter().filter(|q| (q.0 - p.0).hypot(q.1 - p.1) < 60.0).count();
    let p0 = pts.iter().step_by(10).copied().max_by_key(|&p| crowd(p)).unwrap();
    let a = p0.1.atan2(p0.0);
    let g = s.world.planet.surface_radius(a.cos(), a.sin());
    let depth = s.test_blast_depth(10.0);
    let kills0 = s.ground_report().blast_kills;
    s.test_blast_at((a.cos() * g, a.sin() * g), 10.0, depth, false);
    let tl = s.blast_timelines()[0];
    assert!(tl.t_surface < s.eco.tick_s, "the shock reaches the ground {} s on", tl.t_surface);
    // (Those in the loosened column's rock go at once, as before.)
    let at_once = s.ground_report().blast_kills - kills0;
    let (mut jumped, mut wrecked_before) = (None, 0);
    for k in 1..=40 {
        s.step();
        let kills = s.ground_report().blast_kills - kills0 - at_once;
        if jumped.is_none() {
            if s.bodies.hopping() > 0 {
                jumped = Some(k as f64 * s.eco.tick_s);
            } else {
                wrecked_before = wrecked_before.max(kills);
            }
        }
        if k % 5 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {k}: the books are off by {} kg", off(&s));
        }
    }
    let jumped = jumped.expect("the ground kicked up");
    let wrecked = s.ground_report().blast_kills - kills0 - at_once;
    println!("the ground jumped {jumped:.1} s on; {at_once} cutters wrecked at once, {wrecked} by the jump");
    assert!(jumped <= 0.5, "the ground jumped {jumped:.1} s on");
    assert!(wrecked > 50, "the cutters on the jumping ground wrecked ({wrecked})");
    assert_eq!(wrecked_before, 0, "cutters wrecked by the jump before the ground under them moved");
    while s.blasts_pending() > 0 {
        s.step();
    }
    assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
    assert!(air_off(&s) < 1e-6 * initial, "the air is off by {} kg", air_off(&s));
}

/// A charge too deep for its yield to throw a crater out (subsidence, mound
/// or contained: docs/explosion-sequence.md, step 6) throws none: the ground
/// over it jumps and falls back where it was, and no dome bursts and no gas
/// vents. Over a subsidence shot it then sinks a little, as the rubble under
/// it settles into the cavity; over the deeper ones it stays within a few
/// metres of where it was. (Until 27 Nov a full crater was thrown from any
/// depth, popping out 250 m over a contained shot, the ground there 16 to 19
/// m lower.) The books balance throughout.
#[test]
fn a_charge_too_deep_for_a_crater_throws_none() {
    for (depth, kind) in [(151.0, CraterKind::Subsidence), (200.0, CraterKind::Mound), (250.0, CraterKind::Contained)] {
        let mut s = sim();
        let initial: f64 = s.world.ledger.initial.iter().sum();
        let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
        let over = (0.5, -(s.world.planet.max_radius + 5.0));
        // The ground's top over the charge, across 7 m of it.
        let ground = |s: &Sim| {
            (-3..=3)
                .map(|x| {
                    let mut y = over.1 as i64;
                    while s.world.material(x, y) == VACUUM {
                        y += 1;
                    }
                    y
                })
                .sum::<i64>() as f64
                / 7.0
        };
        let y0 = ground(&s);
        s.test_blast_at(over, 10.0, depth, false);
        let tl = s.blast_timelines()[0];
        assert_eq!(tl.kind, kind, "{depth} m down");
        let mut t = 0;
        while s.blasts_pending() > 0 || t < 200 {
            s.step();
            t += 1;
            assert!(t < 600, "{depth} m: the blast ran its course within a minute");
            if t % 5 == 0 {
                assert!(off(&s) < 1e-6 * initial, "{depth} m, tick {t}: the books are off by {} kg", off(&s));
                assert!(air_off(&s) < 1e-6 * initial, "{depth} m, tick {t}: the air is off by {} kg", air_off(&s));
            }
        }
        let y1 = ground(&s);
        println!("{depth} m down ({kind:?}): the ground over it {:.1} m lower", y1 - y0);
        assert_eq!(s.last_crater.0, 0, "{depth} m: a crater thrown out");
        assert!(s.vents.is_empty(), "{depth} m: its gas vented");
        // (A sapper's 10 kt crater leaves the ground there 17 m lower.)
        let (lo, hi) = if kind == CraterKind::Subsidence { (-2.0, 12.0) } else { (-4.0, 4.0) };
        assert!(y1 - y0 >= lo && y1 - y0 <= hi, "{depth} m: the ground over it {:.1} m lower", y1 - y0);
        assert!(off(&s) < 1e-6 * initial, "{depth} m: the books are off by {} kg", off(&s));
    }
}

/// A test blast asked for on the ground goes off on the ground, wherever it
/// is: in the open, with a fireball on a body with air. (Its depth was
/// looked for again beside it, as for a sapper's charge down its shaft: on
/// rough ground 13 of 60 such blasts on the barren body went off buried,
/// one 812 m down, and one on the dev planet 3.5 m down, with no fireball.)
#[test]
fn a_test_blast_on_the_ground_goes_off_on_the_ground() {
    for air in [false, true] {
        let mut s = if air { dev_sim() } else { sim() };
        let initial: f64 = s.world.ledger.initial.iter().sum();
        let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
        let top = s.world.planet.max_radius + 5.0;
        for k in 0..12 {
            let a = k as f64 * std::f64::consts::TAU / 12.0 + 0.01;
            s.take_booms();
            s.test_blast_at((a.cos() * top, a.sin() * top), 0.1, 0.0, k % 2 == 1);
            let tl = *s.blast_timelines().last().unwrap();
            let b = s.take_booms()[0];
            assert!(tl.depth_m == 0.0 && tl.open() && !b.buried, "{k}: {} m down", tl.depth_m);
            assert_eq!(tl.fireball_m > 0.0, air, "{k}: a fireball ({} m) only with air", tl.fireball_m);
        }
        s.finish_blasts();
        assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
        assert!(air_off(&s) < 1e-6 * initial, "the air is off by {} kg", air_off(&s));
    }
}

/// A save taken while a blast's ground is still to be thrown (the kicked-up
/// ground round its crater, due as its shock gets there; the plants its air
/// blast blows down, due as its front does; its crater's deeper layers)
/// throws it all first, so the game loaded from the save has the world the
/// game that goes on has. (Left queued, the game that went on threw it and
/// the loaded one never did: 3,248 plant cells left within 600 m against
/// 3,753.)
#[test]
fn a_save_throws_what_a_blast_still_has_to_throw() {
    let mut s = dev_life();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    // In the thickest forest on the body.
    let (a, _) = (0..72).map(|k| k as f64 * std::f64::consts::TAU / 72.0).map(|a| (a, plants_band(&s, a, 0.0, 300.0).len())).max_by_key(|p| p.1).unwrap();
    let g = s.world.planet.surface_radius(a.cos(), a.sin());
    let depth = s.test_blast_depth(10.0);
    s.test_blast_at((a.cos() * g, a.sin() * g), 10.0, depth, false);
    for _ in 0..3 {
        s.step();
    }
    assert!(s.blasts_pending() > 0, "still going off three ticks in");
    let mut l = save_and_load(&mut s);
    assert_eq!(s.ejecta_pending(), 0, "nothing left waiting to be thrown");
    assert_eq!(s.bodies.hopping() + s.bodies.rising(), 0, "nothing left in the air waiting");
    for _ in 0..60 {
        s.step();
        l.step();
    }
    let (left, loaded) = (plants_band(&s, a, 0.0, 600.0).len(), plants_band(&l, a, 0.0, 600.0).len());
    println!("{left} plant cells left within 600 m in the game that went on, {loaded} in the one loaded");
    assert!(left.abs_diff(loaded) * 50 <= left.max(loaded), "{left} plant cells left in the game that went on, {loaded} in the one loaded");
    for s in [&s, &l] {
        assert!(off(s) < 1e-6 * initial, "the books are off by {} kg", off(s));
        assert!(air_off(s) < 1e-6 * initial, "the air is off by {} kg", air_off(s));
    }
}

/// On a living world, the plants on the ground a blast throws are torn out
/// and thrown with it, and die where they land (they are cut off: the life
/// cycle has them rot); none is left hanging over the hole; and the animals
/// on that ground are flung and killed, a burst of gore each (Benjamin, 27
/// Nov: plants were left standing with their roots blown off).
#[test]
fn plants_and_animals_on_thrown_ground_die() {
    // The dev planet with its life on, as in the game.
    let mut s = {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let base = dft_sim::load_body(&data, "dev-planet").unwrap();
        let mut spec = base.spec.clone();
        spec.life = Some(1.0);
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
        let mut s = Sim::new(World::new(p), eco);
        s.free_drones = Some(0);
        s
    };
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let flora = |s: &Sim, c: (i64, i64)| {
        let m = s.world.material(c.0, c.1);
        m != VACUUM && s.world.planet.mats.get(m).name.starts_with("flora ")
    };
    // Plant cells over the ground within `half` m either way of angle `a`.
    let plants = |s: &Sim, a: f64, half: f64| {
        let (ux, uy) = (a.cos(), a.sin());
        let g = s.world.planet.surface_radius(ux, uy);
        let mut out = std::collections::BTreeSet::new();
        for along in (-half as i64)..=(half as i64) {
            for up in -8..130 {
                let r = g + up as f64;
                let c = ((ux * r - uy * along as f64).floor() as i64, (uy * r + ux * along as f64).floor() as i64);
                if flora(s, c) {
                    out.insert(c);
                }
            }
        }
        out
    };
    // The plants round where a crater would be (its own fly off with its
    // ground, most of them off this small body): from 60 m to 120 m out
    // either way of angle `a`.
    let round = |s: &Sim, a: f64| {
        let r0 = s.world.planet.surface_radius(a.cos(), a.sin());
        let off = |c: &(i64, i64)| {
            let d = (c.1 as f64 + 0.5).atan2(c.0 as f64 + 0.5) - a;
            d.sin().atan2(d.cos()).abs() * r0
        };
        plants(s, a, 120.0).into_iter().filter(|c| off(c) > 60.0).count()
    };
    // Under the animal with the most plants round it.
    let views = s.animal_views();
    assert!(!views.is_empty(), "the dev planet has animals");
    let (pick, near) = views.iter().step_by(8).take(16).map(|v| (v, round(&s, v.to.1.atan2(v.to.0)))).max_by_key(|p| p.1).unwrap();
    let (id, at) = (pick.id, pick.to);
    let a = at.1.atan2(at.0);
    assert!(near > 50, "plants round the blast ({near} cells)");
    let (_, _, lost0) = s.fauna_totals();
    let rotting0 = s.life.as_ref().map_or(0, |l| l.counts().2);
    let depth = s.test_blast_depth(10.0);
    s.test_blast_at(at, 10.0, depth, false);
    let tl = s.blast_timelines()[0];
    s.gore.clear();
    // The animal over it is flung and killed as soon as the ground under it
    // goes (its plan worked out within the first few ticks), not before.
    let mut k = 0;
    while s.animal_views().iter().any(|v| v.id == id) {
        s.step();
        k += 1;
        assert!(k <= 6, "the animal over the blast is killed within {k} ticks");
    }
    assert!(s.bodies.hopping() + s.bodies.rising() > 0, "the ground under it on its way when it is killed");
    let (_, _, lost) = s.fauna_totals();
    assert!(lost > lost0, "counted as lost");
    assert!(!s.gore.is_empty(), "a burst of gore for it");
    let mut t = 0;
    while s.blasts_pending() > 0 || t < 100 {
        s.step();
        t += 1;
        assert!(t < 600, "the blast ran its course within a minute");
        if t % 10 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    assert_eq!(s.bodies.hopping(), 0, "the kicked-up ground all back down");
    // Torn out and landed: the life cycle has them dying.
    let rotting = s.life.as_ref().map_or(0, |l| l.counts().2);
    assert!(rotting >= rotting0 + near / 2, "{near} plant cells over it, {} more dying", rotting as i64 - rotting0 as i64);
    // None left hanging: every plant cell over the ground that jumped is
    // joined up, plant to plant (side by side, as the world holds pieces
    // together), to the ground (or to a big stand).
    let reach = (0.9 * tl.spall_m).min(150.0);
    let loose: std::collections::BTreeSet<(i64, i64)> = s.loose_cells().collect();
    for c in plants(&s, a, reach) {
        // (Not the plants thrown and landed: rubble, lying where it fell.)
        if loose.contains(&c) {
            continue;
        }
        let mut seen = std::collections::BTreeSet::from([c]);
        let mut queue = vec![c];
        let mut held = false;
        while let Some(q) = queue.pop() {
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (q.0 + d.0, q.1 + d.1);
                let m = s.world.material(n.0, n.1);
                if m == VACUUM || seen.contains(&n) {
                    continue;
                }
                if !flora(&s, n) {
                    held = true;
                    break;
                }
                seen.insert(n);
                queue.push(n);
            }
            if held || seen.len() > 4000 {
                held = true;
                break;
            }
        }
        assert!(held, "the plant cell at {c:?} is left hanging");
    }
}

/// The dev planet with its life on, as in the game: it has air, plants and
/// animals.
fn dev_life() -> Sim {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "dev-planet").unwrap();
    let mut spec = base.spec.clone();
    spec.life = Some(1.0);
    let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(0);
    s
}

fn is_plant(s: &Sim, c: (i64, i64)) -> bool {
    let m = s.world.material(c.0, c.1);
    m != VACUUM && s.world.planet.mats.get(m).name.starts_with("flora ")
}

/// The plant cells standing over the ground from `x0` to `x1` m along it
/// either side of angle `a` (not those thrown and landed, which lie loose).
fn plants_band(s: &Sim, a: f64, x0: f64, x1: f64) -> std::collections::BTreeSet<(i64, i64)> {
    let (ux, uy) = (a.cos(), a.sin());
    let g = s.world.planet.surface_radius(ux, uy);
    let loose: std::collections::BTreeSet<(i64, i64)> = s.loose_cells().collect();
    let mut out = std::collections::BTreeSet::new();
    for side in [-1.0, 1.0] {
        let mut x = x0;
        while x <= x1 {
            let b = a + side * x / g;
            let (vx, vy) = (b.cos(), b.sin());
            let r0 = s.world.planet.surface_radius(vx, vy);
            for up in -8..130 {
                let r = r0 + up as f64;
                let c = ((vx * r).floor() as i64, (vy * r).floor() as i64);
                if is_plant(s, c) && !loose.contains(&c) {
                    out.insert(c);
                }
            }
            x += 0.7;
        }
    }
    out
}

/// A plant cell not joined up, plant to plant side by side, to the ground
/// (or to a stand of more than 4,000 cells), if any of `cells` is.
fn hanging(s: &Sim, cells: &std::collections::BTreeSet<(i64, i64)>) -> Option<(i64, i64)> {
    let loose: std::collections::BTreeSet<(i64, i64)> = s.loose_cells().collect();
    for &c in cells {
        if loose.contains(&c) || !is_plant(s, c) {
            continue;
        }
        let mut seen = std::collections::BTreeSet::from([c]);
        let mut queue = vec![c];
        let mut held = false;
        while let Some(q) = queue.pop() {
            for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (q.0 + d.0, q.1 + d.1);
                if s.world.material(n.0, n.1) == VACUUM || seen.contains(&n) {
                    continue;
                }
                if !is_plant(s, n) {
                    held = true;
                    break;
                }
                seen.insert(n);
                queue.push(n);
            }
            if held || seen.len() > 4000 {
                held = true;
                break;
            }
        }
        if !held {
            return Some(c);
        }
    }
    None
}

/// On a body with air, a buried blast's air blast runs out from the ground
/// over it at the speed of sound once its shock gets there
/// (docs/explosion-sequence.md, part 3), far past the ground's jump: the
/// plants it reaches are blown down (in sight thrown downwind, landing to
/// wilt, none of them before the front gets there and none left hanging),
/// animals thrown or killed; and the crater's dust rolls out along the
/// ground as a base surge, from when the dome bursts. Out of sight, the
/// trees it blows down are killed where they stand. The books balance
/// throughout.
#[test]
fn an_air_blast_blows_down_plants_and_throws_animals() {
    for seen in [true, false] {
        let mut s = dev_life();
        if !seen {
            s.set_attention(Some(None));
        }
        let initial: f64 = s.world.ledger.initial.iter().sum();
        let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
        // Over the ground 250 m along from an animal, where the plants
        // round it are thickest.
        let views = s.animal_views();
        let r = s.world.planet.radius;
        let (a, near) = views
            .iter()
            .step_by(4)
            .take(30)
            .flat_map(|v| [-1.0, 1.0].map(|k| v.to.1.atan2(v.to.0) + k * 250.0 / r))
            .map(|a| (a, plants_band(&s, a, 190.0, 330.0).len()))
            .max_by_key(|p| p.1)
            .unwrap();
        assert!(near > 300, "plants round the blast ({near} cells)");
        let rot0 = s.life.as_ref().map_or(0, |l| l.counts().2);
        let g = s.world.planet.surface_radius(a.cos(), a.sin());
        let depth = s.test_blast_depth(10.0);
        s.test_blast_at((a.cos() * g, a.sin() * g), 10.0, depth, false);
        let tl = s.blast_timelines()[0];
        let rep = s.air_last.expect("an air blast on a body with air");
        let born = s.time();
        // The band past the ground's jump, out to where trees go down
        // (30 kPa: more than half of them).
        let (x0, x1) = (tl.spall_m + 5.0, rep.reach(30.0));
        assert!(x1 > x0 + 50.0 && rep.plant_m > x1, "trees down from {x0:.0} m to {x1:.0} m, plants stripped to {:.0} m", rep.plant_m);
        let band = plants_band(&s, a, x0, x1);
        assert!(band.len() > 200, "{} plant cells from {x0:.0} to {x1:.0} m", band.len());
        let mut t = 0;
        while s.blasts_pending() > 0 || t < 100 {
            s.step();
            t += 1;
            assert!(t < 600, "the blast ran its course within a minute");
            // Nothing in the band goes before the front gets there.
            if s.time() + s.eco.tick_s < born + rep.t_at(x0) {
                assert_eq!(plants_band(&s, a, x0, x1).len(), band.len(), "tick {t}: plants went before the air blast got to them");
            }
            if t % 10 == 0 {
                assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
                assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
            }
        }
        assert_eq!(s.bodies.hopping(), 0, "all that was thrown is back down");
        let rep = s.air_last.unwrap();
        let after = plants_band(&s, a, x0, x1);
        let rot = s.life.as_ref().map_or(0, |l| l.counts().2);
        println!("seen {seen}: {rep:?}; {} of {} plant cells left standing in the band, {} more dying", after.len(), band.len(), rot as i64 - rot0 as i64);
        assert!(rep.animals_flung + rep.animals_killed >= 1, "the animal 250 m out is thrown or killed");
        if seen {
            assert!(rep.blown > 0 && rep.plant_cells as usize > band.len() / 2, "{} plants blown down, {} cells thrown", rep.blown, rep.plant_cells);
            assert!(after.len() < band.len() / 2, "{} of {} plant cells left standing", after.len(), band.len());
            assert!(rot >= rot0 + (band.len() - after.len()) / 2, "thrown and landed, they are dying");
            assert_eq!(hanging(&s, &plants_band(&s, a, x0 - 20.0, rep.plant_m)), None, "a plant cell left hanging");
            // The base surge, from when the dome burst.
            let sg = s.surges.first().copied().expect("a base surge");
            assert!((sg.t - (born + tl.t_vent)).abs() < 1e-9 && sg.reach_m > 60.0 && sg.life_s >= 60.0, "{sg:?}");
        } else {
            // Out of sight: killed where they stand, nothing thrown.
            assert!(rep.killed_in_place > 0 && rep.plant_cells == 0 && rep.stripped == 0, "{rep:?}");
            assert!(after.len() + 20 >= band.len(), "{} of {} plant cells still there", after.len(), band.len());
            assert!(rot > rot0 + band.len() / 4, "killed where they stand, they are dying");
        }
        assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
        assert!(air_off(&s) < 1e-6 * initial, "the air is off by {} kg", air_off(&s));
    }
}

/// On a body with air, a blast's air blast blows the swarm's machines away
/// as its front gets to each (Benjamin, 27 Nov): cutters and haulers 300 m
/// out are thrown away from the blast, as far as the wind behind the front
/// carries them in a third of a second, for the screen to draw them flying
/// back; none is told to the screen before the front reaches it. A 300 m
/// out, nothing is strong enough to wreck a cutter.
#[test]
fn an_air_blast_blows_the_swarms_machines_away() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "dev-planet").unwrap();
    let p = dft_sim::planet::Planet::new(base.spec.clone(), base.mats.clone()).unwrap();
    let mut eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    eco.seed_swarm.start_harvesters = 3000;
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(3000);
    s.run(40.0);
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    // Where most cutters are (within 60 m of one).
    let mut legs = Vec::new();
    s.drone_legs(&mut legs);
    let now = s.time();
    let pts: Vec<(f64, f64)> = legs.iter().filter(|l| l.state <= 2).map(|l| l.at(now)).collect();
    let crowd = |p: (f64, f64)| pts.iter().filter(|q| (q.0 - p.0).hypot(q.1 - p.1) < 60.0).count();
    let p0 = pts.iter().step_by(10).copied().max_by_key(|&p| crowd(p)).unwrap();
    assert!(crowd(p0) > 50, "a flock at work ({} cutters)", crowd(p0));
    // A blast 300 m along the ground from it.
    let r = s.world.planet.radius;
    let a = p0.1.atan2(p0.0) + 300.0 / r;
    let g = s.world.planet.surface_radius(a.cos(), a.sin());
    let depth = s.test_blast_depth(10.0);
    s.pushes.clear();
    s.test_blast_at((a.cos() * g, a.sin() * g), 10.0, depth, false);
    let rep = s.air_last.unwrap();
    let born = s.time();
    let mut seen = 0;
    let mut t = 0;
    while s.blasts_pending() > 0 || t < 50 {
        s.step();
        t += 1;
        assert!(t < 600, "the blast ran its course within a minute");
        // Each told to the screen once the front has got to it, and soon
        // after.
        for p in &s.pushes[seen..] {
            assert!(p.t <= s.time() + 1e-9 && p.t >= s.time() - 0.5, "tick {t}: one thrown at {} s told at {} s", p.t - born, s.time() - born);
        }
        seen = s.pushes.len();
        if t % 10 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
        }
    }
    let rep2 = s.air_last.unwrap();
    let cutters: Vec<_> = s.pushes.iter().filter(|p| p.kind == 0).collect();
    let haulers = s.pushes.iter().filter(|p| p.kind == 1).count();
    println!("{rep2:?}; {} cutters and {haulers} haulers thrown on screen", cutters.len());
    assert!(cutters.len() > 50 && rep2.pushed as usize >= s.pushes.len(), "{} cutters thrown", cutters.len());
    assert!(haulers >= 1, "the flock's hauler is thrown too");
    let src = rep.src;
    let away = cutters.iter().filter(|p| (p.dx as f64) * (p0.0 - src.0) + (p.dy as f64) * (p0.1 - src.1) > 0.0).count();
    assert!(away * 10 >= cutters.len() * 8, "{away} of {} thrown away from the blast", cutters.len());
    for p in &cutters {
        let d = (p.dx as f64).hypot(p.dy as f64);
        assert!((d - p.speed as f64 * 0.3).abs() < 1e-3 && p.speed > 0.0 && p.speed <= 150.0, "thrown {d} m at {} m/s", p.speed);
        assert!(p.t >= born + rep.t0 && p.t <= born + rep.t_at(rep.push_m) + 1e-6);
    }
    assert!(rep2.wrecked * 10 <= rep2.pushed, "{} wrecked of {}", rep2.wrecked, rep2.pushed);
    assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
}

/// Test blasts on the surface and in the air over it (sappers bury theirs,
/// but the routine takes any depth) run their course and keep the books; on
/// the surface and over it the dome bursts as the shock reaches the ground,
/// and the screen draws a blast in the open.
#[test]
fn surface_and_air_test_blasts_run_their_course_and_keep_the_books() {
    for (depth, conventional) in [(0.0, false), (-20.0, true), (-300.0, false)] {
        let mut s = sim();
        let initial: f64 = s.world.ledger.initial.iter().sum();
        let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
        s.take_booms();
        s.test_blast_at((0.5, -(s.world.planet.max_radius + 5.0)), 10.0, depth, conventional);
        let tl = s.blast_timelines()[0];
        assert!((tl.depth_m - depth).abs() < 3.0, "{} m deep (put at {depth} m)", tl.depth_m);
        assert!(tl.t_surface < 1e-3 && tl.t_vent < 0.05, "no air, no dome: the shock {} s on, the vent {} s", tl.t_surface, tl.t_vent);
        let boom = s.take_booms()[0];
        assert!(!boom.buried, "{depth} m: a blast in the open");
        // This body has no air: no fireball and no air blast; a nuclear
        // charge still flashes.
        assert_eq!(boom.fireball_m, 0.0, "{depth} m: no fireball with no air");
        assert!(s.air_last.is_none(), "{depth} m: no air blast with no air");
        assert_eq!(s.flash_last.is_some(), !conventional, "{depth} m: a nuclear charge's flash");
        // On the ground a shallow bowl, by the table (see below); smaller
        // over it; none too high for its fireball to reach the ground.
        if depth == -300.0 {
            assert_eq!(tl.kind, CraterKind::Air);
        } else {
            assert!(tl.kind == CraterKind::Crater && tl.bowl_m > 1.0);
        }
        let mut t = 0;
        while s.blasts_pending() > 0 {
            s.step();
            t += 1;
            assert!(t < 600, "{depth} m: the blast ran its course within a minute");
            if t % 5 == 0 {
                assert!(off(&s) < 1e-6 * initial, "{depth} m, tick {t}: the books are off by {} kg", off(&s));
                assert!(air_off(&s) < 1e-6 * initial, "{depth} m, tick {t}: the air is off by {} kg", air_off(&s));
            }
        }
        for _ in 0..100 {
            s.step();
        }
        assert!(s.pushes.is_empty() && s.surges.is_empty(), "{depth} m: nothing thrown by air and no base surge with no air");
        if depth == 0.0 {
            assert!((s.last_crater.1 - tl.bowl_d).abs() < 1e-9 && (11.0..13.0).contains(&tl.bowl_d) && s.last_crater.0 > 300, "a bowl {} m deep planned ({} cells), {} m by the table", s.last_crater.1, s.last_crater.0, tl.bowl_d);
        }
        if depth == -300.0 {
            // Too high: no rock broken, nothing thrown.
            assert_eq!((s.cracked_cells().count(), s.loose_cells().count(), s.last_crater.0), (0, 0, 0), "{depth} m: ground broken");
        }
        assert!(off(&s) < 1e-6 * initial, "{depth} m: the books are off by {} kg", off(&s));
        assert!(air_off(&s) < 1e-6 * initial, "{depth} m: the air is off by {} kg", air_off(&s));
    }
}

/// A nuclear blast on the ground on a body with air (the dev planet with its
/// life on) has a fireball that vaporises the plants and animals in it, and a
/// flash of heat that sets plants in sight alight out past it and burns
/// animals to death (docs/explosion-sequence.md, "A blast in the open"); the
/// swarm is not harmed by it. A conventional charge of the same yield does
/// neither by heat (its air blast still does its work). The books balance
/// throughout.
#[test]
fn a_nuclear_blast_in_the_open_burns_what_is_in_sight_and_a_conventional_one_does_not() {
    for conventional in [false, true] {
        let mut s = dev_life();
        let initial: f64 = s.world.ledger.initial.iter().sum();
        let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
        // 120 m along from an animal (in the fireball), where the plants
        // within 400 m are thickest.
        let views = s.animal_views();
        let r = s.world.planet.radius;
        let (a, near) = views
            .iter()
            .step_by(4)
            .take(30)
            .flat_map(|v| [-1.0, 1.0].map(|k| v.to.1.atan2(v.to.0) + k * 120.0 / r))
            .map(|a| (a, plants_band(&s, a, 0.0, 400.0).len()))
            .max_by_key(|p| p.1)
            .unwrap();
        assert!(near > 300, "plants round the blast ({near} cells)");
        let (_, _, lost0) = s.fauna_totals();
        let g = s.world.planet.surface_radius(a.cos(), a.sin());
        s.take_booms();
        s.test_blast_at((a.cos() * g, a.sin() * g), 10.0, 0.0, conventional);
        let tl = s.blast_timelines()[0];
        let boom = s.take_booms()[0];
        assert!(!boom.buried && boom.fireball_m > 0.0 && (boom.fireball_m as f64 - tl.fireball_m).abs() < 1e-3, "{boom:?}");
        assert!(s.air_last.is_some(), "an air blast on a body with air");
        let mut t = 0;
        while s.blasts_pending() > 0 || t < 50 {
            s.step();
            t += 1;
            assert!(t < 900, "the blast ran its course");
            if t % 10 == 0 {
                assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
                assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
            }
        }
        let (_, _, lost) = s.fauna_totals();
        if conventional {
            assert!(s.flash_last.is_none(), "a conventional charge sends out no heat pulse");
            assert!(tl.fireball_m < 80.0 && tl.thermal_q == 0.0, "{tl:?}");
        } else {
            let f = s.flash_last.expect("a nuclear blast in the open flashes");
            println!("{f:?}; {} animals lost", lost - lost0);
            assert!(f.done, "its flash ran its course");
            assert!(f.vaporised > 100, "{} plant cells vaporised in its fireball", f.vaporised);
            assert!(f.glazed > 100, "{} ground cells glazed under its fireball", f.glazed);
            assert!(f.lit + f.lit_unseen > 0, "plants in sight out past its fireball set alight");
            assert!(f.animals_vaporised + f.animals_burned + f.animals_irradiated >= 1 && lost > lost0, "the animal in its fireball is gone");
            // Next to nothing left standing in its fireball: no plant cell
            // open to the sky, but a few that rubble thrown early lay over
            // when it got there (the roots, and what the rubble still
            // covers, it doesn't reach).
            let open_sky = |c: &(i64, i64)| {
                let p = (c.0 as f64 + 0.5, c.1 as f64 + 0.5);
                let (u, r0) = ((p.0 / p.0.hypot(p.1), p.1 / p.0.hypot(p.1)), p.0.hypot(p.1));
                (1..150).all(|k| {
                    let q = ((u.0 * (r0 + k as f64)).floor() as i64, (u.1 * (r0 + k as f64)).floor() as i64);
                    s.world.material(q.0, q.1) == VACUUM || is_plant(&s, q)
                })
            };
            let left = plants_band(&s, a, 0.0, 0.8 * f.ground_m).into_iter().filter(open_sky).count();
            assert!(left * 20 < f.vaporised as usize, "{left} plant cells left open to the sky in the fireball, {} vaporised", f.vaporised);
        }
        assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
        assert!(air_off(&s) < 1e-6 * initial, "the air is off by {} kg", air_off(&s));
    }
}

/// A big blast's crater (some thousands of cells) is thrown in clumps of
/// several cells a piece, and the books still balance cell by cell: while
/// the clumps fly, when they land as small piles, through a save and load
/// with them in the air, and when the view leaves them and they are set
/// down (or lost to space) a tick's worth at a time.
#[test]
fn a_big_crater_is_thrown_in_clumps_and_keeps_the_books() {
    let mut s = sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), 300.0);
    let (mut most, mut cells, mut seen) = (0, 0, 0);
    for t in 0..400 {
        s.step();
        // (The crater's own pieces, not the ground round it kicked up.)
        most = most.max(s.bodies.drops() - s.bodies.hopping());
        cells = cells.max(s.last_crater.0);
        if t % 5 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
        // Saved and loaded with the crater in the air: it is all still
        // there, booked as before.
        if t == 150 {
            let n = s.bodies.drops();
            assert!(n > 0, "some of the crater in the air to save");
            s = save_and_load(&mut s);
            assert_eq!(s.bodies.drops(), n, "the pieces in the air came back");
            assert!(off(&s) < 1e-6 * initial, "loaded: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "loaded: the air is off by {} kg", air_off(&s));
        }
        // Half a minute on, the view goes elsewhere: what is still in the
        // air comes down where it would land, or is lost if it would never,
        // a few score a tick.
        if t == 300 {
            seen = s.bodies.drops();
            assert!(seen > 0, "some of the crater still in the air");
            s.set_attention(Some(None));
        }
        if t == 301 {
            assert!(s.bodies.drops() + 120 >= seen, "at most 120 set down a tick ({seen} to {})", s.bodies.drops());
        }
    }
    assert!(cells > 4000, "a big crater ({cells} cells)");
    assert!(most > 0 && most * 3 < cells, "{cells} cells flew as at most {most} pieces (clumps of several)");
    assert_eq!(s.bodies.drops(), 0, "nothing left in the air out of sight");
    assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
    let aloft: f64 = s.world.ledger.aloft.iter().sum();
    assert!(aloft.abs() < 1e-6 * initial, "{aloft} kg still booked as in the air");
}

/// In hard rock a blast holds some of the rock together in big pieces. On
/// screen, a few of those lying open to its crater are thrown out whole as
/// tumbling rock bodies, and the books still balance: while they fly, when
/// they come down or leave, and through a save with them in the air. A
/// blast finished at once (as a save does) throws none: they stay held.
#[test]
fn held_pieces_are_thrown_out_whole_and_keep_the_books() {
    let mut s = hard_sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let over = (0.5, -(s.world.planet.max_radius + 5.0));
    s.test_blast(over, 10.0);
    let (mut t, mut most) = (0, 0);
    while s.blasts_pending() > 0 {
        s.step();
        t += 1;
        assert!(t < 600, "the blast ran its course within a minute");
        most = most.max(s.bodies.count());
        if t % 3 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    let (held, thrown, cells) = s.last_chunks;
    assert!(held > thrown, "{held} pieces held");
    assert!(thrown > 0 && thrown <= 24, "{thrown} of them thrown out whole");
    assert!(cells >= 8 * thrown, "{cells} cells in the {thrown} thrown");
    assert!(most > 0, "rock bodies in the air");
    // Saved and loaded with them in the air (a save loses rock in the air,
    // booked as lost), and on until they are all down or gone.
    assert!(s.bodies.count() > 0, "still in the air as the blast ends");
    let mut s = save_and_load(&mut s);
    assert!(off(&s) < 1e-6 * initial, "loaded: the books are off by {} kg", off(&s));
    assert!(air_off(&s) < 1e-6 * initial, "loaded: the air is off by {} kg", air_off(&s));
    for _ in 0..100 {
        s.step();
    }
    assert!(off(&s) < 1e-6 * initial, "loaded, after: the books are off by {} kg", off(&s));
    // Not saved: they fly on, and when the view goes elsewhere what is
    // still in the air comes down where it would land (or is lost).
    let mut s = hard_sim();
    s.test_blast(over, 10.0);
    for t in 0..600 {
        s.step();
        if t == 300 {
            s.set_attention(Some(None));
        }
        if t % 10 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    assert_eq!(s.bodies.count(), 0, "no rock bodies left in the air out of sight");
    let aloft: f64 = s.world.ledger.aloft.iter().sum();
    assert!(aloft.abs() < 1e-6 * initial, "{aloft} kg still booked as in the air");
    assert!(off(&s) < 1e-6 * initial, "the books are off by {} kg", off(&s));
    // Saved a few ticks in (the blast finished at once): the books balance.
    let mut s = hard_sim();
    s.test_blast(over, 10.0);
    for _ in 0..3 {
        s.step();
    }
    let mut s = save_and_load(&mut s);
    assert_eq!(s.blasts_pending(), 0);
    assert!(off(&s) < 1e-6 * initial, "finished at once: the books are off by {} kg", off(&s));
    for _ in 0..100 {
        s.step();
    }
    assert!(off(&s) < 1e-6 * initial, "finished at once, after: the books are off by {} kg", off(&s));
    assert!(air_off(&s) < 1e-6 * initial, "finished at once, after: the air is off by {} kg", air_off(&s));
}

/// Whether these cells are all joined up, side to side.
fn joined(cells: &[(i64, i64)]) -> bool {
    let all: std::collections::HashSet<(i64, i64)> = cells.iter().copied().collect();
    let mut seen = std::collections::HashSet::new();
    let mut todo = vec![cells[0]];
    seen.insert(cells[0]);
    while let Some(c) = todo.pop() {
        for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + d.0, c.1 + d.1);
            if all.contains(&n) && seen.insert(n) {
                todo.push(n);
            }
        }
    }
    seen.len() == all.len()
}

/// A held piece that a sapper's shaft runs through is in two parts: each
/// part big enough flies as a rock body of its own, joined up (it flew as
/// one body with a 7 m gap through it), and the books balance.
#[test]
fn held_pieces_cut_by_a_shaft_fly_joined_up() {
    let mut s = hard_sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let over = (0.5, -(s.world.planet.max_radius + 5.0));
    // Shafts 7 m wide from the ground down to the charge's depth, as a
    // sapper bores one (dug out and booked as mined), through the middle of
    // every column of the squares the loosened column holds rock in (14 m
    // across in this rock, from x = 0) across the crater: every piece in it
    // is cut in two.
    let depth = s.eco.units.sapper.depth_m_per_cbrt_kt * 10f64.cbrt();
    for x in (-3..=2).flat_map(|k| 4 + 14 * k..=10 + 14 * k) {
        let mut y = over.1 as i64;
        while s.world.material(x, y) == VACUUM {
            y += 1;
        }
        for y in y..y + depth as i64 {
            if let Some(m) = s.world.lift_cell(x, y) {
                s.world.mine_aloft(m);
            }
        }
    }
    s.test_blast(over, 10.0);
    let (mut t, mut beside) = (0, 0);
    while s.blasts_pending() > 0 || t < 100 {
        s.step();
        t += 1;
        assert!(t < 700, "the blast ran its course within a minute");
        // (A tumbling body's cells all turn with it; slabs and drops don't
        // turn. A body that has hardly turned is left out: its angle, worked
        // out in single precision from two of its points some 1,450 m from
        // the planet's middle, comes out in steps of 0.00012, and two such
        // bodies can share one, as two of them did on 27 Nov.)
        let mut by: std::collections::HashMap<u32, Vec<(i64, i64)>> = Default::default();
        for a in s.bodies.drawn() {
            if a.angle.abs() > 0.01 {
                by.entry(a.angle.to_bits()).or_default().push(a.home);
            }
        }
        for (angle, cells) in &by {
            assert!(joined(cells), "tick {t}: a body of {} cells turned {} in pieces: {cells:?}", cells.len(), f32::from_bits(*angle));
            // (A part of a cut piece is at most 7 m across.)
            let (x0, x1) = cells.iter().fold((i64::MAX, i64::MIN), |a, c| (a.0.min(c.0), a.1.max(c.0)));
            beside += (x1 - x0 < 7) as usize;
        }
        if t % 3 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    assert!(s.last_chunks.1 > 0, "pieces were thrown out whole");
    assert!(beside > 0, "a part of a cut piece flew");
}

/// Where `test_blast` puts a sapper's charge of `kt` under the top of the far
/// side (straight under the ground there, at the depth for its yield), and
/// its blast radius. Down is +y there, and +x is to one side.
fn charge_at(s: &Sim, kt: f64) -> ((f64, f64), f64) {
    let mut y = -(s.world.planet.max_radius + 5.0) as i64;
    while s.world.material(0, y) == VACUUM {
        y += 1;
    }
    let sp = &s.eco.units.sapper;
    ((0.5, y as f64 + sp.depth_m_per_cbrt_kt * kt.cbrt()), sp.radius_m_per_cbrt_kt * kt.cbrt())
}

/// The cells of rock round a charge at `at` within `half` radians of the
/// direction `dir` (a unit vector), from `d0` to `d1` m out.
fn wedge(s: &Sim, at: (f64, f64), dir: (f64, f64), half: f64, d0: f64, d1: f64) -> Vec<(i64, i64)> {
    let k = d1.ceil() as i64 + 1;
    let (cx, cy) = (at.0.floor() as i64, at.1.floor() as i64);
    let mut out = Vec::new();
    for x in cx - k..=cx + k {
        for y in cy - k..=cy + k {
            let (dx, dy) = (x as f64 + 0.5 - at.0, y as f64 + 0.5 - at.1);
            let d = dx.hypot(dy);
            if d >= d0 && d <= d1 && (dx * dir.0 + dy * dir.1) / d >= half.cos() && s.world.material(x, y) != VACUUM {
                out.push((x, y));
            }
        }
    }
    out
}

/// Of these cells (rock before a blast), how many it broke (left loose, or
/// threw out) and how many more it cracked.
fn hit(s: &Sim, cells: &[(i64, i64)]) -> (usize, usize) {
    let loose: std::collections::HashSet<(i64, i64)> = s.loose_cells().collect();
    let cracked: std::collections::HashSet<(i64, i64)> = s.cracked_cells().collect();
    let broken = |c: &(i64, i64)| loose.contains(c) || s.world.material(c.0, c.1) == VACUUM;
    (cells.iter().filter(|c| broken(c)).count(), cells.iter().filter(|c| !broken(c) && cracked.contains(c)).count())
}

/// A 10 kt blast with an empty pocket 6 m across dug out (and booked as
/// mined) 17 m from its charge, 65 degrees below level to one side. Of the
/// rock in a wedge behind the pocket (as seen from the charge, from just
/// past it to 3 blast radii out), and of the rock in the same wedge mirrored
/// to the other side: how many cells the blast broke, and how many more it
/// cracked. The books are checked as it goes off.
fn cave_shadow() -> ((usize, usize), (usize, usize)) {
    let mut s = sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let (at, r) = charge_at(&s, 10.0);
    let a = 65f64.to_radians();
    let (dir, mirror) = ((a.cos(), a.sin()), (-a.cos(), a.sin()));
    let mid = ((at.0 + dir.0 * 17.0).floor() as i64, (at.1 + dir.1 * 17.0).floor() as i64);
    for x in mid.0 - 3..mid.0 + 3 {
        for y in mid.1 - 3..mid.1 + 3 {
            if let Some(m) = s.world.lift_cell(x, y) {
                s.world.mine_aloft(m);
            }
        }
    }
    // (The pocket reaches 21 m out at its far corner, and seen from the
    // charge it is 20 degrees across.)
    let behind = wedge(&s, at, dir, 8f64.to_radians(), 22.0, 3.0 * r);
    let other = wedge(&s, at, mirror, 8f64.to_radians(), 22.0, 3.0 * r);
    s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), 10.0);
    let mut t = 0;
    while s.blasts_pending() > 0 {
        s.step();
        t += 1;
        assert!(t < 600, "the blast ran its course within a minute");
        if t % 3 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    (hit(&s, &behind), hit(&s, &other))
}

/// A cave shields the rock behind it from a blast: in the pocket's shadow
/// far less rock is broken or cracked than in the same place on the other
/// side, where there is no pocket. (Before the shock's field, rays crossed
/// the pocket cheaply and broke more behind it: 321 cells broken and 336
/// cracked, to 287 and 233 on the other side.)
#[test]
fn a_cave_shields_the_rock_behind_it_from_a_blast() {
    let ((bb, bc), (ob, oc)) = cave_shadow();
    assert!(ob > 50 && oc > 50, "the blast broke {ob} cells and cracked {oc} in the wedge with no pocket");
    assert!(bb * 3 < ob, "broken behind the pocket: {bb}, on the other side {ob}");
    assert!(bc * 2 < oc, "cracked behind the pocket: {bc}, on the other side {oc}");
}

/// A 10 kt blast in hard rock (hardness 8) with a band of soft regolith 6 m
/// thick laid level through its charge (the rock dug out and booked as
/// mined, the regolith brought in from outside): how far the broken rock
/// reaches from the charge sideways (within 10 m of the band's middle, past
/// the loosened column) and straight down (within 10 m either side, under
/// the column), and how many cells of the hard rock flanking the band (3 to
/// 12 m from its middle, from past the column to 4 radii out) the blast broke,
/// and how many more it cracked. The books are checked as it goes off.
fn soft_band() -> (f64, f64, (usize, usize)) {
    let mut s = hard_sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let (at, r) = charge_at(&s, 10.0);
    let soft = s.world.planet.mats.id("regolith").unwrap();
    let (cx, cy) = (at.0.floor() as i64, at.1.floor() as i64);
    for x in cx - 150..=cx + 150 {
        for y in cy - 3..cy + 3 {
            if let Some(m) = s.world.lift_cell(x, y) {
                s.world.mine_aloft(m);
            }
            assert!(s.world.deposit_cell(x, y, soft));
        }
    }
    let k = (4.0 * r) as i64 + 2;
    let mut rock = Vec::new();
    for x in cx - k..=cx + k {
        for y in cy - k..=cy + k {
            if s.world.material(x, y) != VACUUM {
                rock.push((x, y));
            }
        }
    }
    s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), 10.0);
    let mut t = 0;
    while s.blasts_pending() > 0 {
        s.step();
        t += 1;
        assert!(t < 600, "the blast ran its course within a minute");
        if t % 3 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    let loose: std::collections::HashSet<(i64, i64)> = s.loose_cells().collect();
    let broken: Vec<(f64, f64)> = rock.iter().filter(|c| loose.contains(c) || s.world.material(c.0, c.1) == VACUUM).map(|c| (c.0 as f64 + 0.5 - at.0, c.1 as f64 + 0.5 - at.1)).collect();
    let far = |f: &dyn Fn(f64, f64) -> bool| broken.iter().filter(|p| f(p.0, p.1)).map(|p| p.0.hypot(p.1)).fold(0.0, f64::max);
    let side = far(&|dx, dy| dy.abs() <= 10.0 && dx.abs() > 1.15 * r);
    let down = far(&|dx, dy| dx.abs() <= 10.0 && dy > 0.5 * r + 2.0);
    let flank: Vec<(i64, i64)> = rock
        .iter()
        .copied()
        .filter(|c| {
            let (dx, dy) = (c.0 as f64 + 0.5 - at.0, c.1 as f64 + 0.5 - at.1);
            dy.abs() > 3.0 && dy.abs() <= 12.0 && dx.abs() > 1.15 * r && dx.abs() <= 4.0 * r
        })
        .collect();
    (side, down, hit(&s, &flank))
}

/// A soft layer channels a blast: in hard rock, a soft band through the
/// charge lets the broken rock reach clearly further sideways along it than
/// straight down, and the hard rock either side of the band breaks well
/// beyond where it would without the band. (Before the shock's field, only
/// the rays that happened to lie along the band went further: 72 m sideways
/// to 41 m down, and 136 cells broken in the hard rock flanking the band;
/// with it, 71 m to 41, and about 410 cells.)
#[test]
fn a_soft_layer_channels_a_blast() {
    let (side, down, (fb, fc)) = soft_band();
    assert!(side > 1.6 * down, "broken rock reaches {side:.0} m sideways along the band, {down:.0} m down (flanking hard rock: {fb} broken, {fc} cracked)");
    assert!(fb > 300, "{fb} cells broken in the hard rock flanking the band ({fc} more cracked)");
}

/// A 10 kt blast on the test body with a level layer `t` cells thick (open
/// space, or a liquid) dug out (and booked as mined) under its charge, its
/// top `dy` m down and 200 m across: of the rock under the layer, within 3
/// blast radii of the charge, how many cells the blast broke, and how many
/// more it cracked. `t` 0: no layer. The books are checked as it goes off.
fn layer_case(t: i64, dy: f64, liquid: bool) -> (usize, usize) {
    let mut s = sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let (at, r) = charge_at(&s, 10.0);
    let wet = (1..s.world.planet.mats.list.len()).find(|&m| s.world.planet.mats.list[m].state == dft_sim::materials::State::Liquid).unwrap() as dft_sim::materials::MatId;
    let y0 = (at.1 + dy).floor() as i64;
    for x in -100..100 {
        for y in y0..y0 + t {
            if let Some(m) = s.world.lift_cell(x, y) {
                s.world.mine_aloft(m);
            }
            if liquid {
                assert!(s.world.deposit_cell(x, y, wet));
            }
        }
    }
    let k = (3.0 * r) as i64;
    let (cx, cy) = (at.0.floor() as i64, at.1.floor() as i64);
    let mut under = Vec::new();
    for x in cx - k..=cx + k {
        for y in (y0 + t).max(cy - k)..=cy + k {
            if s.world.material(x, y) != VACUUM {
                under.push((x, y));
            }
        }
    }
    s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), 10.0);
    let mut tick = 0;
    while s.blasts_pending() > 0 {
        s.step();
        tick += 1;
        assert!(tick < 600, "the blast ran its course within a minute");
        if tick % 3 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {tick}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {tick}: the air is off by {} kg", air_off(&s));
        }
    }
    hit(&s, &under)
}

/// Only open space about a ninth of a blast radius thick (a node of the
/// shock's field: 3.4 m at 10 kt) or more shields the rock behind it: a
/// crack 1 m wide 13 m under a 10 kt charge barely changes what breaks
/// under it, wherever the field's grid falls on it, one 2 m wide shades
/// only the rock off to the side (the shock crosses it aslant), the same
/// wherever the grid falls, and one 4 m wide shields what lies under it.
/// (Measured: with no crack 2,704 cells broken under 12.8 m; behind a 1 m
/// crack 2,693 to 2,789 by where it lies, a 2 m one 2,178 to 2,245, a 4 m
/// one 24 to 28. Looked at one cell to a node, a 1 m crack a node fell on
/// shielded all of it, and one 2 m wide all or none by where it lay.)
#[test]
fn only_a_thick_gap_shields_the_rock_behind_it_from_a_blast() {
    let (none, _) = layer_case(0, 12.8, false);
    let (a, _) = layer_case(1, 12.0, false);
    let (b, _) = layer_case(1, 12.8, false);
    assert!(a as f64 > 0.9 * none as f64 && b as f64 > 0.9 * none as f64, "broken under a 1 m crack: {a} and {b}; with none, {none}");
    let (c, _) = layer_case(2, 12.0, false);
    let (d, _) = layer_case(2, 12.8, false);
    assert!(c.abs_diff(d) as f64 <= 0.1 * c.max(d) as f64, "broken under a 2 m crack: {c} and {d}, by where it lies");
    assert!(c.min(d) as f64 > 0.6 * none as f64, "broken under a 2 m crack: {c} and {d}; with none, {none}");
    let (e, _) = layer_case(4, 12.8, false);
    assert!((e as f64) < 0.1 * none as f64, "broken under a 4 m gap: {e}; with none, {none}");
}

/// A liquid carries a blast's shock (water and magma carry one well): rock
/// under a seam of liquid 4 m thick breaks about as it would with no seam.
/// (Measured: 2,946 cells broken under it against 2,704 with none; the shock
/// runs cheaply along the seam.)
#[test]
fn a_liquid_seam_does_not_shield_the_rock_behind_it() {
    let (none, _) = layer_case(0, 12.8, true);
    let (wet, _) = layer_case(4, 12.8, true);
    assert!(wet as f64 > 0.9 * none as f64, "broken under a 4 m seam of liquid: {wet}; with none, {none}");
}

/// A tunneler's end blast (50 kt, no loosened column) goes off in its own
/// bore, 12 m wide: the bore carries the shock as rock would, so the rock
/// beside it (6 to 20 m from the bore's middle, 25 to 90 m from the charge)
/// breaks at least about as much as with no bore. (Measured: 3,467 cells
/// broken beside the bore against 3,339 with none; with the bore taken for
/// a cave, 2,069.) The books are checked as it goes off.
#[test]
fn a_tunnelers_own_bore_does_not_shield_its_blast() {
    let case = |dig: bool| {
        let mut s = sim();
        let initial: f64 = s.world.ledger.initial.iter().sum();
        let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
        let mut y = -(s.world.planet.max_radius + 5.0) as i64;
        while s.world.material(0, y) == VACUUM {
            y += 1;
        }
        let at = (0.5, y as f64 + 100.0);
        let bottom = at.1 + 80.0;
        if dig {
            for x in -6..6 {
                for yy in y - 5..bottom as i64 {
                    if let Some(m) = s.world.lift_cell(x, yy) {
                        s.world.mine_aloft(m);
                    }
                }
            }
        }
        let mut beside = Vec::new();
        for x in -21..=21i64 {
            for yy in (at.1 - 91.0) as i64..=(at.1 + 91.0) as i64 {
                let (dx, dy) = (x as f64 + 0.5 - at.0, yy as f64 + 0.5 - at.1);
                let d = dx.hypot(dy);
                if (6.0..=20.0).contains(&(x as f64 + 0.5).abs()) && (25.0..=90.0).contains(&d) && s.world.material(x, yy) != VACUUM {
                    beside.push((x, yy));
                }
            }
        }
        let line = [(0.5, y as f64 - 5.0), at, (0.5, bottom)];
        s.test_bore_blast(at, 50.0, if dig { &line } else { &[] }, 6.0);
        let mut tick = 0;
        while s.blasts_pending() > 0 {
            s.step();
            tick += 1;
            assert!(tick < 1200, "the blast ran its course within two minutes");
            if tick % 3 == 0 {
                assert!(off(&s) < 1e-6 * initial, "tick {tick}: the books are off by {} kg", off(&s));
                assert!(air_off(&s) < 1e-6 * initial, "tick {tick}: the air is off by {} kg", air_off(&s));
            }
        }
        hit(&s, &beside).0
    };
    let (none, bore) = (case(false), case(true));
    assert!(bore as f64 > 0.9 * none as f64, "broken beside the bore: {bore}; with no bore, {none}");
}

/// The 5 km dev planet, a whole disc of rock under its sediments: on the 3 km
/// test body a big blast's charge (530 m down at 3,000 kt, 790 m at 10,000)
/// lies in the open space under a lump of ground the body's great impact
/// basins left, and from 1,000 kt up it is not a blast in plain rock.
fn dev_sim() -> Sim {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, "dev-planet").unwrap();
    let p = dft_sim::planet::Planet::new(base.spec.clone(), base.mats.clone()).unwrap();
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let mut s = Sim::new(World::new(p), eco);
    s.free_drones = Some(0);
    s
}

/// The shock's field, as the yield grows (run by hand: cargo test --release
/// --test sapper_blast -- --ignored --nocapture blast_field): its nodes, the
/// work it took and how long, how many ticks the blast took to run its
/// course, its work in all and in its dearest tick; on the dev planet, where
/// the bigger charges lie in plain rock, also how many cells were left loose
/// and how many cracked. Then the counts of the two tests above: the rock
/// behind a cave and on the other side, and the soft band's reach.
#[test]
#[ignore]
fn blast_field() {
    for (dev, kt) in [(false, 10.0), (false, 1000.0), (false, 10000.0), (true, 1000.0), (true, 10000.0)] {
        let mut s = if dev { dev_sim() } else { sim() };
        s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), kt);
        let (mut ticks, mut most) = (0, 0.0f64);
        while s.blasts_pending() > 0 {
            let b = s.prof2[7];
            s.step();
            most = most.max(s.prof2[7] - b);
            ticks += 1;
        }
        let (nodes, work, secs) = s.last_field;
        eprintln!(
            "{}{kt} kt: field of {nodes} nodes, {work} work, {:.1} ms; {ticks} ticks ({:.1} game s), blast work {:.0} ms in all, at most {:.1} ms a tick; {} cells loose, {} cracked",
            if dev { "dev planet, " } else { "" },
            secs * 1000.0,
            ticks as f64 * s.eco.tick_s,
            s.prof2[7] * 1000.0,
            most * 1000.0,
            s.loose_cells().count(),
            s.cracked_cells().count(),
        );
    }
    let ((bb, bc), (ob, oc)) = cave_shadow();
    eprintln!("10 kt by a cave: behind it {bb} cells broken, {bc} cracked; on the other side {ob} broken, {oc} cracked");
    let (side, down, (fb, fc)) = soft_band();
    eprintln!("10 kt in hard rock with a soft band: broken {side:.0} m sideways, {down:.0} m down; in the hard rock flanking the band {fb} broken, {fc} cracked");
}

/// A buried charge with no loosened column (as a tunneler's end, or the old
/// depth rule), `kt` under the ground at the top of the far side: where the
/// charge is, and its blast radius.
fn columnless_blast(s: &mut Sim, kt: f64) -> (f64, f64) {
    s.eco.units.sapper.depth_m_per_cbrt_kt = 0.0;
    let over = (0.5, -(s.world.planet.max_radius + 5.0));
    let mut y = over.1 as i64;
    while s.world.material(0, y) == VACUUM {
        y += 1;
    }
    let sp = &s.eco.units.sapper;
    let r = sp.radius_m_per_cbrt_kt * kt.cbrt();
    let at = (0.5, y as f64 + r + sp.bury_m);
    s.test_blast(over, kt);
    (at.1, r)
}

/// The highest loose cell over a charge at (0.5, `at_y`) (m over it),
/// within `w` m either side of straight up.
fn loose_top(s: &Sim, at_y: f64, w: f64) -> f64 {
    s.loose_cells().filter(|c| (c.0 as f64).abs() < w).map(|c| at_y - (c.1 as f64 + 0.5)).fold(0.0, f64::max)
}

/// With no loosened column, the roof of the blast's cavity falls in a
/// moment after the blast: a chimney of rubble rises well over the broken
/// ground, and the books still balance, while it falls in and when a save
/// finishes the blast at once.
#[test]
fn a_blast_with_no_column_leaves_a_rubble_chimney_and_keeps_the_books() {
    let mut s = sim();
    let initial: f64 = s.world.ledger.initial.iter().sum();
    let off = |s: &Sim| s.world.ledger.imbalance(&s.world.remaining_kg());
    let (at_y, r) = columnless_blast(&mut s, 10.0);
    // Just before the roof falls in (0.9 s on): what the rays broke.
    let (mut t, mut broken, mut cells) = (0, 0.0, 0);
    while s.blasts_pending() > 0 {
        s.step();
        t += 1;
        assert!(t < 600, "the blast ran its course within a minute");
        if t == 9 {
            broken = loose_top(&s, at_y, 0.25 * r);
            cells = s.loose_cells().count();
        }
        if t % 3 == 0 {
            assert!(off(&s) < 1e-6 * initial, "tick {t}: the books are off by {} kg", off(&s));
            assert!(air_off(&s) < 1e-6 * initial, "tick {t}: the air is off by {} kg", air_off(&s));
        }
    }
    assert!(t > 15, "the roof fell in after the shock ({t} ticks)");
    let top = loose_top(&s, at_y, 0.25 * r);
    assert!(top > broken + 0.3 * r && top < 2.6 * r, "the chimney rises to {:.2} radii over the charge, from {:.2} before the roof fell", top / r, broken / r);
    let more = s.loose_cells().count() - cells;
    assert!(more > 200, "{more} cells fell in");
    for _ in 0..100 {
        s.step();
    }
    assert!(off(&s) < 1e-6 * initial, "after: the books are off by {} kg", off(&s));
    // Finished at once a few ticks in, as a save does: the chimney too.
    let mut s = sim();
    let (at_y, r) = columnless_blast(&mut s, 10.0);
    for _ in 0..3 {
        s.step();
    }
    let mut s = save_and_load(&mut s);
    assert_eq!(s.blasts_pending(), 0);
    let top2 = loose_top(&s, at_y, 0.25 * r);
    assert!(top2 > broken + 0.3 * r, "finished at once, the chimney rises to {:.2} radii", top2 / r);
    assert!(off(&s) < 1e-6 * initial, "loaded: the books are off by {} kg", off(&s));
    for _ in 0..100 {
        s.step();
    }
    assert!(off(&s) < 1e-6 * initial, "loaded, after: the books are off by {} kg", off(&s));
}

/// A blast with no loosened column, as the yield grows (run by hand: cargo
/// test --release --test sapper_blast -- --ignored --nocapture): how many
/// ticks it took to run its course, its work in all and in its dearest
/// tick; the loose cells just before the roof falls in (0.9 s on) and at
/// the end; and how high the loose ground reaches straight over the charge
/// (within a quarter radius of its middle), then and at the end, in m/kt^1/3
/// and in blast radii.
#[test]
#[ignore]
fn blast_chimney_reach() {
    for kt in [10.0, 50.0, 1000.0, 10000.0] {
        let mut s = sim();
        let (at_y, r) = columnless_blast(&mut s, kt);
        let (mut ticks, mut most) = (0, 0.0f64);
        let (mut early, mut early_top) = (0, 0.0);
        while s.blasts_pending() > 0 {
            let b = s.prof2[7];
            s.step();
            most = most.max(s.prof2[7] - b);
            ticks += 1;
            if ticks == 9 {
                early = s.loose_cells().count();
                early_top = loose_top(&s, at_y, 0.25 * r);
            }
        }
        let top = loose_top(&s, at_y, 0.25 * r);
        let k = kt.cbrt();
        eprintln!(
            "{kt} kt (radius {r:.0} m): {ticks} ticks ({:.1} game s), blast work {:.0} ms in all, at most {:.1} ms a tick; loose cells {early} at 0.9 s, {} at the end; loose straight over the charge to {:.0} m/kt^1/3 ({:.2} radii) at 0.9 s, {:.0} m/kt^1/3 ({:.2} radii) at the end",
            ticks as f64 * s.eco.tick_s,
            s.prof2[7] * 1000.0,
            most * 1000.0,
            s.loose_cells().count(),
            early_top / k,
            early_top / r,
            top / k,
            top / r,
        );
    }
}

/// How far and how thickly a blast's shock cracks the rock as the yield
/// grows (run by hand: cargo test --release --test sapper_blast -- --ignored
/// --nocapture): how many ticks the blast took to run its course and the
/// most blast work in one of them, how many cells it cracked and of those
/// how many show as cracks, and how far out they reach from the charge, in
/// m/kt^1/3, the way the physics measures it (the 90th percentile and the
/// farthest; all of them, and those below the charge only, since over it
/// the cracks beside the loosened column run on up to the ground).
#[test]
#[ignore]
fn blast_crack_reach() {
    for kt in [10.0, 1000.0, 10000.0] {
        let mut s = sim();
        let over = (0.5, -(s.world.planet.max_radius + 5.0));
        let mut y = over.1 as i64;
        while s.world.material(0, y) == VACUUM {
            y += 1;
        }
        // (The charge, straight under where the ground was found.)
        let sp = &s.eco.units.sapper;
        let at = (0.5, y as f64 + sp.depth_m_per_cbrt_kt * f64::cbrt(kt));
        s.test_blast(over, kt);
        let (mut ticks, mut most) = (0, 0.0f64);
        while s.blasts_pending() > 0 {
            let b = s.prof2[7];
            s.step();
            most = most.max(s.prof2[7] - b);
            ticks += 1;
        }
        let reach = |deep: bool| {
            let mut d: Vec<f64> = s.cracked_cells().filter(|c| !deep || c.1 as f64 + 0.5 > at.1).map(|c| (c.0 as f64 + 0.5 - at.0).hypot(c.1 as f64 + 0.5 - at.1) / kt.cbrt()).collect();
            d.sort_by(|a, b| a.total_cmp(b));
            (d.get(d.len() * 9 / 10).copied().unwrap_or(0.0), d.last().copied().unwrap_or(0.0))
        };
        let (all, deep) = (reach(false), reach(true));
        let shown = s.cracked_cells().filter(|c| s.world.look(c.0, c.1) == dft_sim::world::Look::CRACKED).count();
        eprintln!(
            "{kt} kt: {ticks} ticks ({:.1} game s), blast work {:.0} ms in all, at most {:.1} ms a tick; {} cells cracked, {shown} shown as cracks; reach {:.0} m/kt^1/3 (90%), {:.0} (farthest); below the charge {:.0} (90%), {:.0} (farthest)",
            ticks as f64 * s.eco.tick_s,
            s.prof2[7] * 1000.0,
            most * 1000.0,
            s.cracked_cells().count(),
            all.0,
            all.1,
            deep.0,
            deep.1,
        );
    }
}

/// How a blast's work is spread over the ticks as the yield grows (run by
/// hand: cargo test --release --test sapper_blast -- --ignored --nocapture):
/// how many ticks the blast took to run its course; the dearest single tick
/// over the blast and the minute after it (the whole step, and in it the
/// blast's own work, the flying pieces, the blast's tick with the settling,
/// and the swarm's work done once a game second: at the 30 s mark that
/// includes its scans for the specialists, which grow with the ground the
/// blast opened), and a typical one; the crater (cells thrown, the depth
/// planned at its middle and how far down the ground there ended up), the
/// most pieces in flight at once, and the share of the crater's mass that
/// left the body for good, within the minute and in the end. (On this small
/// body the pull is weak: most of the crater's slow ground near the rim is
/// still in the air a minute on. To see where it all ends up, the view then
/// moves away, and what is in the air is set down or lost a tick's worth
/// at a time: how long that takes and its dearest tick.)
#[test]
#[ignore]
fn blast_timings() {
    let s = sim();
    let (g, r) = (s.world.planet.real_surface_gravity(), s.world.planet.radius);
    eprintln!("surface pull {g:.4} m/s2, escape speed {:.1} m/s", (2.0 * g * r).sqrt());
    for kt in [10.0, 100.0, 1000.0, 3000.0, 10000.0] {
        let mut s = sim();
        let over = (0.5, -(s.world.planet.max_radius + 5.0));
        let ground = |s: &Sim| {
            let mut y = over.1 as i64;
            while s.world.material(0, y) == VACUUM {
                y += 1;
            }
            y
        };
        let y0 = ground(&s);
        let kg_cell = s.world.planet.mats.get(s.world.material(0, y0 + 2)).density;
        let vented = |s: &Sim| s.world.ledger.vented.iter().sum::<f64>();
        let v0 = vented(&s);
        let t = std::time::Instant::now();
        s.test_blast(over, kt);
        let mut worst = (t.elapsed().as_secs_f64(), 0.0, 0.0, 0.0, 0.0);
        let tl = s.blast_timelines()[0];
        let (mut opened, mut jumped) = (None, None);
        let mut work = vec![s.prof2[7]];
        let mut steps = Vec::new();
        let (mut ticks, mut after, mut most, mut hops) = (0, 0, 0, 0);
        while s.blasts_pending() > 0 || after < 600 {
            let (t, b, f, k, p) = (std::time::Instant::now(), s.prof2[7], s.prof2[2], s.prof2[6], s.prof[3]);
            s.step();
            // (Its crater is queued a tick before its top layer goes.)
            if opened.is_none() && s.last_crater.0 > 0 {
                opened = Some((ticks + after + 2) as f64 * s.eco.tick_s);
            }
            if jumped.is_none() && s.bodies.hopping() > 0 {
                jumped = Some((ticks + after + 1) as f64 * s.eco.tick_s);
            }
            let e = t.elapsed().as_secs_f64();
            steps.push(e);
            if e > worst.0 {
                worst = (e, s.prof2[7] - b, s.prof2[2] - f, s.prof2[6] - k, s.prof[3] - p);
            }
            most = most.max(s.bodies.drops());
            hops = hops.max(s.bodies.hopping());
            if s.blasts_pending() > 0 {
                ticks += 1;
                work.push(s.prof2[7] - b);
            } else {
                after += 1;
            }
        }
        work.sort_by(|a, b| a.total_cmp(b));
        steps.sort_by(|a, b| a.total_cmp(b));
        let (cells, bowl) = s.last_crater;
        let (still, lost_soon) = (s.bodies.drops(), vented(&s) - v0);
        s.set_attention(Some(None));
        let (mut away, mut away_worst) = (0, 0.0f64);
        while s.bodies.drops() > 0 && away < 1000 {
            let t = std::time::Instant::now();
            s.step();
            away_worst = away_worst.max(t.elapsed().as_secs_f64());
            away += 1;
        }
        let share = |kg: f64| 100.0 * kg / (cells.max(1) as f64 * kg_cell);
        eprintln!(
            "{kt} kt: shock {:.0} m/s, at the surface {:.3} s on, the dome bursting {:.2} s on, the crater's ground rising from {} s on, the ground round it from {} s on; the ground jumping at {:.0} m/s over it, out to {:.0} m either side, at most {hops} pieces of it kicked up in the air",
            tl.shock_mps,
            tl.t_surface,
            tl.t_vent,
            opened.map_or("never".to_string(), |v| format!("{v:.1}")),
            jumped.map_or("never".to_string(), |v| format!("{v:.1}")),
            tl.spall_v0,
            tl.spall_m,
        );
        eprintln!(
            "{kt} kt: {ticks} ticks ({:.1} game s); worst tick {:.1} ms (blast work {:.1}, flying {:.1}, blast tick and settling {:.1}, the swarm's once-a-second work {:.1}), median {:.1} ms; blast work at most {:.1} ms, typically {:.1}, {:.0} ms in all; crater {cells} cells, {bowl:.0} m deep planned, ground at the middle {} m lower; at most {most} pieces in flight, {still} still flying a minute on; {:.0}% of the crater's mass left the body within the minute, {:.0}% in the end; the view moved away, the rest set down in {away} ticks, the dearest {:.1} ms",
            ticks as f64 * s.eco.tick_s,
            worst.0 * 1000.0,
            worst.1 * 1000.0,
            worst.2 * 1000.0,
            worst.3 * 1000.0,
            worst.4 * 1000.0,
            steps[steps.len() / 2] * 1000.0,
            work[work.len() - 1] * 1000.0,
            work[work.len() / 2] * 1000.0,
            s.prof2[7] * 1000.0,
            ground(&s) - y0,
            share(lost_soon),
            share(vented(&s) - v0),
            away_worst * 1000.0,
        );
    }
}

/// What a blast costs on a body with air, plants and animals and a swarm at
/// work (run by hand: cargo test --release --test sapper_blast --
/// --ignored --nocapture blast_timings_air): the dev planet with its life
/// on and 20,000 free cutters, a charge going off at a sapper's depth that
/// far along the ground from where most of them work (300 m at 10 kt, more
/// for bigger ones, so they are outside the crater and the ground's jump),
/// then 30 s more: the dearest whole step and the median, the blast's own
/// work at most in a tick and in all, how many ticks it ran, and the most
/// pieces in the air.
#[test]
#[ignore]
fn blast_timings_air() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    // (Also in the thickest forest on the body, every plant cell in sight;
    // and on the surface or over it, `depth` m down, nuclear or not.)
    let rows: [(f64, bool, Option<f64>, bool); 9] = [
        (10.0, false, None, false),
        (100.0, false, None, false),
        (1000.0, false, None, false),
        (10.0, true, None, false),
        (1000.0, true, None, false),
        (10.0, true, Some(0.0), false),
        (10.0, true, Some(0.0), true),
        (100.0, true, Some(-150.0), false),
        (1000.0, false, Some(0.0), false),
    ];
    for (kt, forest, open, conventional) in rows {
        let base = dft_sim::load_body(&data, "dev-planet").unwrap();
        let mut spec = base.spec.clone();
        spec.life = Some(1.0);
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let mut eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
        eco.seed_swarm.start_harvesters = 20_000;
        let mut s = Sim::new(World::new(p), eco);
        s.free_drones = Some(20_000);
        s.run(40.0);
        let mut legs = Vec::new();
        s.drone_legs(&mut legs);
        let now = s.time();
        let pts: Vec<(f64, f64)> = legs.iter().filter(|l| l.state <= 2).map(|l| l.at(now)).collect();
        let crowd = |p: (f64, f64)| pts.iter().filter(|q| (q.0 - p.0).hypot(q.1 - p.1) < 60.0).count();
        let p0 = pts.iter().step_by(50).copied().max_by_key(|&p| crowd(p)).unwrap();
        let a = if forest {
            let (a, n) = (0..72).map(|k| k as f64 * std::f64::consts::TAU / 72.0).map(|a| (a, plants_band(&s, a, 0.0, 300.0).len())).max_by_key(|p| p.1).unwrap();
            println!("{kt} kt in the forest: {n} plant cells within 300 m");
            a
        } else {
            p0.1.atan2(p0.0) + 300.0 * (kt / 10.0f64).cbrt() / s.world.planet.radius
        };
        let g = s.world.planet.surface_radius(a.cos(), a.sin());
        let depth = open.unwrap_or_else(|| s.test_blast_depth(kt));
        let t = std::time::Instant::now();
        s.test_blast_at((a.cos() * g, a.sin() * g), kt, depth, conventional);
        let first = t.elapsed().as_secs_f64();
        println!("{kt} kt, {depth:.0} m down, {}:", if conventional { "conventional" } else { "nuclear" });
        let (mut steps, mut work) = (vec![first], vec![s.prof2[7]]);
        let (mut ticks, mut after, mut most) = (0, 0, 0);
        while s.blasts_pending() > 0 || after < 300 {
            let (t, b) = (std::time::Instant::now(), s.prof2[7]);
            s.step();
            steps.push(t.elapsed().as_secs_f64());
            most = most.max(s.bodies.drops());
            if s.blasts_pending() > 0 {
                ticks += 1;
                work.push(s.prof2[7] - b);
            } else {
                after += 1;
            }
            assert!(ticks < 3000);
        }
        let worst = steps.iter().copied().fold(0.0, f64::max);
        steps.sort_by(|a, b| a.total_cmp(b));
        let wmax = work.iter().copied().fold(0.0, f64::max);
        println!(
            "{kt} kt: {} cutters within 60 m of the flock; {ticks} ticks; worst step {:.1} ms, median {:.1} ms; blast work at most {:.1} ms in a tick, {:.0} ms in all; at most {most} pieces in the air",
            crowd(p0),
            worst * 1000.0,
            steps[steps.len() / 2] * 1000.0,
            wmax * 1000.0,
            work.iter().sum::<f64>() * 1000.0,
        );
        println!("{kt} kt: {:?}", s.air_last);
        if open.is_some() {
            println!("{kt} kt: {:?}", s.flash_last);
        }
    }
}

/// A blast in hard rock, which holds some of it together in big pieces, as
/// the yield grows (run by hand: cargo test --release --test sapper_blast
/// -- --ignored --nocapture): how many pieces it held, how many it threw out
/// whole as rock bodies and how many cells they were, the most rock bodies
/// in the air at once, and how many were still in the air a minute on; how
/// many ticks the blast took to run its course, its work in all and at most
/// in a tick; and the dearest whole step, and the dearest time spent moving
/// what is in the air, over the blast and the minute after it.
#[test]
#[ignore]
fn blast_chunks() {
    for kt in [10.0, 100.0, 1000.0, 10000.0] {
        let mut s = hard_sim();
        let over = (0.5, -(s.world.planet.max_radius + 5.0));
        s.test_blast(over, kt);
        let (mut ticks, mut after, mut most, mut bodies) = (0, 0, 0.0f64, 0);
        let (mut step, mut flying) = (0.0f64, 0.0f64);
        while s.blasts_pending() > 0 || after < 600 {
            let (b, f) = (s.prof2[7], s.prof2[2]);
            let t = std::time::Instant::now();
            s.step();
            step = step.max(t.elapsed().as_secs_f64());
            flying = flying.max(s.prof2[2] - f);
            bodies = bodies.max(s.bodies.count());
            if s.blasts_pending() > 0 {
                most = most.max(s.prof2[7] - b);
                ticks += 1;
            } else {
                after += 1;
            }
        }
        let (held, thrown, cells) = s.last_chunks;
        eprintln!(
            "{kt} kt: {held} pieces held, {thrown} thrown out whole ({cells} cells); at most {bodies} rock bodies in the air, {} a minute on; {ticks} ticks ({:.1} game s), blast work {:.0} ms in all, at most {:.1} ms a tick; dearest step {:.1} ms, dearest flying {:.1} ms",
            s.bodies.count(),
            ticks as f64 * s.eco.tick_s,
            s.prof2[7] * 1000.0,
            most * 1000.0,
            step * 1000.0,
            flying * 1000.0,
        );
    }
}

/// Drawn cracks (cells that show as cracks) per square metre of rock round
/// a charge at `at` of blast radius `r` (`up`: straight up there), in rings
/// a quarter radius wide from 1.5 to 6 radii: (cracks, rock) below the
/// charge, and over it.
fn crack_rings(s: &Sim, at: (f64, f64), up: (f64, f64), r: f64) -> [Vec<[f64; 2]>; 2] {
    let rings = 18;
    let mut out = [vec![[0.0; 2]; rings], vec![[0.0; 2]; rings]];
    let k = (6.0 * r).ceil() as i64;
    let (cx, cy) = (at.0.floor() as i64, at.1.floor() as i64);
    for x in cx - k..=cx + k {
        for y in cy - k..=cy + k {
            let (dx, dy) = (x as f64 + 0.5 - at.0, y as f64 + 0.5 - at.1);
            let i = ((dx.hypot(dy) / r - 1.5) / 0.25).floor();
            if i < 0.0 || i >= rings as f64 || s.world.material(x, y) == VACUUM {
                continue;
            }
            let side = &mut out[(dx * up.0 + dy * up.1 > 0.0) as usize][i as usize];
            side[1] += 1.0;
            if s.world.look(x, y) == dft_sim::world::Look::CRACKED {
                side[0] += 1.0;
            }
        }
    }
    out
}

/// How thickly the drawn cracks lie with distance from the charge (run by
/// hand: cargo test --release --test sapper_blast -- --ignored
/// --nocapture): per square metre of rock in rings a quarter radius wide
/// from 1.5 radii out, below the charge, over several blasts at different
/// places (they should thin out steadily past the broken rock); and for a
/// blast with no loosened column, below and over the charge (they should be
/// about as thick either way).
#[test]
#[ignore]
fn blast_crack_profile() {
    let show = |v: &[[f64; 2]]| v.iter().map(|x| format!("{:.3}", x[0] / x[1].max(1.0))).collect::<Vec<_>>().join(" ");
    let add = |a: &mut Vec<[f64; 2]>, b: &[[f64; 2]]| {
        for (x, y) in a.iter_mut().zip(b) {
            x[0] += y[0];
            x[1] += y[1];
        }
    };
    for (kt, places, column) in [(10.0f64, 8, true), (1000.0, 3, true), (10000.0, 1, true), (10.0, 8, false), (50.0, 4, false)] {
        let mut sum = [vec![[0.0; 2]; 18], vec![[0.0; 2]; 18]];
        for k in 0..places {
            let mut s = sim();
            if !column {
                s.eco.units.sapper.depth_m_per_cbrt_kt = 0.0;
            }
            let rr = s.world.planet.max_radius + 5.0;
            // (Places a little apart, each blast with its own roll of the
            // dice: the test world has ground only round the top of its far
            // side.)
            let a = -std::f64::consts::FRAC_PI_2 + 0.01 * (k as f64 - 0.5 * (places - 1) as f64);
            let up = (a.cos(), a.sin());
            let over = (up.0 * rr, up.1 * rr);
            // (The charge, straight under where the ground was found.)
            let mut d = 0.0;
            while s.world.material((over.0 - up.0 * d).floor() as i64, (over.1 - up.1 * d).floor() as i64) == VACUUM {
                d += 0.25;
                assert!(d < 3000.0, "no ground under place {k}");
            }
            let sp = &s.eco.units.sapper;
            let r = sp.radius_m_per_cbrt_kt * kt.cbrt();
            let depth = if column { sp.depth_m_per_cbrt_kt * kt.cbrt() } else { r + sp.bury_m };
            let at = (over.0 - up.0 * (d + depth), over.1 - up.1 * (d + depth));
            s.test_blast(over, kt);
            let mut t = 0;
            while s.blasts_pending() > 0 {
                s.step();
                t += 1;
                assert!(t < 3000, "{kt} kt at place {k}: the blast ran its course within 5 minutes");
            }
            let rings = crack_rings(&s, at, up, r);
            add(&mut sum[0], &rings[0]);
            add(&mut sum[1], &rings[1]);
        }
        let what = if column { "" } else { " with no column" };
        eprintln!("{kt} kt{what}, {places} places, below the charge, from 1.5 radii: {}", show(&sum[0]));
        if !column {
            eprintln!("{kt} kt{what}, {places} places, over the charge, from 1.5 radii:  {}", show(&sum[1]));
        }
    }
}

/// What the loose and cracked sets cost as a blast's shock has run its
/// course, and a minute on, as the yield grows (run by hand: cargo test
/// --release --test sapper_blast -- --ignored --nocapture blast_cell_sets).
#[test]
#[ignore]
fn blast_cell_sets() {
    for kt in [10.0, 1000.0, 10000.0] {
        let mut s = sim();
        let over = (0.5, -(s.world.planet.max_radius + 5.0));
        s.test_blast(over, kt);
        while s.blasts_pending() > 0 {
            s.step();
        }
        cell_sets_report(&s, &format!("{kt} kt, the shock done"));
        for _ in 0..(60.0 / s.eco.tick_s).round() as usize {
            s.step();
        }
        cell_sets_report(&s, &format!("{kt} kt, a minute on"));
    }
}

/// How many cells the loose and cracked sets hold, the cells still
/// settling, and how many in each set are empty (dug or thrown away, or
/// settled out of); the memory the two sets hold; the bytes they take in a save on
/// their own, raw and deflated; the whole save, raw and deflated; and the
/// time to write it, to deflate it, to inflate it and to read it back (each
/// the best of three). Then fingerprints of the ground and of the two sets,
/// to show a change left them the same. (No blast may be going off.)
fn cell_sets_report(s: &Sim, what: &str) {
    let fnv = |h: u64, v: u64| (h ^ v).wrapping_mul(0x100_0000_01b3);
    let (loose, settling) = s.loose_count();
    let cracked = s.cracked_cells().count();
    let empty = |cells: Vec<(i64, i64)>| cells.iter().filter(|c| s.world.material(c.0, c.1) == VACUUM).count();
    let (empty_loose, empty_cracked) = (empty(s.loose_cells().collect()), empty(s.cracked_cells().collect()));
    let (held, raw_sets, packed_sets) = s.cell_sets_cost();
    let best = |f: &mut dyn FnMut() -> f64| (0..3).map(|_| f()).fold(f64::MAX, f64::min) * 1000.0;
    let mut raw = Vec::new();
    let write_ms = best(&mut || {
        let t = std::time::Instant::now();
        let mut o = W::default();
        s.write_save(&mut o);
        let e = t.elapsed().as_secs_f64();
        raw = o.0;
        e
    });
    let mut packed = Vec::new();
    let pack_ms = best(&mut || {
        let t = std::time::Instant::now();
        packed = dft_sim::save::pack(b"TEST", 1, &raw);
        t.elapsed().as_secs_f64()
    });
    let unpack_ms = best(&mut || {
        let t = std::time::Instant::now();
        let back = dft_sim::save::unpack(b"TEST", 1, &packed).unwrap();
        let e = t.elapsed().as_secs_f64();
        assert_eq!(back.len(), raw.len());
        e
    });
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let eco = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    let read_ms = best(&mut || {
        let t = std::time::Instant::now();
        let l = Sim::read_save(&mut R(&raw, 0), s.world.planet.clone(), eco.clone()).unwrap();
        let e = t.elapsed().as_secs_f64();
        assert_eq!(l.loose_count().0, loose);
        e
    });
    let print = |cells: Vec<(i64, i64)>| {
        let mut cells = cells;
        cells.sort_unstable();
        cells.iter().fold(0xcbf2_9ce4_8422_2325, |h, c| fnv(fnv(h, c.0 as u64), c.1 as u64))
    };
    eprintln!(
        "{what}: {loose} loose, {cracked} cracked, {settling} settling ({empty_loose} loose and {empty_cracked} cracked empty); sets hold {:.2} MB, take {:.2} MB raw / {:.0} KB deflated on their own; save {:.2} MB raw / {:.0} KB deflated; write {write_ms:.1} ms, deflate {pack_ms:.0} ms, inflate {unpack_ms:.1} ms, read {read_ms:.1} ms; ground {:016x}, loose {:016x}, cracked {:016x}",
        held as f64 / 1e6,
        raw_sets as f64 / 1e6,
        packed_sets as f64 / 1e3,
        raw.len() as f64 / 1e6,
        packed.len() as f64 / 1e3,
        s.world.state_hash(),
        print(s.loose_cells().collect()),
        print(s.cracked_cells().collect()),
    );
}
