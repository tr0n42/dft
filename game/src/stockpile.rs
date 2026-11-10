//! The stockpile: the elements the mother swarm has been sent, laid out as a
//! periodic table, the materials collected, and the parts made from them,
//! with the round's score (all the matter the swarm holds) in its title.
//! Starts collapsed; opens and closes only when clicked.

use crate::{Body, Run};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use dft_sim::elements::Class;

pub struct StockpilePlugin;

impl Plugin for StockpilePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, stockpile);
    }
}

/// Where each element sits in the table: (period, group), 1-based. The
/// lanthanides are left out (their slot shows as a placeholder); thorium and
/// uranium, the actinides that matter, sit in the f-block row below.
fn place(symbol: &str) -> Option<(u8, u8)> {
    const ROWS: [&[(&str, u8)]; 7] = [
        &[("H", 1), ("He", 18)],
        &[("Li", 1), ("Be", 2), ("B", 13), ("C", 14), ("N", 15), ("O", 16), ("F", 17), ("Ne", 18)],
        &[("Na", 1), ("Mg", 2), ("Al", 13), ("Si", 14), ("P", 15), ("S", 16), ("Cl", 17), ("Ar", 18)],
        &[("K", 1), ("Ca", 2), ("Sc", 3), ("Ti", 4), ("V", 5), ("Cr", 6), ("Mn", 7), ("Fe", 8), ("Co", 9), ("Ni", 10), ("Cu", 11), ("Zn", 12), ("Ga", 13), ("Ge", 14), ("As", 15), ("Se", 16), ("Br", 17), ("Kr", 18)],
        &[("Rb", 1), ("Sr", 2), ("Y", 3), ("Zr", 4), ("Nb", 5), ("Mo", 6), ("Tc", 7), ("Ru", 8), ("Rh", 9), ("Pd", 10), ("Ag", 11), ("Cd", 12), ("In", 13), ("Sn", 14), ("Sb", 15), ("Te", 16), ("I", 17), ("Xe", 18)],
        &[("Cs", 1), ("Ba", 2), ("Hf", 4), ("Ta", 5), ("W", 6), ("Re", 7), ("Os", 8), ("Ir", 9), ("Pt", 10), ("Au", 11), ("Hg", 12), ("Tl", 13), ("Pb", 14), ("Bi", 15), ("Po", 16), ("At", 17), ("Rn", 18)],
        &[("Fr", 1), ("Ra", 2)],
    ];
    match symbol {
        "Th" => return Some((8, 4)),
        "U" => return Some((8, 6)),
        _ => {}
    }
    ROWS.iter().enumerate().find_map(|(r, row)| row.iter().find(|(s, _)| *s == symbol).map(|&(_, g)| (r as u8 + 1, g)))
}

const CELL: egui::Vec2 = egui::vec2(46.0, 44.0);
const GAP: f32 = 3.0;

/// Mass, readably.
fn mass(kg: f64) -> String {
    if kg >= 1e9 {
        format!("{:.2} Mt", kg / 1e9)
    } else if kg >= 1e6 {
        format!("{:.1} kt", kg / 1e6)
    } else if kg >= 1e3 {
        format!("{:.1} t", kg / 1e3)
    } else if kg >= 1.0 {
        format!("{:.0} kg", kg)
    } else {
        String::new()
    }
}

/// Mass in a cell (short).
fn short(kg: f64) -> String {
    if kg >= 1e9 {
        format!("{:.0}Mt", kg / 1e9)
    } else if kg >= 1e6 {
        format!("{:.0}kt", kg / 1e6)
    } else if kg >= 1e3 {
        format!("{:.0}t", kg / 1e3)
    } else if kg >= 1.0 {
        format!("{:.0}kg", kg)
    } else {
        "·".into()
    }
}

/// The class's colour (the usual periodic-table families, muted).
fn family(c: Class) -> egui::Color32 {
    match c {
        Class::Metal => egui::Color32::from_rgb(88, 132, 196),
        Class::Metalloid => egui::Color32::from_rgb(92, 168, 140),
        Class::Nonmetal => egui::Color32::from_rgb(204, 152, 76),
        Class::Noble => egui::Color32::from_rgb(160, 110, 196),
    }
}

/// What this body holds at all (worked out once per body): which elements
/// and which materials could be found anywhere on it.
#[derive(Default)]
struct OnBody {
    name: String,
    elements: Vec<bool>,
    /// The richest share of each element in anything on the body.
    grades: Vec<f64>,
    materials: Vec<bool>,
}

fn stockpile(mut contexts: EguiContexts, run: Option<Res<Run>>, body: Res<Body>, mut finder: ResMut<crate::finder::Finder>, mut on: Local<OnBody>) -> Result {
    let Some(run) = run else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    if on.name != body.0.spec.name || on.elements.is_empty() {
        on.name = body.0.spec.name.clone();
        on.materials = body.0.possible_materials();
        on.grades = body.0.element_grades();
        on.elements = on.grades.iter().map(|g| *g > 0.0).collect();
    }
    let (stock, refined, rate, matter, parts, built, room, collected) = {
        let out = run.0.out.lock();
        let s = &out.stats;
        (s.stock.clone(), s.refined.clone(), s.refine_rate, s.matter, s.parts.clone(), s.built_for_room, s.room, s.collected.clone())
    };
    let els = &body.0.mats.elements;
    let score: f64 = matter.iter().sum();
    // (A fixed id: the title changes with the score, the window must not.
    // Pinned to the screen's top right corner.)
    let shown = egui::Window::new(format!("Swarm matter  {}", mass(score)))
        .id(egui::Id::new("stockpile-window"))
        .default_open(false)
        .collapsible(true)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .show(ctx, |ui| {
            // Where the matter is.
            let names = ["in the stockpile", "on its way up", "made into parts", "built into cutters", "built into haulers", "grown into the mother swarm"];
            egui::Grid::new("stockpile-matter").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
                for (n, kg) in names.iter().zip(matter) {
                    ui.label(*n);
                    ui.label(egui::RichText::new(mass(kg)).monospace());
                    ui.end_row();
                }
            });
            // What this body is known for (its specialties).
            if !body.0.specialties.is_empty() {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("This body:").strong());
                    for r in &body.0.specialties {
                        let c = if r.jackpot { egui::Color32::from_rgb(255, 210, 70) } else { egui::Color32::from_rgb(150, 210, 255) };
                        ui.label(egui::RichText::new(&r.name).color(c)).on_hover_ui(|ui| {
                            ui.label(egui::RichText::new(&r.name).strong());
                            ui.label(egui::RichText::new(&r.riff).italics());
                            for e in &r.effects {
                                ui.label(e);
                            }
                        });
                    }
                });
            }
            ui.add_space(6.0);
            // Two pages: the elements (a periodic table) and the materials
            // (every kind of matter there is, those collected filled in).
            let tab_id = egui::Id::new("stockpile-tab");
            let mut tab = ui.ctx().data_mut(|d| *d.get_persisted_mut_or_default::<u8>(tab_id));
            ui.horizontal(|ui| {
                if ui.selectable_label(tab == 0, "Elements").clicked() {
                    tab = 0;
                }
                if ui.selectable_label(tab == 1, "Materials").clicked() {
                    tab = 1;
                }
                if ui.selectable_label(tab == 2, "Parts").clicked() {
                    tab = 2;
                }
            });
            ui.ctx().data_mut(|d| d.insert_persisted(tab_id, tab));
            ui.add_space(4.0);
            if tab == 1 {
                materials(ui, &body.0, &collected, &on.materials, &mut finder);
                return;
            }
            if tab == 2 {
                parts_page(ui, &parts, built, room);
                return;
            }
            // Raw (still in the minerals it was dug as) or refined (pure,
            // ready to build with): which the table counts.
            let raw_of = |i: usize| (stock.get(i).copied().unwrap_or(0.0) - refined.get(i).copied().unwrap_or(0.0)).max(0.0);
            let ref_of = |i: usize| refined.get(i).copied().unwrap_or(0.0);
            let (raw_total, ref_total): (f64, f64) = ((0..stock.len()).map(raw_of).sum(), (0..stock.len()).map(ref_of).sum());
            let view_id = egui::Id::new("stockpile-view");
            let mut view = ui.ctx().data_mut(|d| *d.get_persisted_mut_or_default::<u8>(view_id));
            ui.horizontal(|ui| {
                if ui.selectable_label(view == 0, format!("Raw  {}", mass(raw_total))).on_hover_text("Elements still bound in the minerals they were dug as").clicked() {
                    view = 0;
                }
                if ui.selectable_label(view == 1, format!("Refined  {}", mass(ref_total))).on_hover_text("Pure elements, ready for production").clicked() {
                    view = 1;
                }
                ui.label(egui::RichText::new(format!("refining {}/s", mass(rate))).weak());
            });
            ui.ctx().data_mut(|d| d.insert_persisted(view_id, view));
            let shown_kg = |i: usize| if view == 0 { raw_of(i) } else { ref_of(i) };
            // The table.
            let most = (0..stock.len()).map(shown_kg).fold(0.0f64, f64::max).max(1.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0 * (CELL.x + GAP), 8.0 * (CELL.y + GAP) + 6.0), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            for (i, e) in els.list.iter().enumerate() {
                let Some((period, group)) = place(&e.symbol) else { continue };
                // (The f-block row sits a little apart, below.)
                let extra = if period >= 8 { 6.0 } else { 0.0 };
                let min = rect.min + egui::vec2((group as f32 - 1.0) * (CELL.x + GAP), (period as f32 - 1.0) * (CELL.y + GAP) + extra);
                let r = egui::Rect::from_min_size(min, CELL);
                let kg = shown_kg(i);
                let (raw, pure) = (raw_of(i), ref_of(i));
                let here = on.elements.get(i).copied().unwrap_or(true);
                let traces = here && on.grades.get(i).is_some_and(|g| *g < dft_sim::planet::TRACE_GRADE);
                let held = raw + pure >= 1.0;
                // Brighter the more there is (on a log scale: a few kilograms
                // of gold still shows).
                let fill = if kg < 1.0 { 0.0 } else { ((kg.log10() + 1.0) / (most.log10() + 1.0)).clamp(0.15, 1.0) as f32 };
                let base = family(e.class);
                let dim = if here || held { egui::Color32::from_rgb(34, 36, 42) } else { egui::Color32::from_rgb(22, 23, 27) };
                let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * fill) as u8;
                let bg = egui::Color32::from_rgb(mix(dim.r(), base.r()), mix(dim.g(), base.g()), mix(dim.b(), base.b()));
                painter.rect_filled(r, 4.0, bg);
                let edge = if kg >= 1.0 { base } else if here { egui::Color32::from_rgb(58, 60, 68) } else { egui::Color32::from_rgb(40, 41, 46) };
                painter.rect_stroke(r, 4.0, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
                if traces {
                    // Only traces in common rock here: a dotted corner.
                    for k in 0..3 {
                        painter.circle_filled(r.right_top() + egui::vec2(-6.0 - 4.0 * k as f32, 6.0), 1.2, egui::Color32::from_rgb(200, 170, 90));
                    }
                }
                let ink = if kg >= 1.0 {
                    egui::Color32::WHITE
                } else if here {
                    egui::Color32::from_rgb(110, 112, 120)
                } else {
                    egui::Color32::from_rgb(64, 66, 72)
                };
                painter.text(r.left_top() + egui::vec2(5.0, 3.0), egui::Align2::LEFT_TOP, &e.symbol, egui::FontId::proportional(16.0), ink);
                if here || held {
                    painter.text(r.left_bottom() + egui::vec2(5.0, -4.0), egui::Align2::LEFT_BOTTOM, short(kg), egui::FontId::monospace(10.0), ink);
                } else {
                    // Not on this body: a slash through the cell.
                    let red = egui::Color32::from_rgba_unmultiplied(200, 80, 70, 150);
                    painter.line_segment([r.left_bottom() + egui::vec2(6.0, -6.0), r.right_top() + egui::vec2(-6.0, 6.0)], egui::Stroke::new(1.5, red));
                }
                // How much of it is refined: a thin bar along the bottom.
                if held {
                    let share = (pure / (raw + pure)).clamp(0.0, 1.0) as f32;
                    let track = egui::Rect::from_min_max(egui::pos2(r.min.x + 4.0, r.max.y - 3.0), egui::pos2(r.max.x - 4.0, r.max.y - 1.5));
                    painter.rect_filled(track, 0.0, egui::Color32::from_rgb(20, 21, 25));
                    painter.rect_filled(egui::Rect::from_min_size(track.min, egui::vec2(track.width() * share, track.height())), 0.0, egui::Color32::from_rgb(120, 230, 170));
                }
                // Hovered: its name, how much, what it is worth, and what
                // the cutters use it for.
                let resp = ui.interact(r, egui::Id::new(("stockpile-el", i)), if here { egui::Sense::click() } else { egui::Sense::hover() });
                let chosen = finder.found.as_ref().is_some_and(|f| f.element == i);
                if resp.hovered() || chosen {
                    let c = if chosen { egui::Color32::from_rgb(255, 235, 64) } else { egui::Color32::WHITE };
                    painter.rect_stroke(r, 4.0, egui::Stroke::new(2.0, c), egui::StrokeKind::Inside);
                }
                if resp.clicked() {
                    finder.want = Some(i);
                }
                let uses: Vec<String> = parts.iter().flat_map(|p| p.from.iter().filter(|(_, el)| el == &e.symbol).map(move |(c, _)| format!("{} ({c})", p.name.to_lowercase()))).collect();
                resp.on_hover_ui(|ui| {
                    ui.label(egui::RichText::new(format!("{} ({})", e.name, e.symbol)).strong());
                    if held {
                        ui.label(format!("{} raw · {} refined", if raw >= 1.0 { mass(raw) } else { "none".into() }, if pure >= 1.0 { mass(pure) } else { "none".into() }));
                    } else {
                        ui.label("none yet");
                    }
                    ui.label(format!("worth {:.1}× ordinary rock per kg", e.value));
                    if !uses.is_empty() {
                        ui.label(format!("made into {} now", uses.join(", ")));
                    }
                    if traces {
                        ui.label(egui::RichText::new("only traces in common rock here: no ore of it on this body").color(egui::Color32::from_rgb(210, 180, 100)));
                    }
                    if here {
                        ui.label(egui::RichText::new("click: go to the nearest ground with it").weak());
                    } else {
                        ui.label(egui::RichText::new("not found on this body").color(egui::Color32::from_rgb(220, 120, 100)));
                    }
                });
            }
            // The lanthanides' place, left out of the game.
            {
                let r = egui::Rect::from_min_size(rect.min + egui::vec2(2.0 * (CELL.x + GAP), 5.0 * (CELL.y + GAP)), CELL);
                painter.rect_stroke(r, 4.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 41, 46)), egui::StrokeKind::Inside);
                painter.text(r.center(), egui::Align2::CENTER_CENTER, "57–71", egui::FontId::proportional(10.0), egui::Color32::from_rgb(70, 72, 78));
            }
            // The legend, in the gap over the transition metals.
            let lg = rect.min + egui::vec2(3.0 * (CELL.x + GAP), 0.0);
            for (k, (c, n)) in [(Class::Metal, "metal"), (Class::Metalloid, "metalloid"), (Class::Nonmetal, "nonmetal"), (Class::Noble, "noble gas")].into_iter().enumerate() {
                let p = lg + egui::vec2((k % 2) as f32 * 120.0, (k / 2) as f32 * 18.0 + 6.0);
                painter.rect_filled(egui::Rect::from_min_size(p, egui::vec2(12.0, 12.0)), 2.0, family(c));
                painter.text(p + egui::vec2(18.0, 6.0), egui::Align2::LEFT_CENTER, n, egui::FontId::proportional(12.0), egui::Color32::from_gray(200));
            }
            {
                let p = lg + egui::vec2(0.0, 2.0 * 18.0 + 6.0);
                let sq = egui::Rect::from_min_size(p, egui::vec2(12.0, 12.0));
                painter.rect_stroke(sq, 2.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 60, 68)), egui::StrokeKind::Inside);
                painter.line_segment([sq.left_bottom(), sq.right_top()], egui::Stroke::new(1.5, egui::Color32::from_rgb(200, 80, 70)));
                painter.text(p + egui::vec2(18.0, 6.0), egui::Align2::LEFT_CENTER, "not on this body", egui::FontId::proportional(12.0), egui::Color32::from_gray(200));
                let p = lg + egui::vec2(0.0, 3.0 * 18.0 + 6.0);
                painter.rect_filled(egui::Rect::from_min_size(p + egui::vec2(0.0, 4.0), egui::vec2(12.0, 3.0)), 0.0, egui::Color32::from_rgb(120, 230, 170));
                painter.text(p + egui::vec2(18.0, 6.0), egui::Align2::LEFT_CENTER, "share refined", egui::FontId::proportional(12.0), egui::Color32::from_gray(200));
                let p = lg + egui::vec2(120.0, 2.0 * 18.0 + 6.0);
                for k in 0..3 {
                    painter.circle_filled(p + egui::vec2(2.0 + 4.0 * k as f32, 6.0), 1.2, egui::Color32::from_rgb(200, 170, 90));
                }
                painter.text(p + egui::vec2(18.0, 6.0), egui::Align2::LEFT_CENTER, "traces only", egui::FontId::proportional(12.0), egui::Color32::from_gray(200));
            }
            if !finder.status.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&finder.status).color(egui::Color32::from_rgb(255, 235, 64)));
                    if finder.found.is_some() && ui.small_button("clear").clicked() {
                        finder.found = None;
                        finder.status.clear();
                    }
                });
            }
        });
    // (Open, it covers part of the screen: finds are framed beside it.)
    finder.covered = shown.filter(|r| r.inner.is_some()).map(|r| r.response.rect);
    Ok(())
}

/// The parts page: every kind of part, how many are on hand and have been
/// made, what a cutter and a hauler take, and what each is made of now.
fn parts_page(ui: &mut egui::Ui, parts: &[crate::sim::PartRow], built: u64, room: u64) {
    ui.set_min_width(18.0 * (CELL.x + GAP));
    if parts.is_empty() {
        ui.label("No parts catalogue: units are built straight from mass.");
        return;
    }
    let held: f64 = parts.iter().map(|p| p.have as f64 * p.kg).sum();
    ui.label(egui::RichText::new(format!("{} of parts on hand", if held >= 1.0 { mass(held) } else { "nothing".into() })).strong());
    ui.label(egui::RichText::new("Made from the refined stockpile and kept ready; every unit is built from them.").weak());
    ui.add_space(6.0);
    let head = |t: &str| egui::RichText::new(t).strong();
    egui::Grid::new("stockpile-parts").num_columns(6).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
        ui.label(head("Part"));
        ui.label(head("on hand"));
        ui.label(head("made"));
        ui.label(head("cutter"));
        ui.label(head("hauler"));
        ui.label(head("made of now"));
        ui.end_row();
        for p in parts {
            ui.label(egui::RichText::new(&p.name)).on_hover_ui(|ui| {
                ui.label(egui::RichText::new(&p.name).strong());
                ui.label(egui::RichText::new(&p.lore).italics());
                ui.label(format!("{} each", mass(p.kg).trim()));
            });
            ui.label(egui::RichText::new(p.have.to_string()).monospace());
            ui.label(egui::RichText::new(p.made.to_string()).monospace().weak());
            let n = |k: u64| egui::RichText::new(if k > 0 { k.to_string() } else { "·".into() }).monospace();
            ui.label(n(p.per_cutter));
            ui.label(n(p.per_hauler));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (k, (c, el)) in p.from.iter().enumerate() {
                    if k > 0 {
                        ui.label(egui::RichText::new("·").weak());
                    }
                    let t = egui::RichText::new(format!("{c} {el}")).monospace();
                    ui.label(if el == "-" { t.color(egui::Color32::from_rgb(230, 110, 90)) } else { t });
                }
            });
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    let unit = |per: fn(&crate::sim::PartRow) -> u64| parts.iter().map(|p| per(p) as f64 * p.kg).sum::<f64>();
    ui.label(egui::RichText::new(format!("a cutter is {} of parts · a hauler {}", mass(unit(|p| p.per_cutter)), mass(unit(|p| p.per_hauler)))).weak());
    ui.label(egui::RichText::new(format!("{built} cutters built for waiting work · room for {room} more")).weak());
}

/// The materials page: every kind of matter there is, as a catalogue to
/// fill. Those the swarm has collected show their colour, how much has come
/// up, what they are made of and how they behave; the rest are blanks to
/// find. Click one collected to go to the nearest of it.
fn materials(ui: &mut egui::Ui, p: &dft_sim::planet::Planet, collected: &[f64], here: &[bool], finder: &mut crate::finder::Finder) {
    let mats = &p.mats;
    let n = mats.len();
    // (Open space isn't a material.)
    let all: Vec<usize> = (0..n).filter(|&m| m as u16 != dft_sim::materials::VACUUM).collect();
    let got = all.iter().filter(|&&m| collected.get(m).copied().unwrap_or(0.0) >= 1.0).count();
    ui.label(egui::RichText::new(format!("{got} of {} kinds of matter collected", all.len())).strong());
    ui.add_space(4.0);
    let w = 18.0 * (CELL.x + GAP);
    egui::ScrollArea::vertical().max_height(420.0).min_scrolled_height(420.0).show(ui, |ui| {
        ui.set_width(w);
        let per_row = 6usize;
        let card = egui::vec2(w / per_row as f32 - 6.0, 58.0);
        for row in all.chunks(per_row) {
            ui.horizontal(|ui| {
                for &m in row {
                    let mat = mats.get(m as u16);
                    let kg = collected.get(m).copied().unwrap_or(0.0);
                    let known = kg >= 1.0;
                    let (r, resp) = ui.allocate_exact_size(card, if known { egui::Sense::click() } else { egui::Sense::hover() });
                    let painter = ui.painter_at(r);
                    let bg = if resp.hovered() { egui::Color32::from_rgb(44, 47, 54) } else { egui::Color32::from_rgb(30, 32, 37) };
                    painter.rect_filled(r, 4.0, bg);
                    // Its swatch: a block of its cells, grained as the ground.
                    let sw = egui::Rect::from_min_size(r.min + egui::vec2(5.0, 5.0), egui::vec2(24.0, 24.0));
                    for j in 0..4 {
                        for i in 0..4 {
                            let g = 0.8 + 0.35 * crate::rnd::unit(m as u64 * 31 + j * 4 + i, 7) as f32;
                            let c = if known { mat.colour.map(|v| (v as f32 * g).min(255.0) as u8) } else { [40, 42, 47].map(|v| (v as f32 * g) as u8) };
                            let cell = egui::Rect::from_min_size(sw.min + egui::vec2(i as f32 * 6.0, j as f32 * 6.0), egui::vec2(6.0, 6.0));
                            painter.rect_filled(cell, 0.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                        }
                    }
                    let ink = if known { egui::Color32::from_gray(225) } else { egui::Color32::from_gray(90) };
                    let name = if known { mat.name.clone() } else { "???".into() };
                    painter.text(r.min + egui::vec2(34.0, 5.0), egui::Align2::LEFT_TOP, &name, egui::FontId::proportional(12.0), ink);
                    if known {
                        painter.text(r.min + egui::vec2(34.0, 21.0), egui::Align2::LEFT_TOP, mass(kg), egui::FontId::monospace(10.0), egui::Color32::from_gray(170));
                        painter.text(r.min + egui::vec2(5.0, 36.0), egui::Align2::LEFT_TOP, format!("{:.2}× · hardness {}", mat.multiplier, mat.hardness), egui::FontId::proportional(10.0), egui::Color32::from_gray(140));
                    } else {
                        let on = here.get(m).copied().unwrap_or(true);
                        painter.text(r.min + egui::vec2(34.0, 21.0), egui::Align2::LEFT_TOP, if on { "not yet found" } else { "not on this body" }, egui::FontId::proportional(10.0), if on { egui::Color32::from_gray(80) } else { egui::Color32::from_rgb(140, 70, 64) });
                    }
                    if known {
                        let resp = resp.on_hover_ui(|ui| {
                            ui.label(egui::RichText::new(&mat.name).strong());
                            ui.label(format!("{} collected · {:?}", mass(kg), mat.state).to_lowercase());
                            let mut parts: Vec<(&String, &f64)> = mat.composition.iter().collect();
                            parts.sort_by(|a, b| b.1.total_cmp(a.1));
                            ui.label(parts.iter().filter(|(_, f)| **f >= 0.001).map(|(e, f)| format!("{e} {:.1}%", *f * 100.0)).collect::<Vec<_>>().join(" · "));
                            let tr = parts.iter().filter(|(_, f)| **f < 0.001).count();
                            if tr > 0 {
                                ui.label(egui::RichText::new(format!("and traces of {tr} more elements")).weak());
                            }
                            ui.label(format!("{:.0} kg a cell · worth {:.2}× rock · hardness {} of 10", mat.density, mat.multiplier, mat.hardness));
                            if let Some(k) = mat.melts_at_k {
                                ui.label(format!("melts at {:.0} K", k));
                            }
                            ui.label(egui::RichText::new("click: go to the nearest of it").weak());
                        });
                        if resp.clicked() {
                            finder.want_mat = Some(m as u16);
                        }
                    }
                }
            });
            ui.add_space(4.0);
        }
    });
    if !finder.status.is_empty() {
        ui.label(egui::RichText::new(&finder.status).color(egui::Color32::from_rgb(255, 235, 64)));
    }
}
