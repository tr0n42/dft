//! floragen: grow a body's life and show it.
//!
//! Usage: floragen [--body living-world] [--seed N] [--radius-km R]
//!                 [--glamour G] [--life L] [--data data] [--out out/flora]
//!
//! Writes, into the output folder:
//!   report.txt   which niches can hold life (and why not), and every species
//!   gallery.png  every species, numbered as in the report, three specimens each
//!   scenes.png   close-ups of the body itself: surface, seas, caves, vents

use dft_sim::flora::{Flora, Niche};
use dft_sim::materials::MatId;
use dft_sim::planet::Planet;
use dft_sim::render;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

struct Opts {
    body: String,
    seed: Option<u64>,
    radius_km: Option<f64>,
    glamour: f64,
    life: f64,
    data: PathBuf,
    out: PathBuf,
}

fn parse() -> Result<Opts, String> {
    let mut o = Opts { body: "living-world".into(), seed: None, radius_km: None, glamour: 0.4, life: 1.0, data: PathBuf::from("data"), out: PathBuf::from("out/flora") };
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < a.len() {
        let v = |i: usize| a.get(i + 1).cloned().ok_or(format!("{} needs a value", a[i]));
        match a[i].as_str() {
            "--body" => o.body = v(i)?,
            "--seed" => o.seed = Some(v(i)?.parse().map_err(|_| "--seed takes a whole number")?),
            "--radius-km" => o.radius_km = Some(v(i)?.parse().map_err(|_| "--radius-km takes a number")?),
            "--glamour" => o.glamour = v(i)?.parse().map_err(|_| "--glamour takes a number")?,
            "--life" => o.life = v(i)?.parse().map_err(|_| "--life takes a number")?,
            "--data" => o.data = PathBuf::from(v(i)?),
            "--out" => o.out = PathBuf::from(v(i)?),
            other => return Err(format!("unknown option {other}")),
        }
        i += 2;
    }
    Ok(o)
}

fn main() {
    let o = match parse() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(&o) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run(o: &Opts) -> Result<(), String> {
    let t0 = Instant::now();
    let mats = dft_sim::materials::MaterialTable::load(&o.data.join("materials.toml").to_string_lossy())?;
    let mut text = std::fs::read_to_string(o.data.join("bodies").join(format!("{}.toml", o.body))).map_err(|e| format!("{}: {e}", o.body))?;
    if let Some(km) = o.radius_km {
        text = dft_sim::spec::resize_text(&text, km * 1000.0)?;
    }
    let mut spec = dft_sim::spec::BodySpec::parse(&text)?;
    if let Some(s) = o.seed {
        spec.seed = s;
    }
    spec.glamour = Some(o.glamour);
    spec.life = Some(o.life);
    let p = Planet::new(spec, mats)?;
    std::fs::create_dir_all(&o.out).map_err(|e| e.to_string())?;
    println!("{} · seed {} · {:.1} km · glamour {} · life {}: built in {:.1} s", p.spec.name, p.spec.seed, p.radius / 1000.0, o.glamour, o.life, t0.elapsed().as_secs_f64());

    let Some(f) = p.flora.as_ref() else {
        return Err("no flora was made (is data/life.toml there?)".into());
    };
    let rep = report(&p, f);
    std::fs::write(o.out.join("report.txt"), &rep).map_err(|e| e.to_string())?;
    print!("{rep}");

    let t = Instant::now();
    if !f.species.is_empty() {
        let (w, h, img) = gallery(&p, f);
        render::write_png(&o.out.join("gallery.png").to_string_lossy(), w, h, &img)?;
        println!("gallery.png: {} species in {:.1} s", f.species.len(), t.elapsed().as_secs_f64());
    }
    let t = Instant::now();
    let (w, h, img, n) = scenes(&p, f);
    render::write_png(&o.out.join("scenes.png").to_string_lossy(), w, h, &img)?;
    println!("scenes.png: {n} scenes in {:.1} s", t.elapsed().as_secs_f64());
    println!("written to {}", o.out.display());
    Ok(())
}

fn report(p: &Planet, f: &Flora) -> String {
    let mut s = String::new();
    writeln!(s, "{} (seed {}, radius {:.1} km, glamour {}, life {})", p.spec.name, p.spec.seed, p.radius / 1000.0, f.glamour, f.scale).unwrap();
    writeln!(s).unwrap();
    writeln!(s, "Niches").unwrap();
    for r in &f.report {
        let state = if r.alive {
            format!("alive: {} with {}", match r.energy {
                Some(dft_sim::flora::Energy::Light) => "sunlight",
                Some(dft_sim::flora::Energy::Chemistry) => "chemistry",
                Some(dft_sim::flora::Energy::Seepage) => "what seeps down from above",
                None => "?",
            }, r.solvent.as_deref().unwrap_or("?"))
        } else {
            format!("lifeless: {}", r.why)
        };
        writeln!(s, "  {:<22} {:>5} samples, {:>4.0} to {:>4.0} K   {}", r.niche.name(), r.samples, r.low_k, r.high_k, state).unwrap();
    }
    if f.report.is_empty() {
        writeln!(s, "  (none found)").unwrap();
    }
    writeln!(s).unwrap();
    writeln!(s, "Species ({})", f.species.len()).unwrap();
    for (i, sp) in f.species.iter().enumerate() {
        let kind = if sp.cover { format!("{} (ground cover)", sp.plan.name()) } else { sp.plan.name().to_string() };
        let def = if sp.defence.is_empty() { "-".to_string() } else { sp.defence.iter().map(|d| d.name()).collect::<Vec<_>>().join(", ") };
        writeln!(
            s,
            "  {:>2}  {:<20} {:<20} {:<22} up to {:>4.0} m   {:>3.0} to {:>3.0} K   motion {:<6}  defence: {}",
            i + 1,
            sp.name,
            sp.niche.name(),
            kind,
            sp.height,
            sp.min_k,
            sp.max_k,
            format!("{:?}", sp.motion).to_lowercase(),
            def
        )
        .unwrap();
    }
    s
}

// ---- Pictures ----

const SKY: [u8; 3] = [14, 16, 24];
const GROUND: [u8; 3] = [52, 44, 38];
const INK: [u8; 3] = [230, 230, 220];

/// A 3 x 5 digit font.
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32, c: [u8; 3]) -> Canvas {
        let mut px = vec![0u8; (w * h * 3) as usize];
        for i in 0..(w * h) as usize {
            px[i * 3..i * 3 + 3].copy_from_slice(&c);
        }
        Canvas { w, h, px }
    }
    fn set(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h {
            let i = ((y as u32 * self.w + x as u32) * 3) as usize;
            self.px[i..i + 3].copy_from_slice(&c);
        }
    }
    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 3]) {
        for j in y..y + h {
            for i in x..x + w {
                self.set(i, j, c);
            }
        }
    }
    fn number(&mut self, x: i64, y: i64, n: usize, scale: i64) {
        let text = n.to_string();
        for (k, ch) in text.chars().enumerate() {
            let d = DIGITS[ch.to_digit(10).unwrap() as usize];
            for (row, bits) in d.iter().enumerate() {
                for col in 0..3 {
                    if bits & (4 >> col) != 0 {
                        self.rect(x + (k as i64 * 4 + col) * scale, y + row as i64 * scale, scale, scale, INK);
                    }
                }
            }
        }
    }
    fn blit(&mut self, x0: i64, y0: i64, w: u32, h: u32, img: &[u8]) {
        for j in 0..h as i64 {
            for i in 0..w as i64 {
                let k = ((j as u32 * w + i as u32) * 3) as usize;
                self.set(x0 + i, y0 + j, [img[k], img[k + 1], img[k + 2]]);
            }
        }
    }
}

fn gallery(p: &Planet, f: &Flora) -> (u32, u32, Vec<u8>) {
    let tile = 300i64;
    let cols = 5i64;
    let n = f.species.len() as i64;
    let rows = (n + cols - 1) / cols;
    let mut c = Canvas::new((tile * cols) as u32, (tile * rows) as u32, [8, 9, 12]);
    for (i, sp) in f.species.iter().enumerate() {
        let (tx, ty) = ((i as i64 % cols) * tile, (i as i64 / cols) * tile);
        let roof = sp.niche == Niche::CaveRoof;
        // Three specimens: mature, younger, youngest.
        let specs: Vec<Vec<(i64, i64, MatId)>> = [(1.0, 11u64), (0.75, 22), (0.5, 33)].iter().map(|&(age, s)| f.specimen(i, s * 7919 + i as u64, age)).collect();
        // Fit them side by side.
        let mut widths = Vec::new();
        let (mut top, mut bottom) = (1i64, -1i64);
        for cells in &specs {
            let (mut x0, mut x1) = (0i64, 0i64);
            for &(x, y, _) in cells {
                x0 = x0.min(x);
                x1 = x1.max(x);
                top = top.max(y);
                bottom = bottom.min(y);
            }
            widths.push((x0, x1));
        }
        let total_w: i64 = widths.iter().map(|w| w.1 - w.0 + 3).sum();
        let span_h = top - bottom + 4;
        let scale = ((tile - 30) as f64 / total_w.max(span_h) as f64).min(6.0).max(0.5);
        // Background: sky (or dark) above, ground below.
        let bg = if sp.niche == Niche::Soil || sp.niche == Niche::Bare || sp.niche == Niche::Frozen { SKY } else if sp.niche == Niche::Shallows || sp.niche == Niche::Deeps { [14, 30, 48] } else { [10, 10, 12] };
        c.rect(tx + 2, ty + 2, tile - 4, tile - 4, bg);
        let ground_y = if roof { ty + 22 + ((top + 2) as f64 * scale) as i64 } else { ty + tile - 14 - ((-bottom + 1) as f64 * scale) as i64 };
        if roof {
            c.rect(tx + 2, ty + 2, tile - 4, ground_y - ty - 2, GROUND);
        } else {
            c.rect(tx + 2, ground_y, tile - 4, ty + tile - 2 - ground_y, GROUND);
        }
        let mut cx = tx + 10 + ((tile - 20) - (total_w as f64 * scale) as i64) / 2;
        for (k, cells) in specs.iter().enumerate() {
            let (x0, x1) = widths[k];
            for &(x, y, m) in cells {
                let col = p.palette.colours[m as usize];
                let sx = cx + ((x - x0 + 1) as f64 * scale) as i64;
                let sy = ground_y - ((y + 1) as f64 * scale) as i64;
                let sz = scale.ceil().max(1.0) as i64;
                c.rect(sx, sy, sz, sz, col);
            }
            cx += ((x1 - x0 + 3) as f64 * scale) as i64;
        }
        c.number(tx + 8, ty + 8, i + 1, 3);
    }
    (c.w, c.h, c.px)
}

fn scenes(p: &Planet, f: &Flora) -> (u32, u32, Vec<u8>, usize) {
    // Up to three spots per living niche, spread around the body.
    let mut picks: Vec<(Niche, i64, i64)> = Vec::new();
    for niche in Niche::ALL {
        let all: Vec<&(Niche, i64, i64)> = f.spots.iter().filter(|s| s.0 == niche).collect();
        if all.is_empty() {
            continue;
        }
        let want = if niche == Niche::CaveRoof { 0 } else { 3.min(all.len()) };
        for k in 0..want {
            picks.push(*all[k * all.len() / want]);
        }
    }
    let (sw, sh) = (640u32, 400u32);
    let cols = 3u32;
    let rows = ((picks.len() as u32) + cols - 1) / cols;
    let mut c = Canvas::new(sw * cols + 8 * (cols + 1), (sh + 8) * rows.max(1) + 8, [8, 9, 12]);
    let cells = |x: i64, y: i64| p.material(x, y);
    for (i, &(niche, x, y)) in picks.iter().enumerate() {
        let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
        let r = (fx * fx + fy * fy).sqrt();
        let a = fx.atan2(fy);
        // 320 m wide, 200 m tall at 2 px per metre.
        let half = 160.0 / r;
        let (top, bottom) = if niche == Niche::Soil || niche == Niche::Bare || niche == Niche::Frozen { (r + 130.0, r - 70.0) } else { (r + 100.0, r - 100.0) };
        let img = render::strip(p, &cells, a - half, a + half, top, bottom, sw, sh);
        let (ox, oy) = (8 + (i as u32 % cols) * (sw + 8), 8 + (i as u32 / cols) * (sh + 8));
        c.blit(ox as i64, oy as i64, sw, sh, &img);
        c.number(ox as i64 + 6, oy as i64 + 6, i + 1, 3);
        println!("  scene {:>2}: {}", i + 1, niche.name());
    }
    (c.w, c.h, c.px, picks.len())
}
