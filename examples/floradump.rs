use dft_sim::planet::Planet;
fn main() {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<i64> = all[..3].iter().map(|s| s.parse().unwrap()).collect();
    let body = all.get(3).cloned().unwrap_or("living-world".into());
    let mats = dft_sim::materials::MaterialTable::load("data/materials.toml").unwrap();
    let mut spec = dft_sim::spec::BodySpec::load(&format!("data/bodies/{body}.toml")).unwrap();
    spec.life = Some(1.0);
    spec.glamour = Some(a[2] as f64 / 10.0);
    let p = Planet::new(spec, mats).unwrap();
    let (cx, cy) = (a[0], a[1]);
    for y in (cy - 12..=cy + 12).rev() {
        let mut line = String::new();
        for x in cx - 20..=cx + 20 {
            let m = p.material(x, y);
            let b = p.base_material(x, y);
            let n = &p.mats.get(m).name;
            let ch = if m != b { if n.ends_with("stem") { 'S' } else if n.ends_with("foliage") { 'f' } else if n.ends_with("organ") { 'o' } else { 'r' } } else if m == 0 { '.' } else if p.mats.get(m).state == dft_sim::materials::State::Liquid { '~' } else { '#' };
            line.push(if (x, y) == (cx, cy) { '@' } else { ch });
        }
        println!("{line}");
    }
}
