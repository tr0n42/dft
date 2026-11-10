//! Planet specialties: rolled from the seed, steerable by the meta tree's
//! bias, and off when asked.
use std::path::Path;

fn build(body: &str, seed: u64, bias: dft_sim::specialty::Bias) -> dft_sim::planet::Planet {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
    let mut spec = dft_sim::spec::BodySpec::load(&data.join("bodies").join(format!("{body}.toml")).to_string_lossy()).unwrap();
    spec.seed = seed;
    spec.bias = bias;
    dft_sim::planet::Planet::new(spec, mats).unwrap()
}

#[test]
fn bias_steers_the_roll_and_off_rolls_nothing() {
    let off = dft_sim::specialty::Bias { off: true, ..Default::default() };
    assert!(build("rubble-asteroid", 7, off).specialties.is_empty());
    // Asteroids are pit stops: most seeds roll something.
    let seeds: Vec<u64> = (1..=40).map(|k| k * 7919).collect();
    let plain: Vec<_> = seeds.iter().map(|&s| build("rubble-asteroid", s, Default::default())).collect();
    assert!(plain.iter().filter(|p| !p.specialties.is_empty()).count() >= 34, "asteroids should nearly always have a specialty");
    let tungsten = |ps: &[dft_sim::planet::Planet]| ps.iter().filter(|p| p.specialties.iter().any(|r| r.name == "Tungsten cache")).count();
    let mut bias = dft_sim::specialty::Bias::default();
    bias.element.insert("W".into(), 40.0);
    let steered: Vec<_> = seeds.iter().map(|&s| build("rubble-asteroid", s, bias.clone())).collect();
    let (a, b) = (tungsten(&plain), tungsten(&steered));
    assert!(b >= 20 && b > 3 * a.max(1), "tungsten caches: {a} plain, {b} steered of 40");
}
