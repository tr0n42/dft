//! The realistic models, locked in: every body's generator fingerprint must
//! match data/reference/fingerprints.toml. A deliberate change to a body is
//! re-blessed with BLESS=1 (and the reference sheets rebuilt with refsheet).
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn every_body_matches_its_locked_reference() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let file = data.join("reference").join("fingerprints.toml");
    let mut names: Vec<String> = std::fs::read_dir(data.join("bodies")).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).map(|p| p.file_stem().unwrap().to_string_lossy().into_owned()).collect();
    names.sort();
    let got: BTreeMap<String, String> = std::thread::scope(|sc| {
        let hs: Vec<_> = names
            .iter()
            .map(|n| {
                let data = &data;
                sc.spawn(move || {
                    let p = dft_sim::load_body(data, n).unwrap();
                    (n.clone(), format!("{:016x}", dft_sim::reference::fingerprint(&p).0))
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    if std::env::var("BLESS").is_ok() {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let mut s = String::from("# Locked generator fingerprints of the realistic models (glamour 0).\n# Rewrite with: BLESS=1 cargo test --release --test reference\n");
        for (n, h) in &got {
            s += &format!("\"{n}\" = \"{h}\"\n");
        }
        std::fs::write(&file, s).unwrap();
        return;
    }
    let want: BTreeMap<String, String> = toml::from_str(&std::fs::read_to_string(&file).expect("no reference file: run with BLESS=1")).unwrap();
    let mut bad = Vec::new();
    for (n, h) in &got {
        match want.get(n) {
            Some(w) if w == h => {}
            Some(w) => bad.push(format!("{n}: {h} (locked {w})")),
            None => bad.push(format!("{n}: {h} (not locked yet)")),
        }
    }
    assert!(bad.is_empty(), "bodies differ from their locked reference:\n  {}", bad.join("\n  "));
}
