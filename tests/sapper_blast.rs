//! A sapper's buried blast (the crater thrown out, the shock's cracks, the
//! loosened column) makes and loses nothing: what was blown out is all still
//! on the body (on the ground, or in the air on its way).
use dft_sim::economy::Economy;
use dft_sim::materials::VACUUM;
use dft_sim::save::{R, W};
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
        most = most.max(s.bodies.drops());
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
        let mut work = vec![s.prof2[7]];
        let mut steps = Vec::new();
        let (mut ticks, mut after, mut most) = (0, 0, 0);
        while s.blasts_pending() > 0 || after < 600 {
            let (t, b, f, k, p) = (std::time::Instant::now(), s.prof2[7], s.prof2[2], s.prof2[6], s.prof[3]);
            s.step();
            let e = t.elapsed().as_secs_f64();
            steps.push(e);
            if e > worst.0 {
                worst = (e, s.prof2[7] - b, s.prof2[2] - f, s.prof2[6] - k, s.prof[3] - p);
            }
            most = most.max(s.bodies.drops());
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
