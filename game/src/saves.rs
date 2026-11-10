//! Saves: the whole game in one file, written in the background.
//!
//! A save holds the body's source (so it can be rebuilt exactly), the
//! simulation (world, swarm, sandbox: see `Sim::write_save` for what is kept
//! only roughly), the charted ground, the view and the settings. The
//! simulation is copied out while it waits (a short hold); compressing and
//! writing happen on another thread, so the game doesn't hitch.
//!
//! File: "DFTG" | version u32 | header length u32 | header (key=value lines,
//! readable without unpacking) | `save::pack`ed body of the save.
//!
//! The game autosaves every AUTOSAVE_S seconds and on quitting, keeping the
//! last AUTOSAVES (autosave-1 the newest). F5 saves to quicksave; F9 loads the
//! newest save. Benchmarks and screenshots never autosave.
use crate::sim::{self, Control};
use crate::{Body, Run, Source, Start};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use dft_sim::save::{R, W};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 4] = b"DFTG";
const VERSION: u32 = 1;
const PACK_MAGIC: &[u8; 4] = b"DFTZ";
/// Real seconds between autosaves.
const AUTOSAVE_S: f64 = 300.0;
/// Autosaves kept.
const AUTOSAVES: usize = 3;

pub struct SavesPlugin;

impl Plugin for SavesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(PostStartup, restore_at_start)
            .add_systems(Update, (keys.run_if(not(bevy_egui::input::egui_wants_any_keyboard_input)), autosave, start_save, finish_save, finish_switch).chain())
            .add_systems(Last, save_on_quit);
    }
}

/// Where things were left, beyond the simulation.
#[derive(Clone, Debug)]
pub struct Restore {
    pub view: (f64, f64, f64),
    pub speed: usize,
    pub charted: Vec<(i64, i64)>,
}

/// A save to restore once the app is up (loaded at the start).
#[derive(Resource)]
pub struct Restoring(pub Option<Restore>);

/// A saved game on disk, for the panel's list.
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    /// When it was saved (unix seconds) and what's in it, from its header.
    pub saved_at: u64,
    pub summary: String,
    pub bytes: u64,
}

/// What to save to.
#[derive(Clone, Debug, PartialEq)]
pub enum Slot {
    /// The autosave rotation.
    Auto,
    /// A file by name (quicksave, bench, ...).
    Named(String),
}

#[derive(Resource)]
pub struct Saves {
    pub autosave: bool,
    /// A save asked for, not yet begun.
    pub wanted: Option<Slot>,
    writing: Option<Task<Result<Written, String>>>,
    /// A load or new body under way (the panel's build uses this too).
    switching: Option<(Task<Result<Start, String>>, Instant, String)>,
    last_auto: f64,
    /// The game time at the last save (no autosave if nothing has run since).
    last_game_t: f64,
    /// For the panel: what happened last, and the list of saves.
    pub status: String,
    pub error: Option<String>,
    pub list: Vec<Entry>,
}

impl Saves {
    pub fn busy(&self) -> bool {
        self.switching.is_some()
    }

    pub fn writing(&self) -> bool {
        self.writing.is_some()
    }

    /// Load a save in the background, then switch to it.
    pub fn load(&mut self, path: PathBuf) {
        if self.switching.is_some() {
            return;
        }
        let label = format!("load {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        self.switching = Some((AsyncComputeTaskPool::get().spawn(async move { read_file(&path) }), Instant::now(), label));
    }

    /// Build a new body in the background, then switch to it.
    pub fn build(&mut self, r: crate::Recipe) {
        if self.switching.is_some() {
            return;
        }
        let label = format!("new body {}", r.body);
        self.switching = Some((AsyncComputeTaskPool::get().spawn(async move { crate::make(&r) }), Instant::now(), label));
    }

    pub fn refresh(&mut self) {
        self.list = list();
    }
}

struct Written {
    path: PathBuf,
    raw: usize,
    bytes: usize,
    pack_s: f64,
}

/// Where saves go: `saves` beside the data folder.
pub fn save_dir() -> PathBuf {
    crate::data_dir().join("..").join("saves")
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Every save on disk, newest first.
pub fn list() -> Vec<Entry> {
    let mut v: Vec<Entry> = std::fs::read_dir(save_dir())
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "dft"))
                .filter_map(|p| {
                    let bytes = std::fs::metadata(&p).ok()?.len();
                    let head = read_header(&p).ok()?;
                    let get = |k: &str| head.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone()).unwrap_or_default();
                    let t: f64 = get("time_s").parse().unwrap_or(0.0);
                    let summary = format!("{} {} km · {} · {} drones · {}% eaten", get("body"), get("radius_km"), clock(t), get("drones"), get("eaten_pct"));
                    Some(Entry { name: p.file_stem()?.to_string_lossy().into_owned(), saved_at: get("saved_at").parse().unwrap_or(0), summary, bytes, path: p })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.saved_at.cmp(&a.saved_at).then(a.name.cmp(&b.name)));
    v
}

fn clock(t: f64) -> String {
    let s = t.max(0.0) as u64;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Find a save: "last" for the newest, a path, or a name in the saves folder.
pub fn find(which: &str) -> Result<PathBuf, String> {
    if which == "last" {
        return list().first().map(|e| e.path.clone()).ok_or_else(|| format!("no saves in {}", save_dir().display()));
    }
    let p = PathBuf::from(which);
    if p.exists() {
        return Ok(p);
    }
    let named = save_dir().join(if which.ends_with(".dft") { which.to_string() } else { format!("{which}.dft") });
    if named.exists() { Ok(named) } else { Err(format!("no save called {which}")) }
}

fn read_header(path: &Path) -> Result<Vec<(String, String)>, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).map_err(|e| e.to_string())?;
    if &head[0..4] != MAGIC {
        return Err("not a Dark Forest Theory save".into());
    }
    let n = u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize;
    if n > 1 << 20 {
        return Err("corrupt save header".into());
    }
    let mut text = vec![0u8; n];
    f.read_exact(&mut text).map_err(|e| e.to_string())?;
    Ok(parse_header(&String::from_utf8_lossy(&text)))
}

fn parse_header(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(|l| l.split_once('=')).map(|(a, b)| (a.trim().to_string(), b.trim().to_string())).collect()
}

/// A save, uncompressed: the header text and the body to pack. Holds the
/// simulation only while copying it out.
fn snapshot(run: &Run, body: &Body, fog: &crate::fog::Fog, view: &crate::view::View) -> (String, Vec<u8>, f64) {
    let src: &Source = &body.1;
    let control = *run.0.control.lock();
    let mut o = W::default();
    o.text(&src.name);
    o.text(&src.text);
    o.u64(src.seed);
    o.f64(src.glamour);
    o.f64(view.x);
    o.f64(view.y);
    o.f64(view.mpp);
    o.u32(control.speed as u32);
    // (Once whether the economy was off, and the free swarm's size: the
    // economy is always on now. Kept so older saves still read.)
    o.u8(0);
    o.u64(0);
    let charted = fog.charted_cells();
    o.u64(charted.len() as u64);
    let mut prev = (0i64, 0i64);
    for c in charted {
        o.i64(c.0 - prev.0);
        o.i64(c.1 - prev.1);
        prev = c;
    }
    let t0 = Instant::now();
    let (time, drones, eaten) = {
        let s = run.0.world();
        s.write_save(&mut o);
        (s.time(), s.drone_count() as u64 + s.packet, s.consumed_share())
    };
    let hold = t0.elapsed().as_secs_f64();
    let header = format!(
        "body={}\nradius_km={:.2}\nseed={}\nglamour={}\ntime_s={:.1}\ndrones={}\neaten_pct={:.3}\nsaved_at={}\n",
        src.name,
        body.0.radius / 1000.0,
        src.seed,
        src.glamour,
        time,
        drones,
        eaten * 100.0,
        now_unix()
    );
    (header, o.0, hold)
}

/// Compress and write a save (atomically: a temporary file, then renamed).
fn write_file(path: &Path, header: &str, raw: &[u8]) -> Result<usize, String> {
    let packed = dft_sim::save::pack(PACK_MAGIC, VERSION, raw);
    let mut out = Vec::with_capacity(packed.len() + header.len() + 12);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(header.len() as u32).to_le_bytes());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(&packed);
    std::fs::create_dir_all(save_dir()).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &out).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(out.len())
}

/// The file for a slot. The autosaves shift down one first (the oldest goes).
fn slot_path(slot: &Slot) -> PathBuf {
    let dir = save_dir();
    match slot {
        Slot::Named(n) => dir.join(format!("{n}.dft")),
        Slot::Auto => {
            let _ = std::fs::create_dir_all(&dir);
            for k in (1..AUTOSAVES).rev() {
                let from = dir.join(format!("autosave-{k}.dft"));
                if from.exists() {
                    let _ = std::fs::rename(&from, dir.join(format!("autosave-{}.dft", k + 1)));
                }
            }
            dir.join("autosave-1.dft")
        }
    }
}

/// Read a save and rebuild the game it holds.
pub fn read_file(path: &Path) -> Result<Start, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 12 || &bytes[0..4] != MAGIC {
        return Err("not a Dark Forest Theory save".into());
    }
    let v = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if v != VERSION {
        return Err(format!("save version {v} is not supported"));
    }
    let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let rest = bytes.get(12 + n..).ok_or("save is truncated")?;
    let raw = dft_sim::save::unpack(PACK_MAGIC, VERSION, rest)?;
    let mut r = R(&raw, 0);
    let name = r.text()?;
    let text = r.text()?;
    let seed = r.u64()?;
    let glamour = r.f64()?;
    let source = Source { name, text, seed, glamour };
    let view = (r.f64()?, r.f64()?, r.f64()?);
    let speed = (r.u32()? as usize).min(sim::SPEEDS.len() - 1);
    let free = r.u8()? != 0;
    let n_free = r.u64()?;
    let mut charted = Vec::new();
    let mut prev = (0i64, 0i64);
    for _ in 0..r.u64()? {
        let c = (prev.0 + r.i64()?, prev.1 + r.i64()?);
        prev = c;
        charted.push(c);
    }
    let (planet, eco) = crate::build_body(&source)?;
    let mut sim = dft_sim::swarm::Sim::read_save(&mut r, planet.clone(), eco)?;
    // A save from when the economy could be off: it goes on with the real
    // economy, its free energy dropped.
    let _ = n_free;
    if free {
        sim.energy = sim.eco.seed_swarm.start_energy;
    }
    Ok(Start { planet, sim, source, restore: Some(Restore { view, speed, charted }) })
}

fn setup(mut commands: Commands, o: Res<crate::Options>) {
    let mut s = Saves { autosave: o.autosave, wanted: None, writing: None, switching: None, last_auto: 0.0, last_game_t: -1.0, status: String::new(), error: None, list: Vec::new() };
    s.refresh();
    commands.insert_resource(s);
}

/// A game loaded at the start: put the view and the charts back.
fn restore_at_start(r: Option<ResMut<Restoring>>, mut view: ResMut<crate::view::View>, mut fog: ResMut<crate::fog::Fog>, body: Res<Body>, mut cam: ResMut<crate::camera::Camera>, time: Res<Time<Real>>, mut tiles: ResMut<crate::tiles::Tiles>) {
    let Some(mut r) = r else { return };
    let Some(r) = r.0.take() else { return };
    // The saved game's ground has changed in places the screen never saw change.
    tiles.all_touched = true;
    apply(r, &mut view, &mut fog, &body.0, &mut cam, time.elapsed_secs_f64());
}

fn apply(r: Restore, view: &mut crate::view::View, fog: &mut crate::fog::Fog, planet: &Arc<dft_sim::planet::Planet>, cam: &mut crate::camera::Camera, now: f64) {
    (view.x, view.y, view.mpp) = r.view;
    fog.restore = Some((Arc::as_ptr(planet) as usize, r.charted));
    // Hold the view where it was left for a moment before the director moves it.
    if cam.director {
        cam.manual(now);
    }
}

/// F5 saves to the quicksave; F9 loads the newest save.
fn keys(keys: Res<ButtonInput<KeyCode>>, mut saves: ResMut<Saves>) {
    if keys.just_pressed(KeyCode::F5) {
        saves.wanted = Some(Slot::Named("quicksave".into()));
    }
    if keys.just_pressed(KeyCode::F9) {
        saves.refresh();
        if let Some(e) = saves.list.first() {
            let p = e.path.clone();
            saves.load(p);
        }
    }
}

fn autosave(mut saves: ResMut<Saves>, time: Res<Time<Real>>, run: Res<Run>) {
    let now = time.elapsed_secs_f64();
    if !saves.autosave || now - saves.last_auto < AUTOSAVE_S {
        return;
    }
    saves.last_auto = now;
    let t = run.0.out.lock().stats.time;
    if t > saves.last_game_t + 1.0 && saves.wanted.is_none() {
        saves.wanted = Some(Slot::Auto);
    }
}

/// Copy the game out, then hand compressing and writing to another thread.
fn start_save(mut saves: ResMut<Saves>, run: Res<Run>, body: Res<Body>, fog: Res<crate::fog::Fog>, view: Res<crate::view::View>, mut perf: ResMut<crate::log::Perf>) {
    if saves.writing.is_some() || saves.switching.is_some() {
        return;
    }
    let Some(slot) = saves.wanted.take() else { return };
    let (header, raw, hold) = snapshot(&run, &body, &fog, &view);
    let path = slot_path(&slot);
    saves.last_game_t = parse_header(&header).iter().find(|(k, _)| k == "time_s").and_then(|(_, v)| v.parse().ok()).unwrap_or(0.0);
    perf.events.push(("save begun".into(), format!(",\"slot\":{},\"hold_s\":{},\"raw_mb\":{}", crate::log::json_text(&format!("{slot:?}")), crate::log::json_num(hold), crate::log::json_num(raw.len() as f64 / 1e6))));
    saves.status = format!("saving ({:.0} ms copying)...", hold * 1000.0);
    saves.writing = Some(AsyncComputeTaskPool::get().spawn(async move {
        let t = Instant::now();
        let bytes = write_file(&path, &header, &raw)?;
        Ok(Written { path, raw: raw.len(), bytes, pack_s: t.elapsed().as_secs_f64() })
    }));
}

fn finish_save(mut saves: ResMut<Saves>, mut perf: ResMut<crate::log::Perf>) {
    let Some(task) = &mut saves.writing else { return };
    let Some(result) = block_on(poll_once(task)) else { return };
    saves.writing = None;
    match result {
        Ok(w) => {
            let name = w.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            perf.events.push(("saved".into(), format!(",\"file\":{},\"raw_mb\":{},\"file_mb\":{},\"write_s\":{}", crate::log::json_text(&name), crate::log::json_num(w.raw as f64 / 1e6), crate::log::json_num(w.bytes as f64 / 1e6), crate::log::json_num(w.pack_s))));
            saves.status = format!("saved {name} ({:.1} MB, {:.1} s)", w.bytes as f64 / 1e6, w.pack_s);
            saves.error = None;
        }
        Err(e) => {
            perf.events.push(("save failed".into(), format!(",\"error\":{}", crate::log::json_text(&e))));
            saves.error = Some(format!("save failed: {e}"));
        }
    }
    saves.refresh();
}

/// When a loaded game or new body is ready: stop the old simulation, forget
/// its tiles and drones, and start the new one.
#[allow(clippy::too_many_arguments)]
fn finish_switch(
    mut commands: Commands,
    mut saves: ResMut<Saves>,
    mut tiles: ResMut<crate::tiles::Tiles>,
    mut view: ResMut<crate::view::View>,
    run: Res<Run>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut perf: ResMut<crate::log::Perf>,
    mut fleet: ResMut<crate::drones::Fleet>,
    mut fog: ResMut<crate::fog::Fog>,
    mut cam: ResMut<crate::camera::Camera>,
    time: Res<Time<Real>>,
) {
    let Some((task, _, _)) = &mut saves.switching else { return };
    let Some(result) = block_on(poll_once(task)) else { return };
    let (_, began, label) = saves.switching.take().unwrap();
    let took = began.elapsed().as_secs_f64();
    use crate::log::{json_num, json_text};
    match result {
        Ok(start) => {
            let Start { planet, sim, source, restore } = start;
            perf.events.push((
                if restore.is_some() { "loaded".into() } else { "new body".into() },
                format!(",\"action\":{},\"body\":{},\"radius_km\":{},\"seed\":{},\"glamour\":{},\"build_s\":{},\"game_s\":{},\"drones\":{}", json_text(&label), json_text(&source.name), json_num(planet.radius / 1000.0), source.seed, json_num(source.glamour), json_num(took), json_num(sim.time()), sim.drone_count()),
            ));
            let old = *run.0.control.lock();
            sim::stop(&run.0);
            fleet.clear();
            tiles.clear(&mut commands);
            view.fit(&planet);
            if let Ok(mut w) = windows.single_mut() {
                w.title = crate::title(&planet);
            }
            let control = Control { speed: old.speed, paused: false, pause_at: None, free_drones: old.free_drones, attention: old.attention, full_detail: old.full_detail, quit: false };
            if let Some(r) = restore {
                tiles.all_touched = true;
                apply(r, &mut view, &mut fog, &planet, &mut cam, time.elapsed_secs_f64());
            }
            saves.last_game_t = sim.time();
            saves.status = format!("{label}: ready in {took:.1} s");
            saves.error = None;
            commands.insert_resource(Run(sim::start(sim, control)));
            commands.insert_resource(Body(planet, Arc::new(source)));
        }
        Err(e) => {
            perf.events.push(("switch failed".into(), format!(",\"action\":{},\"error\":{}", json_text(&label), json_text(&e))));
            saves.error = Some(format!("{label}: {e}"));
        }
    }
}

/// On quitting: wait for a save under way, then autosave (in the
/// foreground: the game is closing anyway).
fn save_on_quit(mut exit: MessageReader<AppExit>, mut saves: ResMut<Saves>, run: Res<Run>, body: Res<Body>, fog: Res<crate::fog::Fog>, view: Res<crate::view::View>) {
    if exit.read().next().is_none() || !saves.autosave {
        return;
    }
    if let Some(task) = saves.writing.take() {
        let _ = block_on(task);
    }
    if run.0.out.lock().stats.time <= saves.last_game_t + 1.0 {
        return;
    }
    saves.autosave = false;
    let (header, raw, _) = snapshot(&run, &body, &fog, &view);
    if let Err(e) = write_file(&slot_path(&Slot::Auto), &header, &raw) {
        eprintln!("autosave on quit failed: {e}");
    }
}
