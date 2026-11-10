//! The viewer's pixels must match the sim's own preview renderer exactly.
use dft_viewer::*;

fn put(s: &str) -> (*const u8, usize) {
    let p = alloc(s.len());
    unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), p, s.len()) };
    (p, s.len())
}

#[test]
fn viewer_matches_sim_preview() {
    let d = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/");
    let m = std::fs::read_to_string(format!("{d}materials.toml")).unwrap();
    let m = m + "\n" + &std::fs::read_to_string(format!("{d}materials-catalogue.toml")).unwrap() + "\n" + &std::fs::read_to_string(format!("{d}geology.toml")).unwrap();
    let e = std::fs::read_to_string(format!("{d}elements.toml")).unwrap();
    let (mp, ml) = put(&m);
    let (ep, el) = put(&e);
    assert_eq!(unsafe { set_tables(mp, ml, ep, el) }, 1);
    for body in ["rubble-asteroid", "test-planetoid-3km", "tutorial-1-2km"] {
        let st = std::fs::read_to_string(format!("{d}bodies/{body}.toml")).unwrap();
        let (sp, sl) = put(&st);
        assert_eq!(unsafe { set_body(sp, sl, -1.0) }, 1);
        let p = dft_sim::load_body(std::path::Path::new(d), body).unwrap();
        let size = 300u32;
        let want = dft_sim::render::whole(&p, size);
        let span = 2.0 * (p.max_radius + p.spec.atmosphere_m) * 1.06;
        let t = std::time::Instant::now();
        let n = render(-span / 2.0, span / 2.0, span / size as f64, size, size, 2);
        let ms = t.elapsed().as_millis();
        assert_eq!(n, size * size);
        let got = unsafe { std::slice::from_raw_parts(rgba_ptr(), (n * 4) as usize) };
        let mut diff = 0;
        for i in 0..n as usize {
            if got[i * 4..i * 4 + 3] != want[i * 3..i * 3 + 3] {
                diff += 1;
            }
        }
        println!("{body}: {size}x{size} in {ms} ms, {diff} pixels differ");
        assert_eq!(diff, 0);
        assert_eq!(info(), 1);
    }
}
