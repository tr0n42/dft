use dft_sim::sandbox::Sandbox;
use dft_sim::world::World;
use std::path::Path;

fn demo() -> (World, Sandbox, (i64, i64)) {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut w = World::new(dft_sim::load_body(&data, "sandbox-demo").unwrap());
    let mut sb = Sandbox::new(&w);
    let scene = dft_sim::sandbox::open_demo(&mut w, &mut sb);
    (w, sb, scene.cavity)
}

/// All the matter the body has ever held: what's left, what was taken or lost,
/// less the corrections made when regions were first generated exactly.
fn total_kg(w: &World) -> f64 {
    let l = &w.ledger;
    w.remaining_kg().iter().sum::<f64>() + l.mined.iter().sum::<f64>() + l.vented.iter().sum::<f64>() - l.reconciled.iter().sum::<f64>()
}

#[test]
fn loose_rubble_caves_in_and_mass_is_kept() {
    let (mut w, mut sb, inside) = demo();
    let before = total_kg(&w);
    for _ in 0..300 {
        sb.step(&mut w);
    }
    assert!(sb.stats.moves > 1000, "{:?}", sb.stats);
    // Rubble has fallen into the cavity.
    let regolith = w.planet.mats.id("regolith").unwrap();
    let fallen = (-14..=14).filter(|&x| (inside.1 - 10..inside.1 + 10).any(|y| w.material(x, y) == regolith)).count();
    assert!(fallen > 10, "only {fallen} columns of the cavity have rubble in them");
    // Not a gram made or lost, and the books agree with the world.
    assert!((total_kg(&w) - before).abs() < 1e-6 * before);
    let initial: f64 = w.ledger.initial.iter().sum();
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
}

#[test]
fn exposed_lava_crusts_over_and_water_boils_against_it() {
    let (mut w, mut sb, _) = demo();
    let water = w.planet.mats.id("water").unwrap();
    let mut boiling = false;
    for _ in 0..1200 {
        sb.step(&mut w);
        // Water at its boiling point, banking heat toward turning to steam.
        boiling |= sb.latent.iter().any(|(c, v)| *v > 0.0 && w.material(c.0, c.1) == water);
    }
    // No reaction rows: it all comes from heat. The lava loses heat to open
    // space and cold rock and crusts over; water that runs down to it is
    // brought to the boil (here the lava front crusts over before the water
    // arrives, so the water boils against hot crust rather than flashing off).
    let rock = w.planet.mats.id("lava-rock").unwrap() as usize;
    let steam = w.planet.mats.id("steam").unwrap() as usize;
    assert_eq!(sb.stats.reactions, 0);
    assert!(w.ledger.changed[rock] > 0.0, "no lava crusted over: {:?}", sb.stats);
    assert!(boiling || w.ledger.changed[steam] > 0.0, "no water came to the boil: {:?}", sb.stats);
}

#[test]
fn same_start_same_sandbox() {
    let run = || {
        let (mut w, mut sb, _) = demo();
        for _ in 0..200 {
            sb.step(&mut w);
        }
        w.state_hash()
    };
    let a = run();
    println!("  fingerprint same_start_same_sandbox: {a:016x}");
    assert_eq!(a, run());
}

#[test]
fn a_breached_comet_gas_pocket_vents_to_space() {
    // Cut a shaft from the surface into a pocket of trapped carbon monoxide:
    // the gas rises up the shaft and is lost to space, every kilogram booked.
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut w = World::new(dft_sim::load_body(&data, "comet").unwrap());
    let mut sb = Sandbox::new(&w);
    let co = w.planet.mats.id("carbon monoxide").unwrap();
    // The shallowest gas cell on a coarse search.
    let r = w.planet.max_radius as i64;
    let mut best: Option<((i64, i64), i64)> = None;
    for y in (-r..=r).step_by(3) {
        for x in (-r..=r).step_by(3) {
            if w.material(x, y) == co {
                let d = x * x + y * y;
                if best.is_none_or(|b| d > b.1) {
                    best = Some(((x, y), d));
                }
            }
        }
    }
    let (g, _) = best.expect("the comet holds gas pockets");
    let gr = ((g.0 * g.0 + g.1 * g.1) as f64).sqrt();
    let (ux, uy) = (g.0 as f64 / gr, g.1 as f64 / gr);
    let before = total_kg(&w);
    // A 3-cell-wide shaft straight out from the pocket.
    let mut r_at = gr;
    while r_at < w.planet.max_radius + 2.0 {
        for k in -1..=1 {
            let (x, y) = ((ux * r_at - uy * k as f64).floor() as i64, (uy * r_at + ux * k as f64).floor() as i64);
            let m = w.material(x, y);
            if m != dft_sim::materials::VACUUM && m != co {
                w.mine_cell(x, y);
                sb.wake_around((x, y));
            }
        }
        r_at += 0.7;
    }
    let count = |w: &World| {
        let mut n = 0;
        for y in g.1 - 60..g.1 + 60 {
            for x in g.0 - 60..g.0 + 60 {
                if w.material(x, y) == co {
                    n += 1;
                }
            }
        }
        n
    };
    let gas = count(&w);
    let t = std::time::Instant::now();
    for _ in 0..3000 {
        sb.step(&mut w);
    }
    let ms = t.elapsed().as_millis();
    println!("  shaft {:.0} m deep, {gas} cells of gas; after 300 s, {} left: {:?} in {ms} ms", w.planet.max_radius - gr, count(&w), sb.stats);
    assert!(sb.stats.vented as f64 >= 0.7 * gas as f64, "the gas didn't get out");
    assert!((total_kg(&w) - before).abs() < 1e-6 * before);
    let initial: f64 = w.ledger.initial.iter().sum();
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
}

#[test]
fn sunlight_warms_ground_dug_out_on_the_day_side() {
    // Dig a pit on the comet's noon side down to a band of dry ice: sunlight
    // reaching the pit floor warms the exposed ice well above the comet's
    // 110 K. The same pit dug on the night side stays cold.
    let run = |sign: f64| {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let mut w = World::new(dft_sim::load_body(&data, "comet").unwrap());
        let mut sb = Sandbox::new(&w);
        let dry = w.planet.mats.id("dry ice").unwrap();
        let x0 = 0;
        let top = (w.planet.surface_radius(0.0, sign) * sign) as i64;
        let step = if sign > 0.0 { -1 } else { 1 };
        let mut y = top;
        while w.material(x0, y) != dry {
            y += step;
            assert!((y - top).abs() < 400, "no dry ice under this spot");
        }
        let mut yy = top + 2 * -step;
        while yy != y {
            for x in x0 - 3..=x0 + 3 {
                if w.material(x, yy) != dft_sim::materials::VACUUM {
                    w.mine_cell(x, yy);
                }
                sb.wake_around((x, yy));
            }
            yy += step;
        }
        let lit = Sandbox::sees_sun(&w, (x0, y - step), 0.0).is_some();
        let mut peak: f64 = 0.0;
        for k in 0..1200 {
            sb.time_s = k as f64 * 0.1;
            sb.step(&mut w);
            if let Some(t) = sb.temps.get(&(x0, y)) {
                peak = peak.max(*t);
            }
        }
        (lit, peak, sb.stats.clone())
    };
    let (lit_day, day, sd) = run(1.0);
    let (lit_night, night, _) = run(-1.0);
    println!("  noon pit floor: lit {lit_day}, peak {day:.0} K ({} changes of state); midnight: lit {lit_night}, peak {night:.0} K", sd.phase_changes);
    assert!(lit_day && !lit_night);
    assert!(day > 140.0, "the sunlit floor barely warmed");
    assert!(night < 115.0, "the night-side floor warmed");
}

#[test]
fn a_breached_ice_shell_floods_up_to_its_balance_level() {
    // Bore through the ice moon's shell into its ocean: brine rises up the
    // shaft, pushed by the weight of the ice, to about nine tenths of the
    // way up, and no further. Every kilogram is kept.
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut w = World::new(dft_sim::load_body(&data, "ice-moon").unwrap());
    let mut sb = Sandbox::new(&w);
    let brine = w.planet.mats.id("brine").unwrap();
    let top = w.planet.surface_radius(0.0, 1.0) as i64;
    let mut y = top + 2;
    while w.material(0, y) != brine {
        for x in -2..=2 {
            if w.material(x, y) != dft_sim::materials::VACUUM {
                w.mine_cell(x, y);
            }
            sb.wake_around((x, y));
        }
        y -= 1;
    }
    let ocean_top = y;
    let shell = top - ocean_top;
    let before = total_kg(&w);
    let t = std::time::Instant::now();
    let mut level = ocean_top;
    for _ in 0..6000 {
        sb.step(&mut w);
    }
    while w.material(0, level + 1) == brine || w.planet.mats.get(w.material(0, level + 1)).name == "sea ice" {
        level += 1;
    }
    let ms = t.elapsed().as_millis();
    let rise = (level - ocean_top) as f64 / shell as f64;
    println!("  shell {shell} m; brine rose {} m ({:.0}% of the way) in 600 s, {ms} ms; {:?}", level - ocean_top, rise * 100.0, sb.stats);
    assert!(rise > 0.6, "the ocean barely came up");
    assert!(level < top, "the ocean came out on top");
    assert!((total_kg(&w) - before).abs() < 1e-6 * before);
}
