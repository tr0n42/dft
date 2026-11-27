//! A sapper's buried blast (the crater thrown out, the shock's cracks, the
//! loosened column) makes and loses nothing: what was blown out is all still
//! on the body (on the ground, or in the air on its way).
use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
use dft_sim::swarm::Sim;
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
}

/// How a blast's work is spread over the ticks as the yield grows (run by
/// hand: cargo test --release --test sapper_blast -- --ignored --nocapture):
/// how many ticks the blast took to run its course; the dearest single tick
/// (the whole step, and the blast's own work in it) and a typical one (the
/// median of the blast's work); and the time in all.
#[test]
#[ignore]
fn blast_timings() {
    for kt in [10.0, 100.0, 1000.0, 3000.0, 10000.0] {
        let mut s = sim();
        let t = std::time::Instant::now();
        s.test_blast((0.5, -(s.world.planet.max_radius + 5.0)), kt);
        let mut worst = t.elapsed().as_secs_f64();
        let mut work = vec![s.prof2[7]];
        while s.blasts_pending() > 0 {
            let (t, b) = (std::time::Instant::now(), s.prof2[7]);
            s.step();
            worst = worst.max(t.elapsed().as_secs_f64());
            work.push(s.prof2[7] - b);
        }
        let ticks = work.len() - 1;
        work.sort_by(|a, b| a.total_cmp(b));
        eprintln!(
            "{kt} kt: {ticks} ticks ({:.1} game s); worst tick {:.1} ms (blast work in it at most {:.1} ms, typically {:.1}); blast work {:.0} ms in all, {:.0} ms with the rest of each step",
            ticks as f64 * s.eco.tick_s,
            worst * 1000.0,
            work[work.len() - 1] * 1000.0,
            work[work.len() / 2] * 1000.0,
            s.prof2[7] * 1000.0,
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
}
