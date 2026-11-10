use dft_sim::totals::{self, Node, Tree};
use std::path::Path;

fn body(name: &str) -> dft_sim::planet::Planet {
    dft_sim::load_body(Path::new(env!("CARGO_MANIFEST_DIR")).join("data").as_path(), name).unwrap()
}

#[test]
fn same_seed_same_world() {
    let (a, b) = (body("rubble-asteroid"), body("rubble-asteroid"));
    let mut h = 0u64;
    for k in 0..20_000i64 {
        let (x, y) = ((k * 7919) % 5000 - 2500, (k * 104729) % 5000 - 2500);
        assert_eq!(a.material(x, y), b.material(x, y));
        h = dft_sim::noise::hash(h, a.material(x, y) as i64, k, 1);
    }
    println!("  fingerprint same_seed_same_world: {h:016x}");
}

#[test]
fn different_seed_different_world() {
    let a = body("rubble-asteroid");
    let mut spec = a.spec.clone();
    spec.seed += 1;
    let b = dft_sim::planet::Planet::new(spec, a.mats.clone()).unwrap();
    let differ = (0..20_000i64)
        .filter(|k| {
            let (x, y) = ((k * 7919) % 5000 - 2500, (k * 104729) % 5000 - 2500);
            a.material(x, y) != b.material(x, y)
        })
        .count();
    assert!(differ > 1000, "only {differ} of 20000 cells differ");
}

#[test]
fn heart_is_the_centre_cell_only() {
    let p = body("active-planetoid");
    let heart = p.mats.id("heart").unwrap();
    assert_eq!(p.material(0, 0), heart);
    for (x, y) in [(1, 0), (-1, 0), (0, 1), (0, -1), (-1, -1)] {
        assert_ne!(p.material(x, y), heart);
    }
    let a = body("rubble-asteroid");
    assert_ne!(a.material(0, 0), a.mats.id("heart").unwrap());
}

#[test]
fn outside_the_body_is_vacuum() {
    let p = body("active-planetoid");
    let r = p.max_radius as i64 + 5;
    for (x, y) in [(r, 0), (0, r), (-r, 0), (0, -r), (r, r)] {
        assert_eq!(p.material(x, y), 0);
    }
}

#[test]
fn parts_sum_to_the_whole() {
    let p = body("rubble-asteroid");
    let t = Tree::for_planet(&p);
    // Several nodes across the body, at two levels.
    for (level, ix, iy) in [(3, 4, 4), (3, 6, 7), (4, 2, 3), (2, 13, 15)] {
        let n = Node { level, ix, iy };
        let parent = totals::exact(&p, &t, n);
        let mut sum = totals::Totals::zero(p.mats.len());
        for c in n.children() {
            sum.add(&totals::exact(&p, &t, c));
        }
        assert_eq!(parent, sum, "node {n:?}");
    }
}

#[test]
fn estimate_is_close_to_exact() {
    let p = body("rubble-asteroid");
    let t = Tree::for_planet(&p);
    let n = Node { level: 5, ix: 1, iy: 1 };
    let ex = totals::exact(&p, &t, n).total_kg(&p.mats);
    let es = totals::estimate(&p, &t, n, 256).total_kg(&p.mats);
    let err = (es - ex).abs() / ex;
    assert!(err < 0.02, "estimate off by {:.2}%", err * 100.0);
}

#[test]
fn compositions_sum_to_one() {
    let p = body("rubble-asteroid");
    for m in &p.mats.list {
        let s: f64 = m.composition.values().sum();
        assert!(m.composition.is_empty() || (s - 1.0).abs() < 1e-9, "{}", m.name);
    }
}

#[test]
fn pressure_and_gravity_match_a_uniform_sphere() {
    // One basalt layer, 1,000 km radius: a uniform sphere, where the answers
    // are known exactly: g = 4/3 π G ρ R at the surface, P = 2/3 π G ρ² R² at
    // the centre, and P = 2/3 π G ρ² (R² − r²) in between.
    let a = body("rubble-asteroid");
    let mut spec = a.spec.clone();
    spec.radius_m = 1.0e6;
    spec.layer.truncate(1);
    spec.layer[0].outer = 1.0;
    spec.layer[0].material = "basalt".into();
    let p = dft_sim::planet::Planet::new(spec, a.mats.clone()).unwrap();
    let (g, rho, r) = (6.674e-11, p.mats.get(p.mats.id("basalt").unwrap()).density, 1.0e6);
    let pi = std::f64::consts::PI;
    let gs = 4.0 / 3.0 * pi * g * rho * r;
    let pc = 2.0 / 3.0 * pi * g * rho * rho * r * r;
    let ph = 2.0 / 3.0 * pi * g * rho * rho * (r * r - 0.25 * r * r);
    println!("  surface g {:.4} (exact {gs:.4}); centre {:.4e} Pa (exact {pc:.4e}); half radius {:.4e} (exact {ph:.4e})",
        p.real_surface_gravity(), p.pressure_pa(0, 0), p.pressure_pa(500_000, 0));
    assert!((p.real_surface_gravity() / gs - 1.0).abs() < 1e-3);
    assert!((p.pressure_pa(0, 0) / pc - 1.0).abs() < 1e-3);
    assert!((p.pressure_pa(500_000, 0) / ph - 1.0).abs() < 1e-3);
    assert_eq!(p.pressure_pa(1_000_100, 0), 0.0);

    for name in ["rubble-asteroid", "test-planetoid-3km", "active-planetoid"] {
        let b = body(name);
        println!("  {name}: real surface g {:.5} m/s², game g {} m/s², centre pressure {:.3} MPa",
            b.real_surface_gravity(), b.gravity, b.pressure_pa(0, 0) / 1e6);
    }
}

#[test]
fn strata_follow_the_surface_and_faults_step_them() {
    let t = std::time::Instant::now();
    let p = body("strata-demo");
    let load_ms = t.elapsed().as_millis();
    let st = p.strata.as_ref().unwrap();
    println!("  strata demo: {} bands, {} directions, loaded in {load_ms} ms", st.bands.len(), st.directions());
    assert!(load_ms < 2000);
    // Near the surface, almost every solid cell belongs to a strata band;
    // deep down, none does.
    let band_mats: std::collections::HashSet<_> = st.all_bands().into_iter().collect();
    let (mut near, mut near_band, mut deep_band) = (0, 0, 0);
    let mut h = 0u64;
    for k in 0..20_000i64 {
        let (x, y) = ((k * 7919) % 4000 - 2000, (k * 104729) % 4000 - 2000);
        let m = p.material(x, y);
        h = dft_sim::noise::hash(h, m as i64, k, 2);
        let r = ((x * x + y * y) as f64).sqrt();
        if r > 1700.0 && r < 1850.0 && m != 0 {
            near += 1;
            if band_mats.contains(&m) {
                near_band += 1;
            }
        }
        if r < 1400.0 && band_mats.contains(&m) && p.mats.get(m).name != "basalt" {
            deep_band += 1;
        }
    }
    println!("  fingerprint strata_demo: {h:016x}; {near_band} of {near} near-surface cells in strata");
    // The rest is soil, ice caps, lenses, intrusions and crater breccia.
    assert!(near_band * 10 > near * 8);
    assert_eq!(deep_band, 0);
    // Faults move rock: some cells see a shift.
    let fs = p.faults.as_ref().unwrap();
    let moved = (0..4000i64).filter(|k| fs.shift((k % 4000 - 2000) as f64, 1900.0).0 != 0.0).count();
    assert!(moved > 0);
}

#[test]
fn ore_is_zoned_around_plutons() {
    // Sample the demo's crust and record how far each ore cell lies from the
    // nearest granite: molybdenum should sit close in, mercury far out.
    let t = std::time::Instant::now();
    let p = body("strata-demo");
    println!("  strata demo with intrusions loaded in {} ms", t.elapsed().as_millis());
    let (np, ns) = p.intrusions.as_ref().unwrap().counts();
    println!("  {np} plutons, {ns} dikes and veins");
    let id = |n: &str| p.mats.id(n).unwrap();
    let granite = id("granite");
    let mut h = 0u64;
    let mut count = std::collections::BTreeMap::new();
    // Every 2nd cell of the outer 700 m of the body.
    let mut granite_cells = Vec::new();
    let mut ore_cells: Vec<(dft_sim::materials::MatId, i64, i64)> = Vec::new();
    for y in (-2050i64..2050).step_by(2) {
        for x in (-2050i64..2050).step_by(2) {
            let r2 = x * x + y * y;
            if r2 < 1300 * 1300 || r2 > 2050 * 2050 {
                continue;
            }
            let m = p.material(x, y);
            h = dft_sim::noise::hash(h, m as i64, x, y as u64);
            if m == granite {
                if (x + y) % 16 == 0 {
                    granite_cells.push((x, y));
                }
            } else {
                *count.entry(p.mats.get(m).name.clone()).or_insert(0u32) += 1;
                for n in ["molybdenite", "galena", "gold-quartz", "cinnabar", "stibnite"] {
                    if m == id(n) {
                        ore_cells.push((m, x, y));
                    }
                }
            }
        }
    }
    println!("  fingerprint intrusions: {h:016x}");
    for n in ["marble", "slate", "quartzite", "andesite", "gabbro", "chromite", "magnetite", "garnet", "quartz crystal"] {
        println!("    {n:<15} {:>6} cells", count.get(n).copied().unwrap_or(0));
        assert!(count.get(n).copied().unwrap_or(0) > 0, "no {n} formed");
    }
    let mean_dist = |name: &str| {
        let m = id(name);
        let ds: Vec<f64> = ore_cells
            .iter()
            .filter(|c| c.0 == m)
            .map(|c| granite_cells.iter().map(|g| (((g.0 - c.1).pow(2) + (g.1 - c.2).pow(2)) as f64).sqrt()).fold(f64::MAX, f64::min))
            .collect();
        (ds.iter().sum::<f64>() / ds.len().max(1) as f64, ds.len())
    };
    let mut last = 0.0;
    for n in ["stibnite", "cinnabar"] {
        let (d, k) = mean_dist(n);
        println!("    {n:<15} {k:>5} cells, mean {d:>6.1} m from granite (far zone)");
    }
    for n in ["molybdenite", "galena", "gold-quartz"] {
        let (d, k) = mean_dist(n);
        println!("    {n:<15} {k:>5} cells, mean {d:>6.1} m from granite");
        assert!(k > 0, "no {n} in veins");
        assert!(d > last, "{n} should lie farther out than the ore before it");
        last = d;
    }
}

#[test]
fn g3_rubble_lenses_caves_impacts_and_ice() {
    let t = std::time::Instant::now();
    let rb = body("rubble-demo");
    let geo = body("strata-demo");
    println!("  rubble and geology demos loaded in {} ms", t.elapsed().as_millis());
    let mut h = 0u64;
    // Rubble pile: a mix of rocks, dusty gaps, some empty spaces, breccia under craters.
    let mut counts = std::collections::BTreeMap::new();
    for y in (-1150i64..1150).step_by(3) {
        for x in (-1150i64..1150).step_by(3) {
            let m = rb.material(x, y);
            h = dft_sim::noise::hash(h, m as i64, x, y as u64);
            *counts.entry(rb.mats.get(m).name.clone()).or_insert(0u32) += 1;
        }
    }
    for n in ["chondrite", "basalt", "dirty ice", "iron-nickel", "troilite", "pallasite", "regolith", "impact breccia"] {
        println!("    rubble {n:<15} {:>7}", counts.get(n).copied().unwrap_or(0));
        assert!(counts.get(n).copied().unwrap_or(0) > 0, "no {n} in the rubble pile");
    }
    // Geology demo: lenses, banded iron, gold in the gravels, ice only near the poles.
    let ice = geo.mats.id("ice").unwrap();
    let mut gcounts = std::collections::BTreeMap::new();
    for y in (-2050i64..2050).step_by(2) {
        for x in (-2050i64..2050).step_by(2) {
            let r2 = x * x + y * y;
            if r2 < 1500 * 1500 {
                continue;
            }
            let m = geo.material(x, y);
            h = dft_sim::noise::hash(h, m as i64, x, y as u64);
            *gcounts.entry(geo.mats.get(m).name.clone()).or_insert(0u32) += 1;
            if m == ice {
                let s = (y as f64 + 0.5).abs() / (r2 as f64).sqrt();
                assert!(s > 0.79, "ice at latitude sine {s:.2}");
            }
        }
    }
    // (Placer gold is still there, but rare now: it is checked as possible.)
    assert!(geo.possible_materials()[geo.mats.id("gold").unwrap() as usize], "no gold possible in the geology demo");
    for n in ["gravel", "rock salt", "gypsum", "banded iron", "ice", "impact breccia", "obsidian", "soil"] {
        println!("    geology {n:<15} {:>7}", gcounts.get(n).copied().unwrap_or(0));
        assert!(gcounts.get(n).copied().unwrap_or(0) > 0, "no {n} in the geology demo");
    }
    println!("  fingerprint g3: {h:016x}");
}

#[test]
fn deep_interior_has_zones_streaks_plumes_slabs_and_a_rich_core() {
    let t = std::time::Instant::now();
    let p = body("test-planetoid-3km");
    println!("  test planetoid with deep interior loaded in {} ms", t.elapsed().as_millis());
    let mut h = 0u64;
    let mut counts = std::collections::BTreeMap::new();
    let (plume, slab) = (p.mats.id("plume rock").unwrap(), p.mats.id("eclogite").unwrap());
    let (mut hot, mut cold) = ((0.0, 0u32), (0.0, 0u32));
    for y in (-1500i64..1500).step_by(3) {
        for x in (-1500i64..1500).step_by(3) {
            let m = p.material(x, y);
            h = dft_sim::noise::hash(h, m as i64, x, y as u64);
            *counts.entry(p.mats.get(m).name.clone()).or_insert(0u32) += 1;
            // Compare each plume or slab cell with the rock 120 m to the side.
            if m == plume || m == slab {
                let d = p.ambient_k(x, y) - p.ambient_k(x + 120, y);
                if m == plume { hot.0 += d; hot.1 += 1; } else { cold.0 += d; cold.1 += 1; }
            }
        }
    }
    for n in ["perovskite rock", "ringwoodite rock", "harzburgite", "pyroxenite", "plume rock", "eclogite",
              "deep pile rock", "iron crystal", "platinum-iron alloy", "sulfur-rich core melt"] {
        println!("    {n:<22} {:>7}", counts.get(n).copied().unwrap_or(0));
        assert!(counts.get(n).copied().unwrap_or(0) > 0, "no {n}");
    }
    let (hot_k, cold_k) = (hot.0 / hot.1.max(1) as f64, cold.0 / cold.1.max(1) as f64);
    println!("    plumes {hot_k:+.0} K, slabs {cold_k:+.0} K against the rock beside them");
    assert!(hot_k > 50.0 && cold_k < -50.0);
    println!("  fingerprint deep: {h:016x}");
}

#[test]
fn g5_every_seed_gets_its_own_surface() {
    // Five seeds of the test planetoid: each draws its own features, so the
    // surfaces differ in shape, and each has real relief (tens of metres).
    let base = body("test-planetoid-3km");
    let mut shapes = Vec::new();
    for k in 0..5u64 {
        let mut spec = base.spec.clone();
        spec.seed += k * 977;
        let t = std::time::Instant::now();
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let ms = t.elapsed().as_millis();
        let rel = p.relief.as_ref().unwrap();
        let kinds: Vec<String> = rel.features.iter().map(|f| format!("{:?}", f.kind)).collect();
        // Surface offsets at 720 directions.
        let offs: Vec<f64> = (0..720).map(|i| {
            let (ux, uy) = dft_sim::geology::random_dir(99, i);
            rel.offset(ux, uy)
        }).collect();
        let (lo, hi) = offs.iter().fold((f64::MAX, f64::MIN), |a, &o| (a.0.min(o), a.1.max(o)));
        println!("  seed {}: {} features {:?}, relief {:.0} to {:+.0} m, loaded in {ms} ms", p.spec.seed, kinds.len(), kinds, lo, hi);
        assert!(hi - lo > 20.0, "seed {} is nearly flat", p.spec.seed);
        shapes.push(offs);
    }
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            let diff: f64 = shapes[i].iter().zip(&shapes[j]).map(|(a, b)| (a - b).abs()).sum::<f64>() / 720.0;
            assert!(diff > 2.0, "seeds {i} and {j} have nearly the same surface");
        }
    }
}

#[test]
fn g5_volcanoes_have_plumbing() {
    // Under every volcano: a chamber of molten magma (kept hot) if it is
    // active, frozen rock if not; hotspots sit over the deep plumes.
    let base = body("strata-demo");
    let magma = base.mats.id("magma").unwrap();
    let (mut live, mut dead, mut hotspots) = (0, 0, 0);
    for k in 0..4u64 {
        let mut spec = base.spec.clone();
        spec.seed += k * 977;
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        let rel = p.relief.as_ref().unwrap();
        let centres = rel.chamber_centres();
        for (f, &(cx, cy)) in rel.features.iter().filter(|f| f.kind == dft_sim::relief::Kind::Volcano).zip(&centres) {
            let (x, y) = (cx.floor() as i64, cy.floor() as i64);
            let m = p.material(x, y);
            if f.active {
                assert_eq!(m, magma, "seed {}: active chamber holds {}", p.spec.seed, p.mats.get(m).name);
                assert!(p.ambient_k(x, y) >= 1400.0, "magma chamber is only {:.0} K", p.ambient_k(x, y));
                live += 1;
            } else {
                assert_eq!(m, f.chamber, "seed {}: dead chamber holds {}", p.spec.seed, p.mats.get(m).name);
                dead += 1;
            }
            if f.feed_r > 0.0 {
                hotspots += 1;
            }
        }
    }
    println!("  {live} live and {dead} dead volcanoes, {hotspots} over plumes");
    assert!(live > 0 && dead > 0 && hotspots > 0);
}

#[test]
fn g5_an_older_era_lies_under_an_unconformity() {
    // The geology demo has an old folded stack (quartzite, slate, marble,
    // schist) worn flat under the young layers, with gravel on the worn surface.
    let base = body("strata-demo");
    let old: Vec<_> = ["quartzite", "slate", "marble", "schist"].iter().map(|n| base.mats.id(n).unwrap()).collect();
    let gravel = base.mats.id("conglomerate").unwrap();
    let p = dft_sim::planet::Planet::new(base.spec.clone(), base.mats.clone()).unwrap();
    // Depth bands below the surface: how many sampled cells are old rock.
    let (mut shallow_old, mut deep_old, mut seen_gravel, mut n) = (0, 0, 0, 0);
    for i in 0..360 {
        let (ux, uy) = dft_sim::geology::random_dir(4242, i);
        let s = p.surface_radius(ux, uy);
        for k in 0..60 {
            let depth = 5.0 + k as f64 * 5.0;
            let r = s - depth;
            let m = p.material((ux * r).floor() as i64, (uy * r).floor() as i64);
            if old.contains(&m) {
                if depth < 40.0 { shallow_old += 1 } else { deep_old += 1 }
            }
            if m == gravel {
                seen_gravel += 1;
            }
            n += 1;
        }
    }
    println!("  {deep_old} old-rock samples deeper than 40 m, {shallow_old} shallower, {seen_gravel} conglomerate, of {n}");
    assert!(deep_old > n / 10, "the old stack barely shows");
    assert!(shallow_old * 20 < deep_old, "old rock reaches the surface too often");
    assert!(seen_gravel > 0);
}

#[test]
fn metal_asteroid_has_an_etched_iron_core() {
    // The iron core shows the Widmanstätten structure (kamacite plates edged
    // with taenite, plessite between), with nodules and needles in it; the
    // pallasite holds olivine; the shell is broken mantle rock.
    let p = body("metal-asteroid");
    let mut n: std::collections::BTreeMap<String, u32> = Default::default();
    let mut h = 0u64;
    for k in 0..40_000i64 {
        let (x, y) = ((k * 7919) % 6000 - 3000, (k * 104729) % 6000 - 3000);
        let m = p.material(x, y);
        h = dft_sim::noise::hash(h, m as i64, k, 9);
        *n.entry(p.mats.get(m).name.clone()).or_default() += 1;
    }
    println!("  fingerprint metal_asteroid: {h:016x}");
    for (k, v) in &n {
        println!("    {k:<16} {v}");
    }
    let get = |s: &str| n.get(s).copied().unwrap_or(0);
    assert!(get("kamacite") > get("taenite") && get("taenite") > 0 && get("plessite") > 0);
    for m in ["troilite", "olivine", "pallasite", "harzburgite", "schreibersite"] {
        assert!(get(m) > 0, "no {m}");
    }
    assert_eq!(get("iron-nickel") > 0, true, "metal boulders in the shell");
}

#[test]
fn comet_is_layered_ice_over_a_pebble_heap() {
    // Onion layers of ices, dust and organics over a heap of icy pebbles,
    // with CO ice and trapped gas deep inside and a dark dust crust.
    let p = body("comet");
    let mut n: std::collections::BTreeMap<String, u32> = Default::default();
    let mut h = 0u64;
    for k in 0..40_000i64 {
        let (x, y) = ((k * 7919) % 6000 - 3000, (k * 104729) % 6000 - 3000);
        let m = p.material(x, y);
        h = dft_sim::noise::hash(h, m as i64, k, 9);
        *n.entry(p.mats.get(m).name.clone()).or_default() += 1;
    }
    println!("  fingerprint comet: {h:016x}");
    for (k, v) in &n {
        println!("    {k:<20} {v}");
    }
    let get = |s: &str| n.get(s).copied().unwrap_or(0);
    for m in ["dirty ice", "ice", "dry ice", "comet dust", "tholin", "ammonia ice", "carbon monoxide ice", "carbon monoxide"] {
        assert!(get(m) > 0, "no {m}");
    }
}

#[test]
fn rogue_world_keeps_an_ocean_under_its_ice() {
    // Frozen air on top, a thick ice shell, a liquid brine ocean under it kept
    // warm by the rock below, and cryovolcanoes whose live chambers hold
    // brine just above freezing.
    let p = body("rogue-world");
    let brine = p.mats.id("brine").unwrap();
    let ocean_r = 0.81 * p.radius;
    let at = |r: f64| p.material(0, r as i64);
    assert_eq!(at(ocean_r), brine, "the ocean is liquid");
    assert!(p.ambient_k(0, ocean_r as i64) > 252.0, "and warm enough to stay so");
    let mut n: std::collections::BTreeMap<String, u32> = Default::default();
    for i in 0..4000 {
        let (ux, uy) = dft_sim::geology::random_dir(77, i);
        let s = p.surface_radius(ux, uy);
        for d in [2.0, 10.0, 60.0, 200.0, 500.0] {
            let r = s - d;
            *n.entry(p.mats.get(p.material((ux * r) as i64, (uy * r) as i64)).name.clone()).or_default() += 1;
        }
    }
    println!("  near-surface: {n:?}");
    for m in ["nitrogen ice", "methane ice", "air snow"] {
        assert!(n.get(m).copied().unwrap_or(0) > 0, "no {m} near the surface");
    }
    let rel = p.relief.as_ref().unwrap();
    let mut live = 0;
    for (f, &(cx, cy)) in rel.features.iter().filter(|f| f.kind == dft_sim::relief::Kind::Volcano).zip(&rel.chamber_centres()) {
        if f.active {
            let (x, y) = (cx.floor() as i64, cy.floor() as i64);
            assert_eq!(p.material(x, y), brine, "a live cryovolcano holds brine");
            let t = p.ambient_k(x, y);
            assert!(t > 252.0 && t < 273.0, "brine chamber at {t:.0} K");
            live += 1;
        }
    }
    println!("  {live} live cryovolcanoes");
}

/// Material counts over a coarse grid of a body's cells within `r` of the centre.
fn census(p: &dft_sim::planet::Planet, r: i64) -> (std::collections::BTreeMap<String, u32>, u64) {
    let mut n: std::collections::BTreeMap<String, u32> = Default::default();
    let mut h = 0u64;
    for k in 0..60_000i64 {
        let (x, y) = ((k * 7919) % (2 * r) - r, (k * 104729) % (2 * r) - r);
        let m = p.material(x, y);
        h = dft_sim::noise::hash(h, m as i64, k, 9);
        *n.entry(p.mats.get(m).name.clone()).or_default() += 1;
    }
    (n, h)
}

/// The same near the surface: `samples` directions, at several depths.
fn surface_census(p: &dft_sim::planet::Planet, depths: &[f64]) -> std::collections::BTreeMap<String, u32> {
    let mut n: std::collections::BTreeMap<String, u32> = Default::default();
    for i in 0..3000 {
        let (ux, uy) = dft_sim::geology::random_dir(91, i);
        let s = p.surface_radius(ux, uy);
        for d in depths {
            let r = s - d;
            *n.entry(p.mats.get(p.material((ux * r) as i64, (uy * r) as i64)).name.clone()).or_default() += 1;
        }
    }
    n
}

#[test]
fn sulfur_world_is_sulfur_and_magma_under_frost() {
    let p = body("sulfur-world");
    let (_, h) = census(&p, 26_000);
    println!("  fingerprint sulfur_world: {h:016x}");
    let n = surface_census(&p, &[2.0, 30.0, 150.0, 600.0, 1500.0]);
    println!("  near-surface: {n:?}");
    for m in ["sulfur", "sulfur dioxide frost", "basalt", "magma", "volcanic ash", "pyrite"] {
        assert!(n.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
}

#[test]
fn iron_world_is_mostly_core() {
    let p = body("iron-world");
    let (n, h) = census(&p, 31_000);
    println!("  fingerprint iron_world: {h:016x}");
    let get = |s: &str| n.get(s).copied().unwrap_or(0);
    let core = get("molten-iron") + get("iron-nickel") + get("iron crystal") + get("sulfur-rich core melt") + get("platinum-iron alloy");
    let rock = get("peridotite") + get("gabbro") + get("basalt");
    println!("  core {core} cells, mantle and crust {rock}");
    assert!(core > 2 * rock, "the core should dominate");
    let s = surface_census(&p, &[2.0, 40.0, 200.0, 600.0]);
    println!("  near-surface: {s:?}");
    for m in ["graphite", "impact breccia", "basalt"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
}

#[test]
fn carbon_world_has_more_diamond_deeper() {
    let p = body("carbon-world");
    let (_, h) = census(&p, 21_000);
    println!("  fingerprint carbon_world: {h:016x}");
    let diamond = p.mats.id("diamond").unwrap();
    // Share of diamond in the graphite mantle, by depth band.
    let share = |f0: f64, f1: f64| {
        let (mut d, mut n) = (0, 0);
        for i in 0..20_000 {
            let (ux, uy) = dft_sim::geology::random_dir(5, i);
            let f = f0 + (f1 - f0) * ((i % 97) as f64 / 97.0);
            let r = f * p.radius;
            if p.material((ux * r) as i64, (uy * r) as i64) == diamond {
                d += 1;
            }
            n += 1;
        }
        d as f64 / n as f64
    };
    let (deep, mid, shallow) = (share(0.63, 0.70), share(0.71, 0.80), share(0.81, 0.89));
    println!("  diamond share: deep {deep:.3}, middle {mid:.3}, shallow {shallow:.3}");
    assert!(deep > mid && mid > shallow && shallow > 0.0);
    let s = surface_census(&p, &[2.0, 30.0, 150.0, 600.0]);
    println!("  near-surface: {s:?}");
    for m in ["graphite", "silicon carbide", "tar"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
}

#[test]
fn methane_world_has_liquid_seas_on_ice() {
    let p = body("methane-world");
    let (_, h) = census(&p, 31_000);
    println!("  fingerprint methane_world: {h:016x}");
    let s = surface_census(&p, &[1.0, 5.0, 20.0, 100.0, 400.0]);
    println!("  near-surface: {s:?}");
    for m in ["liquid methane", "liquid ethane", "tholin", "ice", "dirty ice"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    // The lakes stay liquid: their own cold is above methane's freezing point.
    let lm = p.mats.id("liquid methane").unwrap();
    for i in 0..3000 {
        let (ux, uy) = dft_sim::geology::random_dir(91, i);
        let r = p.surface_radius(ux, uy) - 5.0;
        let (x, y) = ((ux * r) as i64, (uy * r) as i64);
        if p.material(x, y) == lm {
            let t = p.start_k(x, y, lm);
            assert!(t > 89.0 && t < 112.0, "methane at {t:.0} K");
            break;
        }
    }
}

#[test]
fn ice_moon_is_an_ocean_under_a_shell() {
    let p = body("ice-moon");
    let (n, h) = census(&p, 10_500);
    println!("  fingerprint ice_moon: {h:016x}");
    println!("  census: {n:?}");
    for m in ["brine", "ice", "peridotite", "iron-nickel"] {
        assert!(n.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    let s = surface_census(&p, &[1.0, 50.0, 200.0, 500.0]);
    println!("  near-surface: {s:?}");
    for m in ["ice", "sea ice"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
}

#[test]
fn ocean_world_is_deep_water_over_ice_over_rock() {
    let p = body("ocean-world");
    let (n, h) = census(&p, 41_000);
    println!("  fingerprint ocean_world: {h:016x}");
    println!("  census: {n:?}");
    for m in ["water", "pressure ice", "hot ice", "peridotite"] {
        assert!(n.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    // Water is the biggest layer by area.
    let w = n["water"];
    assert!(n.iter().all(|(k, v)| k == "water" || k == "vacuum" || *v <= w), "{n:?}");
}

#[test]
fn lava_world_is_a_magma_ocean_under_glass() {
    let p = body("lava-world");
    let (n, h) = census(&p, 21_000);
    println!("  fingerprint lava_world: {h:016x}");
    println!("  census: {n:?}");
    for m in ["magma", "perovskite rock", "molten-iron"] {
        assert!(n.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    let s = surface_census(&p, &[1.0, 20.0, 100.0, 300.0]);
    println!("  near-surface: {s:?}");
    for m in ["obsidian", "volcanic ash"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
}

#[test]
fn barren_world_is_cratered_highland_rock_with_dark_basins() {
    let p = body("barren-world");
    let (n, h) = census(&p, 21_000);
    println!("  fingerprint barren_world: {h:016x}");
    for m in ["anorthosite", "peridotite", "molten-iron", "iron-nickel"] {
        assert!(n.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    let s = surface_census(&p, &[1.0, 20.0, 100.0, 300.0]);
    println!("  near-surface: {s:?}");
    for m in ["regolith", "anorthosite", "impact breccia", "basalt"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    // No water anywhere.
    assert!(!n.contains_key("water") && !s.contains_key("water") && !s.contains_key("ice"));
}

#[test]
fn dry_world_is_red_dust_over_old_wet_sediments_and_ground_ice() {
    let p = body("dry-world");
    let (_, h) = census(&p, 31_000);
    println!("  fingerprint dry_world: {h:016x}");
    let s = surface_census(&p, &[1.0, 20.0, 60.0, 150.0, 400.0]);
    println!("  near-surface: {s:?}");
    for m in ["red dust", "basalt", "sandstone", "dirty ice"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    // Frozen, never liquid, water.
    assert!(!s.contains_key("water"));
}

#[test]
fn hothouse_world_is_young_lava_plains_kept_hot() {
    let p = body("hothouse-world");
    let (_, h) = census(&p, 31_000);
    println!("  fingerprint hothouse_world: {h:016x}");
    let s = surface_census(&p, &[1.0, 20.0, 100.0, 300.0]);
    println!("  near-surface: {s:?}");
    for m in ["basalt", "volcanic ash"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    // The ground starts at the scorching surface heat.
    let (ux, uy) = (0.6f64, 0.8f64);
    let r = p.surface_radius(ux, uy) - 2.0;
    let t = p.ambient_k((ux * r) as i64, (uy * r) as i64);
    assert!(t > 700.0, "surface at {t:.0} K");
}

#[test]
fn living_world_has_a_level_sea_over_basalt_and_land_of_sediments() {
    let p = body("living-world");
    let (_, h) = census(&p, 36_000);
    println!("  fingerprint living_world: {h:016x}");
    let s = surface_census(&p, &[1.0, 20.0, 100.0, 400.0]);
    println!("  near-surface: {s:?}");
    for m in ["soil", "water", "sandstone", "shale", "limestone"] {
        assert!(s.get(m).copied().unwrap_or(0) > 0, "no {m}");
    }
    // The sea's surface is level: every open-water direction meets the sky
    // at the same radius.
    let (water, level) = p.sea().expect("a sea");
    let mut seen = 0;
    for i in 0..2000 {
        let a = std::f64::consts::TAU * i as f64 / 2000.0;
        let (ux, uy) = (a.sin(), a.cos());
        let top = p.surface_radius(ux, uy);
        let r = top - 0.5;
        if p.relief.as_ref().unwrap().sea_depth(ux, uy) > 1.0 && p.material((ux * r).floor() as i64, (uy * r).floor() as i64) == water {
            assert!((top - level).abs() < 0.01, "sea surface at {top:.2}, level {level:.2}");
            seen += 1;
        }
    }
    assert!(seen > 300, "only {seen} directions of open sea");
    // Basalt floors it, and the water is at the surface's temperature from
    // top to bottom (and so is the sea floor).
    let (ux, uy) = (0..2000)
        .map(|i| std::f64::consts::TAU * i as f64 / 2000.0)
        .map(|a| (a.sin(), a.cos()))
        .find(|&(ux, uy)| p.relief.as_ref().unwrap().sea_depth(ux, uy) > 1200.0)
        .expect("deep sea");
    let d = p.relief.as_ref().unwrap().sea_depth(ux, uy);
    let floor = level - d - 20.0;
    assert_eq!(p.mats.get(p.material((ux * floor) as i64, (uy * floor) as i64)).name, "basalt");
    let deep = level - d + 5.0;
    let tw = p.ambient_k((ux * deep) as i64, (uy * deep) as i64);
    let tf = p.ambient_k((ux * floor) as i64, (uy * floor) as i64);
    assert!((tw - 288.0).abs() < 31.0 && tf < 320.0, "sea {tw:.0} K, floor {tf:.0} K");
    // No life yet.
    for m in ["wood", "plant", "tree"] {
        assert!(p.mats.id(m).is_err() || !s.contains_key(m));
    }
}

fn with_glamour(name: &str, g: f64) -> dft_sim::planet::Planet {
    let p = body(name);
    let mut spec = p.spec.clone();
    spec.glamour = Some(g);
    dft_sim::planet::Planet::new(spec, p.mats.clone()).unwrap()
}

#[test]
fn glamour_zero_is_the_locked_model_and_more_adds_variety_deep_down() {
    for name in ["active-planetoid", "living-world", "ice-moon"] {
        let real = body(name);
        let zero = with_glamour(name, 0.0);
        assert_eq!(dft_sim::reference::fingerprint(&real).0, dft_sim::reference::fingerprint(&zero).0, "{name}: glamour 0 changed the body");
        let glam = with_glamour(name, 0.5);
        // Deep down: many more kinds of rock.
        let kinds = |p: &dft_sim::planet::Planet| {
            let mut s = std::collections::BTreeSet::new();
            for k in 0..20_000i64 {
                let (ux, uy) = dft_sim::geology::random_dir(77, k);
                let f = 0.35 + 0.5 * dft_sim::noise::unit(dft_sim::noise::hash(78, k, 0, 0));
                let r = f * p.radius;
                s.insert(p.material((ux * r) as i64, (uy * r) as i64));
            }
            s.len()
        };
        let (a, b) = (kinds(&real), kinds(&glam));
        println!("  {name}: {a} materials deep down, {b} with glamour 0.5");
        assert!(b >= a + 6, "{name}: glamour added too little ({a} -> {b})");
        // The surface and the crust just under it are untouched.
        for i in 0..2000 {
            let (ux, uy) = dft_sim::geology::random_dir(79, i);
            for d in [0.5, 5.0, 30.0] {
                let r = real.surface_radius(ux, uy) - d;
                let (x, y) = ((ux * r) as i64, (uy * r) as i64);
                assert_eq!(real.material(x, y), glam.material(x, y), "{name}: glamour reached {d} m below the surface");
            }
        }
        // And it is as deterministic as the rest.
        let again = with_glamour(name, 0.5);
        assert_eq!(dft_sim::reference::fingerprint(&glam).0, dft_sim::reference::fingerprint(&again).0);
    }
}

#[test]
fn palettes_leave_glamour_zero_alone_and_keep_materials_apart() {
    use dft_sim::palette::to_lch;
    let lab = |c: [u8; 3]| {
        let (l, ch, h) = to_lch(c);
        (l, ch * h.to_radians().cos(), ch * h.to_radians().sin())
    };
    let de = |a: [u8; 3], b: [u8; 3]| {
        let (x, y) = (lab(a), lab(b));
        ((x.0 - y.0).powi(2) + (x.1 - y.1).powi(2) + (x.2 - y.2).powi(2)).sqrt()
    };
    let base = body("living-world");
    let natural: Vec<[u8; 3]> = base.mats.list.iter().map(|m| m.colour).collect();
    assert_eq!(with_glamour("living-world", 0.0).palette.colours, natural);
    // Materials that look different naturally still look different in
    // every palette (lightness is kept, so most pairs stay well apart).
    let (mut pairs, mut merged) = (0, 0);
    for k in 0..12u64 {
        let mut spec = base.spec.clone();
        spec.seed += k * 977;
        spec.glamour = Some(1.0);
        let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
        for i in 1..natural.len() {
            for j in i + 1..natural.len() {
                if de(natural[i], natural[j]) > 0.06 {
                    pairs += 1;
                    if de(p.palette.colours[i], p.palette.colours[j]) < 0.02 {
                        merged += 1;
                    }
                }
            }
        }
    }
    println!("  {merged} of {pairs} distinct pairs drawn too alike");
    assert!((merged as f64) < 0.01 * pairs as f64);
}

#[test]
fn every_body_type_builds_at_every_size_class() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
    for b in ["rubble-asteroid", "comet", "active-planetoid", "living-world", "ocean-world", "sulfur-world", "iron-world"] {
        let text = std::fs::read_to_string(data.join("bodies").join(format!("{b}.toml"))).unwrap();
        // The designed size passes through untouched.
        let r0 = dft_sim::spec::BodySpec::parse(&text).unwrap().radius_m;
        assert_eq!(dft_sim::spec::resize_text(&text, r0).unwrap(), text);
        for km in [0.8, 6.0, 20.0, 60.0, 300.0, 1200.0, 5000.0] {
            let t = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
            let spec = dft_sim::spec::BodySpec::parse(&t).unwrap_or_else(|e| panic!("{b} at {km} km: {e}"));
            let p = dft_sim::planet::Planet::new(spec, mats.clone()).unwrap_or_else(|e| panic!("{b} at {km} km: {e}"));
            assert!((p.radius - km * 1000.0).abs() < 1.0);
            // Solid at the centre, open sky well above.
            let solid = (0..200i64).filter(|k| {
                let (ux, uy) = dft_sim::geology::random_dir(3, *k);
                let r = p.radius * 0.6;
                p.material((ux * r) as i64, (uy * r) as i64) != dft_sim::materials::VACUUM
            }).count();
            assert!(solid > 120, "{b} at {km} km: only {solid} of 200 inside cells solid");
            assert_eq!(p.material(0, (p.max_radius + 10.0) as i64), dft_sim::materials::VACUUM);
        }
    }
}
