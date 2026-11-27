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
        // turn.)
        let mut by: std::collections::HashMap<u32, Vec<(i64, i64)>> = Default::default();
        for a in s.bodies.drawn() {
            if a.angle != 0.0 {
                by.entry(a.angle.to_bits()).or_default().push(a.home);
            }
        }
        for cells in by.values() {
            assert!(joined(cells), "tick {t}: a body of {} cells in pieces: {cells:?}", cells.len());
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
