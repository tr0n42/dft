//! Procedural chemistry, step C1: the element table, and phase changes that
//! come from each material's own melting and boiling points.

use dft_sim::materials::State;
use dft_sim::sandbox::Sandbox;
use dft_sim::world::World;
use std::path::Path;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

#[test]
fn every_element_in_every_material_is_known() {
    let p = dft_sim::load_body(&data(), "test-planetoid-3km").unwrap();
    let mats = &p.mats;
    assert!(mats.elements.list.len() >= 40);
    for m in &mats.list {
        for el in m.composition.keys() {
            assert!(mats.elements.get(el).is_some(), "{}: {el}", m.name);
        }
    }
    // Spot checks against the reference values.
    let au = mats.elements.get("Au").unwrap();
    let ag = mats.elements.get("Ag").unwrap();
    assert_eq!((au.radius_pm, ag.radius_pm), (144.0, 144.0));
    assert!((au.melt_k - 1337.0).abs() < 1.0 && (ag.melt_k - 1235.0).abs() < 1.0);
}

#[test]
fn every_meltable_solid_gets_a_molten_form_that_freezes_back() {
    let p = dft_sim::load_body(&data(), "test-planetoid-3km").unwrap();
    let mats = &p.mats;
    let mut generated = 0;
    for (i, m) in mats.list.iter().enumerate() {
        let ph = mats.phases[i];
        if matches!(m.state, State::Solid | State::Powder) {
            if let Some(melt) = ph.up[0] {
                let molten = mats.get(melt.to);
                // Melting gives a liquid, except for ices that sublimate straight to gas
                // (dry ice, sulfur dioxide frost).
                assert!(matches!(molten.state, State::Liquid | State::Gas), "{} melts into {}", m.name, molten.name);
                // A cell keeps its mass whatever it turns into.
                assert_eq!(molten.density, m.density);
                if molten.source.is_some() {
                    generated += 1;
                    // ...and the melt remembers what it was.
                    assert_eq!(mats.phases[melt.to as usize].down[0].unwrap().to as usize, i, "{}", molten.name);
                }
            }
        }
        if m.state == State::Liquid {
            if let Some(boil) = ph.up[1] {
                let vapour = mats.get(boil.to);
                assert_eq!(vapour.state, State::Gas, "{}", vapour.name);
                assert_eq!(vapour.density, m.density);
            }
        }
    }
    assert!(generated >= 10, "only {generated} molten forms generated");
    assert!(mats.id("molten basalt").is_ok() && mats.id("molten gold").is_ok() && mats.id("methane vapour").is_ok());
}

/// Hold a cell at `kelvin` (a heat source) and step the sandbox until it has
/// become `want`, or give up after `ticks`.
fn heat_until(w: &mut World, sb: &mut Sandbox, c: (i64, i64), kelvin: f64, want: &str, ticks: u32) -> u32 {
    let target = w.planet.mats.id(want).unwrap();
    for t in 0..ticks {
        if w.material(c.0, c.1) == target {
            return t;
        }
        sb.temps.insert(c, kelvin);
        sb.wake(c);
        sb.step(w);
    }
    panic!("{} at {c:?} never became {want}", w.planet.mats.get(w.material(c.0, c.1)).name);
}

#[test]
fn sulfur_melts_then_boils_by_its_own_numbers() {
    let mut w = World::new(dft_sim::load_body(&data(), "test-planetoid-3km").unwrap());
    let sulfur = w.planet.mats.id("sulfur").unwrap();
    let r = w.planet.max_radius as i64;
    // A sulfur cell buried inside its pocket, so its melt has nowhere to run.
    let buried = |w: &World, x: i64, y: i64| (-1..=1).all(|dy| (-1..=1).all(|dx| w.material(x + dx, y + dy) == sulfur));
    let c = (-r..r).step_by(3).flat_map(|y| (-r..r).step_by(3).map(move |x| (x, y))).find(|&(x, y)| buried(&w, x, y)).expect("sulfur pockets");
    let mut sb = Sandbox::new(&w);
    let initial: f64 = w.ledger.initial.iter().sum();
    // 700 K: deep in the crust the rock around is already ~500 K, and a melting
    // point is held a margin above its surroundings (pressure), so 600 K can
    // fall short there.
    let melted = heat_until(&mut w, &mut sb, c, 700.0, "molten sulfur", 50);
    let boiled = heat_until(&mut w, &mut sb, c, 900.0, "sulfur vapour", 50);
    // Latent heat holds the change back for a few ticks; boiling takes more
    // heat than melting.
    assert!(melted >= 1 && boiled > 1, "melted after {melted} ticks, boiled after {boiled}");
    assert!(w.ledger.imbalance(&w.remaining_kg()) < 1e-6 * initial);
}

#[test]
fn liquid_methane_boils_with_no_rule_written_for_it() {
    let p = dft_sim::load_body(&data(), "sandbox-demo").unwrap();
    let mats = &p.mats;
    let methane = mats.id("liquid methane").unwrap();
    let ph = mats.phases[methane as usize];
    // Only its boiling point is written; the vapour is generated.
    let boil = ph.up[1].expect("liquid methane boils");
    assert_eq!(mats.get(boil.to).name, "methane vapour");
    assert!(mats.get(boil.to).source.is_some());
    assert_eq!(boil.at_k, 112.0);
    assert_eq!(mats.get(ph.down[0].expect("and freezes").to).name, "methane ice");
    // The vapour condenses back into liquid methane.
    assert_eq!(mats.phases[boil.to as usize].down[1].unwrap().to, methane);
}
