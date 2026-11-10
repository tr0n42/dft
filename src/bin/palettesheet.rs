//! palettesheet: one body in many colour schemes, to judge the palettes.
//!
//! cargo run --release --bin palettesheet -- <body> <out.png> [seeds] [glamour,glamour,...]
//! Rows are glamour settings, columns are seeds; each tile is the whole disc
//! over a strip of its surface. Also prints how far apart the seeds' colours
//! are (mean OKLab distance between their average colours).
use dft_sim::palette::to_lch;
use dft_sim::render;
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (body, out) = (&a[0], &a[1]);
    let n: u64 = a.get(2).map_or(6, |s| s.parse().unwrap());
    let gs: Vec<f64> = a.get(3).map_or(vec![0.0, 0.3, 0.6, 1.0], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let base = dft_sim::load_body(&data, body).unwrap();
    let (tw, disc, strip) = (240u32, 240u32, 70u32);
    let th = disc + strip;
    let (w, h) = (tw * n as u32, th * gs.len() as u32);
    let mut img = vec![0u8; (w * h * 3) as usize];
    for (row, &g) in gs.iter().enumerate() {
        let mut means = Vec::new();
        let mut pals: Vec<(Vec<[u8; 3]>, Vec<f64>)> = Vec::new();
        let mut names = Vec::new();
        for k in 0..n {
            let mut spec = base.spec.clone();
            spec.seed += k * 977;
            spec.glamour = Some(g);
            let p = dft_sim::planet::Planet::new(spec, base.mats.clone()).unwrap();
            names.push(if g > 0.0 { format!("{} {:.0}", p.palette.scheme.name(), p.palette.base_hue) } else { "natural".into() });
            let d = render::whole(&p, disc);
            let cells = |x: i64, y: i64| p.material(x, y);
            let below = (0.12 * p.radius).clamp(300.0, 6000.0);
            let s = render::strip(&p, &cells, 0.0, std::f64::consts::TAU, p.max_radius, p.radius - below, tw, strip);
            // The average colour of the disc's body pixels, in OKLab.
            let (mut sl, mut sa, mut sb, mut cnt) = (0.0, 0.0, 0.0, 0.0);
            // (Only the outer part of the disc: crust and mantle, not the core.)
            for (i, px) in d.chunks(3).enumerate() {
                let (x, y) = ((i as u32 % disc) as f64 - disc as f64 / 2.0, (i as u32 / disc) as f64 - disc as f64 / 2.0);
                if (x * x + y * y).sqrt() < 0.6 * 0.47 * disc as f64 {
                    continue;
                }
                let (l, c, hh) = to_lch([px[0], px[1], px[2]]);
                if l > 0.12 {
                    sl += l;
                    sa += c * hh.to_radians().cos();
                    sb += c * hh.to_radians().sin();
                    cnt += 1.0;
                }
            }
            means.push((sl / cnt, sa / cnt, sb / cnt));
            // What the body is made of (by share of samples), and its colours.
            let mut wts = vec![0.0; p.mats.list.len()];
            for q in 0..20_000i64 {
                let (ux, uy) = dft_sim::geology::random_dir(5, q);
                let r = p.radius * dft_sim::noise::unit(dft_sim::noise::hash(6, q, 0, 0)).sqrt();
                wts[p.material((ux * r) as i64, (uy * r) as i64) as usize] += 1.0 / 20_000.0;
            }
            pals.push((p.palette.colours.clone(), wts));
            for y in 0..th as usize {
                let src: &[u8] = if y < disc as usize { &d[y * disc as usize * 3..(y + 1) * disc as usize * 3] } else { &s[(y - disc as usize) * tw as usize * 3..(y - disc as usize + 1) * tw as usize * 3] };
                let dst = ((row * th as usize + y) * w as usize + k as usize * tw as usize) * 3;
                img[dst..dst + tw as usize * 3].copy_from_slice(&src[..tw as usize * 3]);
            }
        }
        let mut sum: f64 = 0.0;
        let mut pairs: f64 = 0.0;
        for i in 0..means.len() {
            for j in i + 1..means.len() {
                let (a, b) = (means[i], means[j]);
                sum += ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt();
                pairs += 1.0;
            }
        }
        // Per material, weighted by how much of it there is: how far each
        // seed's colours are from the natural ones and from each other seed's.
        let lab = |c: [u8; 3]| {
            let (l, ch, h) = to_lch(c);
            (l, ch * h.to_radians().cos(), ch * h.to_radians().sin())
        };
        let de = |a: [u8; 3], b: [u8; 3]| {
            let (x, y) = (lab(a), lab(b));
            ((x.0 - y.0).powi(2) + (x.1 - y.1).powi(2) + (x.2 - y.2).powi(2)).sqrt()
        };
        let natural: Vec<[u8; 3]> = base.mats.list.iter().map(|m| m.colour).collect();
        let from_nat: f64 = pals.iter().map(|(c, w)| (0..c.len()).map(|m| w[m] * de(c[m], natural[m])).sum::<f64>()).sum::<f64>() / pals.len() as f64;
        let (mut between, mut np): (f64, f64) = (0.0, 0.0);
        for i in 0..pals.len() {
            for j in i + 1..pals.len() {
                between += (0..pals[i].0.len()).map(|m| 0.5 * (pals[i].1[m] + pals[j].1[m]) * de(pals[i].0[m], pals[j].0[m])).sum::<f64>();
                np += 1.0;
            }
        }
        println!("  per material: {from_nat:.3} from natural, {:.3} between seeds (OKLab; 0.02 is just noticeable side by side)", between / np.max(1.0));
        let chroma: f64 = means.iter().map(|m| (m.1 * m.1 + m.2 * m.2).sqrt()).sum::<f64>() / means.len() as f64;
        println!("{body} glamour {g}: seeds differ by {:.3} (mean OKLab distance of average colour), average chroma {chroma:.3}; {}", sum / pairs.max(1.0), names.join(", "));
    }
    render::write_png(out, w, h, &img).unwrap();
}
