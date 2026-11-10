use dft_sim::bodies::Bodies;
use dft_sim::materials::VACUUM;
use dft_sim::world::World;
use std::path::Path;

/// Cut a 10 x 10 block free in the body's upper half (a 3 m moat all round
/// it, deepest underneath), so it drops into its own pit.
fn scene() -> (World, Bodies, Vec<(i64, i64)>) {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut w = World::new(dft_sim::load_body(&data, "sandbox-demo").unwrap());
    let mut b = Bodies::new(&w);
    let (x0, y0) = (-5, 80);
    let mut block = Vec::new();
    for y in y0 - 12..y0 + 13 {
        for x in x0 - 3..x0 + 13 {
            let inside = (x0..x0 + 10).contains(&x) && (y0..y0 + 10).contains(&y);
            if inside {
                block.push((x, y));
            } else if w.material(x, y) != VACUUM {
                w.mine_cell(x, y);
                b.opened((x, y));
            }
        }
    }
    (w, b, block)
}

fn total_kg(w: &World) -> f64 {
    let l = &w.ledger;
    w.remaining_kg().iter().sum::<f64>() + l.mined.iter().sum::<f64>() + l.aloft.iter().sum::<f64>() - l.reconciled.iter().sum::<f64>() - l.deposited.iter().sum::<f64>()
}

#[test]
fn a_block_cut_free_falls_and_lands_with_every_kilogram() {
    let (mut w, mut b, block) = scene();
    let before = total_kg(&w);
    let initial: f64 = w.ledger.initial.iter().sum();
    let lifted = b.break_free(&mut w);
    assert_eq!(lifted.len(), block.len(), "the whole block breaks free");
    assert!(block.iter().all(|c| w.material(c.0, c.1) == VACUUM));
    let mut landed = 0;
    for _ in 0..600 {
        landed += b.step(&mut w).landed.len();
        // The books balance at every tick, with the block counted as aloft.
        assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
        if b.count() == 0 {
            break;
        }
    }
    assert_eq!(b.count(), 0, "the block came to rest within a minute");
    assert_eq!(landed, block.len());
    assert!((total_kg(&w) - before).abs() < 1e-6 * before);
    // It moved: cut free, it was shoved (out toward space or rolling), so
    // not every cell is back where it started.
    let moved = block.iter().filter(|c| w.material(c.0, c.1) == VACUUM).count();
    assert!(moved > 0, "no cell left its old place");
}

#[test]
fn same_fall_every_time() {
    let run = || {
        let (mut w, mut b, _) = scene();
        b.break_free(&mut w);
        for _ in 0..200 {
            b.step(&mut w);
        }
        b.settle_all(&mut w);
        w.state_hash()
    };
    let a = run();
    println!("  fingerprint same_fall_every_time: {a:016x}");
    assert_eq!(a, run());
}

#[test]
fn a_wrecked_drone_leaves_scrap_on_the_books() {
    let (mut w, mut b, _) = scene();
    let initial: f64 = w.ledger.initial.iter().sum();
    let at = b.wreck(&mut w, -6.5, 79.5).expect("room for the scrap");
    let scrap = w.planet.mats.id("drone-scrap").unwrap();
    assert_eq!(w.material(at.0, at.1), scrap);
    assert!(w.ledger.deposited[scrap as usize] > 0.0);
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
}

#[test]
fn a_block_falls_true_on_an_earth_sized_body() {
    // 6,371 km out, 32-bit floats step in half metres; each region gets its
    // own physics frame, so the block falls into its pit as it would at home.
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut w = World::new(dft_sim::load_body(&data, "earth-test").unwrap());
    let mut b = Bodies::new(&w);
    let top = w.planet.surface_radius(0.0, 1.0) as i64;
    let (x0, y0) = (-5, top - 30);
    let mut block = Vec::new();
    for y in y0 - 12..y0 + 13 {
        for x in x0 - 3..x0 + 13 {
            let inside = (x0..x0 + 10).contains(&x) && (y0..y0 + 10).contains(&y);
            if inside {
                block.push((x, y));
            } else if w.material(x, y) != VACUUM {
                w.mine_cell(x, y);
                b.opened((x, y));
            }
        }
    }
    let initial: f64 = w.ledger.initial.iter().sum();
    let before = total_kg(&w);
    assert_eq!(b.break_free(&mut w).len(), block.len());
    let mut landed = Vec::new();
    for _ in 0..1200 {
        let ev = b.step(&mut w);
        landed.extend(ev.landed.iter().map(|l| l.0));
        assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
        if b.count() == 0 {
            break;
        }
    }
    assert_eq!(b.count(), 0, "the block came to rest");
    assert_eq!(landed.len(), block.len());
    assert!((total_kg(&w) - before).abs() < 1e-6 * before);
    // It dropped into its pit (shoved a little as it broke free, and maybe
    // broken up, but on Earth's pull it can't go far).
    let (xmin, xmax) = landed.iter().fold((i64::MAX, i64::MIN), |a, c| (a.0.min(c.0), a.1.max(c.0)));
    let (ymin, ymax) = landed.iter().fold((i64::MAX, i64::MIN), |a, c| (a.0.min(c.1), a.1.max(c.1)));
    println!("  landed in x {xmin}..{xmax}, y {}..{} below where it started", y0 - ymax, y0 - ymin);
    assert!(xmin >= x0 - 25 && xmax < x0 + 35, "it went too far sideways");
    assert!(ymin >= y0 - 40 && ymin < y0, "it didn't drop into the pit");
}
