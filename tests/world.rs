use dft_sim::materials::VACUUM;
use dft_sim::noise::{hash, unit};
use dft_sim::save;
use dft_sim::totals::Node;
use dft_sim::world::{State, World, key};
use std::path::Path;

fn planet(name: &str) -> dft_sim::planet::Planet {
    dft_sim::load_body(Path::new(env!("CARGO_MANIFEST_DIR")).join("data").as_path(), name).unwrap()
}

/// A fixed sequence of mixed operations: tunnels dug cell by cell, plus whole
/// nodes mined at once.
fn dig(w: &mut World, seed: u64) {
    for t in 0..6 {
        let x0 = (unit(hash(seed, t, 0, 1)) * 3000.0 - 1500.0) as i64;
        let y0 = (unit(hash(seed, t, 0, 2)) * 3000.0 - 1500.0) as i64;
        for s in 0..300 {
            for d in -1..=1 {
                w.mine_cell(x0 + s, y0 + d + s / 3);
            }
        }
    }
    for k in 0..4 {
        let n = Node { level: 2, ix: 10 + k * 3, iy: 12 + (k % 2) * 5 };
        w.mine_node(n);
    }
}

fn check_parts_sum(w: &World) {
    for (k, e) in &w.nodes {
        if e.state != State::Split {
            continue;
        }
        let level = (k >> 58) as u32;
        // Find this node's coordinates by searching its children's keys.
        let _ = level;
        let mut found = 0;
        for (k2, e2) in &w.nodes {
            if (k2 >> 58) as u32 + 1 == level && (k2 & ((1 << 58) - 1)) >> 2 == (k & ((1 << 58) - 1)) {
                found += 1;
                let _ = e2;
            }
        }
        assert_eq!(found, 4, "a split node must have four child entries");
        let sum: Vec<f64> = (0..e.totals.len())
            .map(|m| {
                w.nodes
                    .iter()
                    .filter(|(k2, _)| (*k2 >> 58) as u32 + 1 == level && (*k2 & ((1 << 58) - 1)) >> 2 == (k & ((1 << 58) - 1)))
                    .map(|(_, c)| c.totals[m])
                    .sum()
            })
            .collect();
        for m in 0..sum.len() {
            assert!((sum[m] - e.totals[m]).abs() < 1e-6 * e.totals[m].abs().max(1.0), "parts don't sum for material {m}");
        }
    }
}

#[test]
fn untouched_world_matches_the_generator() {
    let p = planet("rubble-asteroid");
    let w = World::new(planet("rubble-asteroid"));
    for k in 0..5000i64 {
        let (x, y) = ((k * 7919) % 5800 - 2900, (k * 104729) % 5800 - 2900);
        assert_eq!(w.material(x, y), p.material(x, y));
    }
}

#[test]
fn mining_a_cell_removes_it_and_books_it() {
    let mut w = World::new(planet("rubble-asteroid"));
    let (x, y) = (100, -200);
    let before = w.material(x, y);
    assert_ne!(before, VACUUM);
    assert_eq!(w.mine_cell(x, y), Some(before));
    assert_eq!(w.material(x, y), VACUUM);
    assert_eq!(w.mine_cell(x, y), None);
    let density = w.planet.mats.get(before).density;
    assert_eq!(w.ledger.mined[before as usize], density);
    // Neighbours are untouched.
    let p = planet("rubble-asteroid");
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        assert_eq!(w.material(x + dx, y + dy), p.material(x + dx, y + dy));
    }
}

#[test]
fn books_balance_and_parts_sum_after_mixed_mining() {
    let mut w = World::new(planet("rubble-asteroid"));
    dig(&mut w, 7);
    let imbalance = w.ledger.imbalance(&w.remaining_kg());
    let total: f64 = w.ledger.initial.iter().sum();
    assert!(imbalance < 1e-9 * total, "books off by {imbalance} kg");
    check_parts_sum(&w);
    // Mined plus remaining equals the start plus corrections, in total.
    let start: f64 = w.ledger.initial.iter().sum::<f64>() + w.ledger.reconciled.iter().sum::<f64>();
    let now: f64 = w.remaining_kg().iter().sum::<f64>() + w.ledger.mined_kg();
    assert!((start - now).abs() < 1e-9 * start);
}

#[test]
fn mining_a_node_pays_what_is_left_in_it() {
    let mut w = World::new(planet("rubble-asteroid"));
    // Touch the node's area first so it has exact leaves inside.
    let n = Node { level: 1, ix: 30, iy: 31 };
    let (x0, _, y0, _) = w.tree.bounds(n);
    let mut taken = 0.0;
    for i in 0..50 {
        if let Some(m) = w.mine_cell(x0 + i, y0 + 5) {
            taken += w.planet.mats.get(m).density;
        }
    }
    let had: f64 = {
        let t = w.node_totals(n).unwrap();
        t.iter().enumerate().map(|(i, c)| c * w.planet.mats.get(i as dft_sim::materials::MatId).density).sum()
    };
    let paid: f64 = w.mine_node(n).iter().sum();
    assert!((paid - had).abs() < 1e-6 * had.max(1.0));
    assert!(taken > 0.0);
    for i in 0..50 {
        assert_eq!(w.material(x0 + i, y0 + 5), VACUUM);
        assert_eq!(w.material(x0 + i, y0 + 40), VACUUM);
    }
}

#[test]
fn emptied_regions_collapse() {
    let mut w = World::new(planet("rubble-asteroid"));
    let n = Node { level: 0, ix: 70, iy: 70 };
    let (x0, x1, y0, y1) = w.tree.bounds(n);
    for y in y0..y1 {
        for x in x0..x1 {
            w.mine_cell(x, y);
        }
    }
    // The leaf holds no cells any more: it is Empty or merged into an Empty parent.
    let gone = match w.nodes.get(&key(n)).map(|e| &e.state) {
        None => true,
        Some(State::Empty) => true,
        _ => false,
    };
    assert!(gone);
    let dense = w.nodes.values().filter(|e| matches!(e.state, State::Dense(_))).count();
    assert_eq!(dense, 0, "no dense leaf should remain");
    // Mining a whole block of four siblings merges them into one Empty parent.
    let p = Node { level: 3, ix: 9, iy: 9 };
    for c in p.children() {
        w.mine_node(c);
    }
    assert!(!w.nodes.contains_key(&key(p.children()[0])));
}

#[test]
fn same_operations_same_world() {
    let mut a = World::new(planet("rubble-asteroid"));
    let mut b = World::new(planet("rubble-asteroid"));
    dig(&mut a, 11);
    dig(&mut b, 11);
    assert_eq!(a.state_hash(), b.state_hash());
    let mut c = World::new(planet("rubble-asteroid"));
    dig(&mut c, 12);
    assert_ne!(a.state_hash(), c.state_hash());
}

#[test]
fn saves_round_trip_exactly() {
    let mut w = World::new(planet("rubble-asteroid"));
    dig(&mut w, 3);
    let (_, bytes) = save::save(&w);
    let back = save::load(&bytes, planet("rubble-asteroid")).unwrap();
    assert_eq!(back.state_hash(), w.state_hash());
    assert_eq!(back.ledger, w.ledger);
    for k in 0..5000i64 {
        let (x, y) = ((k * 7919) % 5800 - 2900, (k * 104729) % 5800 - 2900);
        assert_eq!(back.material(x, y), w.material(x, y));
    }
    // A save never loads onto a different body, and damage is caught.
    assert!(save::load(&bytes, planet("active-planetoid")).is_err());
    let mut broken = bytes.clone();
    broken.truncate(bytes.len() / 2);
    assert!(save::load(&broken, planet("rubble-asteroid")).is_err());
}

#[test]
fn thread_count_never_changes_the_world() {
    let run = |threads: usize| {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
            let mut w = World::new(planet("rubble-asteroid"));
            dig(&mut w, 5);
            w.state_hash()
        })
    };
    let one = run(1);
    let many = run(std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).max(2));
    println!("  fingerprint thread_count_never_changes_the_world: {one:016x}");
    assert_eq!(one, many);
}

/// A renderer's copy of the dug ground (leaf snapshots) matches the world.
#[test]
fn leaf_snapshots_match_the_world() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut w = World::new(dft_sim::load_body(&data, "rubble-asteroid").unwrap());
    let top = w.planet.surface_radius(0.0, 1.0) as i64;
    let mut dug = Vec::new();
    for y in (top - 150)..(top - 100) {
        for x in -70..70 {
            if w.mine_cell(x, y).is_some() {
                dug.push((x, y));
            }
        }
    }
    assert!(!dug.is_empty());
    let leaves = w.stored_leaves();
    for &(x, y) in &dug {
        let (lx, ly) = w.leaf_of(x, y);
        assert!(leaves.binary_search(&(lx, ly)).is_ok(), "leaf of {x},{y} not listed");
        let snap = w.leaf_snapshot(lx, ly).expect("a dug leaf is stored");
        let (x0, y0) = w.leaf_corner(lx, ly);
        assert_eq!(snap.at(x - x0, y - y0), w.material(x, y));
    }
    // And every cell of every listed leaf agrees.
    for &(lx, ly) in leaves.iter().take(50) {
        let snap = w.leaf_snapshot(lx, ly).unwrap();
        let (x0, y0) = w.leaf_corner(lx, ly);
        for j in (0..64).step_by(7) {
            for i in (0..64).step_by(5) {
                assert_eq!(snap.at(i, j), w.material(x0 + i, y0 + j));
            }
        }
    }
}
