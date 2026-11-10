//! The full element table (phase 0): every element but the four that only
//! radioactivity makes is somewhere in the bodies, and `possible_materials`
//! never misses what the generator actually places.
use std::path::Path;

#[test]
fn every_natural_element_is_on_some_body_and_possible_covers_what_is_placed() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut names: Vec<String> = std::fs::read_dir(data.join("bodies")).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).map(|p| p.file_stem().unwrap().to_string_lossy().into_owned()).collect();
    names.sort();
    let results: Vec<(String, Vec<bool>, Vec<String>, Vec<String>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = names
            .iter()
            .map(|n| {
                let data = &data;
                sc.spawn(move || {
                    let p = dft_sim::load_body(data, n).unwrap();
                    let can = p.possible_materials();
                    let (_, seen) = dft_sim::reference::fingerprint(&p);
                    let missed: Vec<String> = seen.keys().filter(|name| !can[p.mats.id(name).unwrap() as usize]).cloned().collect();
                    let syms: Vec<String> = p.mats.elements.list.iter().map(|e| e.symbol.clone()).collect();
                    let new_seen: Vec<String> = seen.iter().map(|(k, v)| format!("{k}:{v}")).collect();
                    (n.clone(), p.possible_elements(), missed, if std::env::var("SHOW").is_ok() { new_seen } else { syms })
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let p = dft_sim::load_body(&data, &names[0]).unwrap();
    let els = &p.mats.elements;
    let mut anywhere = vec![false; els.list.len()];
    for (n, has, missed, extra) in &results {
        assert!(missed.is_empty(), "{n}: placed but not judged possible: {missed:?}");
        let absent: Vec<&str> = els.list.iter().zip(has).filter(|(_, h)| !**h).map(|(e, _)| e.symbol.as_str()).collect();
        println!("{n:22} {} of {} elements; absent: {}", has.iter().filter(|h| **h).count(), has.len(), absent.join(" "));
        if std::env::var("SHOW").is_ok() {
            println!("    {}", extra.join(" "));
        }
        for (a, h) in anywhere.iter_mut().zip(has) {
            *a |= *h;
        }
    }
    let nowhere: Vec<&str> = els.list.iter().zip(&anywhere).filter(|(_, a)| !**a).map(|(e, _)| e.symbol.as_str()).collect();
    // (Polonium too, now pitchblende carries only its natural trace of radium:
    // these four come with radioactivity.)
    assert_eq!(nowhere, vec!["Tc", "Po", "At", "Fr"], "elements on no body");
}
