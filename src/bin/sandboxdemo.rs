//! sandboxdemo: watch the sandbox at work on a small demo body. Opens a
//! corridor between a magma pocket and a water pocket, and a cavity under the
//! loose surface rubble, then runs the cell physics and writes frames.
//!
//! Usage: sandboxdemo [--body sandbox-demo] [--seconds 90] [--every 5] [--data data] [--out out/sandbox]
//! Writes frame-0000.png ... and prints what happened.

use dft_sim::render;
use dft_sim::sandbox::Sandbox;
use dft_sim::world::World;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut data, mut out, mut body, mut seconds, mut every) = (PathBuf::from("data"), PathBuf::from("out/sandbox"), String::from("sandbox-demo"), 90.0, 5u64);
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--data" => data = PathBuf::from(v),
            "--out" => out = PathBuf::from(v),
            "--body" => body = v,
            "--seconds" => seconds = v.parse().unwrap(),
            "--every" => every = v.parse().unwrap(),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    std::fs::create_dir_all(&out).unwrap();
    let planet = dft_sim::load_body(&data, &body).unwrap();
    let mut w = World::new(planet);
    let mats = w.planet.mats.clone();
    let id = |n: &str| mats.id(n).unwrap();
    let r = w.planet.max_radius as i64 + 2;
    let mut sb = Sandbox::new(&w);
    let scene = dft_sim::sandbox::open_demo(&mut w, &mut sb);
    // A block of rock cut free by a moat (deepest underneath), to watch it fall.
    let mut bodies = dft_sim::bodies::Bodies::new(&w);
    let (bx, by) = (-75, 20);
    for y in by - 14..by + 13 {
        for x in bx - 3..bx + 13 {
            let inside = (bx..bx + 10).contains(&x) && (by..by + 10).contains(&y);
            if !inside && w.material(x, y) != dft_sim::materials::VACUUM {
                w.mine_cell(x, y);
                sb.wake_around((x, y));
                bodies.opened((x, y));
            }
        }
    }
    bodies.break_free(&mut w);
    let (magma, water, opened) = (scene.magma, scene.water, scene.opened);
    println!("{body}: magma pocket near {magma:?}, water pocket near {water:?}; opened {opened} cells");

    let ticks = (seconds * mats.physics.ticks_per_s) as u64;
    let mut frame = 0;
    let t0 = std::time::Instant::now();
    for t in 0..=ticks {
        if t % every == 0 {
            let flying: std::collections::HashMap<(i64, i64), dft_sim::materials::MatId> = bodies.cells().into_iter().collect();
            let cells = |x: i64, y: i64| flying.get(&(x, y)).copied().unwrap_or_else(|| w.material(x, y));
            let img = render::region_with(&w.planet, &cells, -r, r, (2 * r) as u32, (2 * r) as u32, 2);
            render::write_png(&out.join(format!("frame-{frame:04}.png")).to_string_lossy(), (4 * r) as u32, (4 * r) as u32, &img).unwrap();
            frame += 1;
        }
        sb.step(&mut w);
        bodies.step(&mut w);
    }
    bodies.settle_all(&mut w);
    let initial: f64 = w.ledger.initial.iter().sum();
    let imb = w.ledger.imbalance(&w.remaining_kg());
    println!("{:.0} s of sandbox in {:.1} s: {:?}", seconds, t0.elapsed().as_secs_f64(), sb.stats);
    println!("bodies: {:?}", bodies.stats);
    for name in ["magma", "lava-rock", "water", "steam", "ice", "regolith"] {
        let m = id(name) as usize;
        println!("  {name:<10} now {:>10.0} t (changed {:+.0} t, vented {:.0} t)", w.remaining_kg()[m] / 1e3, w.ledger.changed[m] / 1e3, w.ledger.vented[m] / 1e3);
    }
    println!("books balance: {} (largest mismatch {imb:.2e} kg); {frame} frames in {}", if imb < 1e-6 * initial { "yes" } else { "NO" }, out.display());
}
