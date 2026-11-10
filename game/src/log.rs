//! The performance log: one file per session in the project's `logs` folder,
//! one JSON object per line, written as the game runs (flushed every line, so
//! a crash or a forced quit still leaves everything up to that second).
//!
//! Lines, by "kind":
//!   start    the machine (OS, CPU, cores, memory, graphics card), the body,
//!            the options and what each list of numbers below is named
//!   sample   once a second of real time: frame times (average, median, 95th
//!            percentile, worst), how long the game's own systems took per
//!            frame, the main update as a whole, what was drawn, tile
//!            building, and the simulation: speed asked and achieved, compute
//!            per game second split by part, how often things happened, its
//!            thread's waits, the swarm's numbers, process memory and CPU
//!   event    something changed: speed, pause, economy, a new body, the swarm
//!            reaching its size, a slow frame (with what took the time)
//!   end      the session's totals
//!   panic    a crash, with its message
//!
//! tools/logsum.py turns a log into a summary.
use crate::sim::{self, SPEEDS, ThreadPerf};
use bevy::diagnostic::{DiagnosticsStore, SystemInfo, SystemInformationDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The game's own systems, timed every frame.
pub const PARTS: [&str; 7] = ["tile changes", "tile requests", "tile results", "tile placing", "drones", "dev panel", "fog"];
pub const P_CHANGES: usize = 0;
pub const P_REQUEST: usize = 1;
pub const P_COLLECT: usize = 2;
pub const P_PLACE: usize = 3;
pub const P_DRONES: usize = 4;
pub const P_PANEL: usize = 5;
pub const P_FOG: usize = 6;

/// A frame slower than this is logged as an event.
const SLOW_FRAME_S: f64 = 0.05;
/// Keep this many logs; older ones are removed.
const KEEP_LOGS: usize = 40;

/// Timings and counts the systems add to as they run.
#[derive(Resource, Default)]
pub struct Perf {
    /// This frame's seconds per part.
    pub frame: [f64; PARTS.len()],
    /// Drone squares and hub markers drawn this frame.
    pub quads: usize,
    /// Tiles visible this frame.
    pub tiles_shown: usize,
    /// Places on screen this frame with no picture at all (nothing arrived
    /// for them or any coarser tile): blank holes.
    pub tile_holes: usize,
    /// Tile results taken this frame (first builds and rebuilds).
    pub built: usize,
    pub rebuilt: usize,
    /// Events from elsewhere (the dev panel): what, and more JSON fields
    /// (each starting with a comma).
    pub events: Vec<(String, String)>,
}

impl Perf {
    pub fn add(&mut self, part: usize, since: Instant) {
        self.frame[part] += since.elapsed().as_secs_f64();
    }
}

/// The open log and what it is summing up for the next sample.
#[derive(Resource)]
struct Log {
    file: Option<std::io::BufWriter<std::fs::File>>,
    started: Instant,
    last: Instant,
    frames: Vec<f64>,
    main: Vec<f64>,
    parts: [f64; PARTS.len()],
    parts_max: [f64; PARTS.len()],
    quads: usize,
    tiles_shown: usize,
    /// Frames with any hole, and the most holes in one frame.
    hole_frames: usize,
    holes_max: usize,
    built: usize,
    rebuilt: usize,
    /// The simulation's totals at the last sample.
    prof0: [f64; 8],
    counts0: [u64; 6],
    time0: f64,
    tile0: [u64; 5],
    control: Option<sim::Control>,
    target_reached: bool,
    slow_this_second: u32,
    frames_total: u64,
    slow_total: u64,
    peak_drones: usize,
    peak_mem_gib: f64,
    /// When the main update began this frame.
    update_start: Option<Instant>,
}

static PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub struct LogPlugin;

impl Plugin for LogPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SystemInformationDiagnosticsPlugin)
            .init_resource::<Perf>()
            .add_systems(Startup, open)
            .add_systems(First, mark_update_start)
            .add_systems(Last, (sample, write_end_on_exit));
    }
}

/// A JSON number (null if it isn't finite).
fn num(v: f64) -> String {
    if v.is_finite() { format!("{}", (v * 1e4).round() / 1e4) } else { "null".into() }
}

fn text(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn list_f(v: &[f64]) -> String {
    format!("[{}]", v.iter().map(|x| num(*x)).collect::<Vec<_>>().join(","))
}

fn list_s(v: &[&str]) -> String {
    format!("[{}]", v.iter().map(|x| text(x)).collect::<Vec<_>>().join(","))
}

/// Seconds since 1970 as a UTC date and time, for file names and the header.
fn utc(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d, (rem / 3600) as u32, (rem / 60 % 60) as u32, (rem % 60) as u32)
}

/// Where logs go: `logs` beside the data folder.
pub fn log_dir() -> PathBuf {
    crate::data_dir().join("..").join("logs")
}

/// The current log's path, if one is open (for the dev panel).
pub fn current() -> Option<PathBuf> {
    PATH.lock().ok().and_then(|p| p.clone())
}

fn append_line(line: &str) {
    if let Some(p) = current() {
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(p) {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn open(mut commands: Commands, o: Res<crate::Options>, body: Res<crate::Body>, run: Res<crate::Run>, sysinfo: Option<Res<SystemInfo>>, adapter: Option<Res<RenderAdapterInfo>>) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, mo, d, h, mi, s) = utc(now);
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    // Forget the oldest logs beyond the budget.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut logs: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jsonl")).collect();
        logs.sort();
        while logs.len() >= KEEP_LOGS {
            let _ = std::fs::remove_file(logs.remove(0));
        }
    }
    let path = dir.join(format!("game-{y:04}-{mo:02}-{d:02}_{h:02}-{mi:02}-{s:02}.jsonl"));
    let file = std::fs::File::create(&path).ok().map(std::io::BufWriter::new);
    if file.is_some() {
        *PATH.lock().unwrap() = Some(path.clone());
        // A crash still leaves a line saying what happened.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
            let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
            let thread = std::thread::current().name().unwrap_or("?").to_string();
            append_line(&format!("{{\"kind\":\"panic\",\"thread\":{},\"at\":{},\"message\":{}}}", text(&thread), text(&at), text(&msg)));
            previous(info);
        }));
    }
    let p = &body.0;
    let c = *run.0.control.lock();
    let mut l = String::new();
    let _ = write!(l, "{{\"kind\":\"start\",\"utc\":\"{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}\",\"game\":{}", text(env!("CARGO_PKG_VERSION")));
    if let Some(si) = sysinfo {
        let _ = write!(l, ",\"os\":{},\"cpu\":{},\"cores\":{},\"memory\":{}", text(&si.os), text(&si.cpu), text(&si.core_count), text(&si.memory));
    }
    let _ = write!(l, ",\"threads\":{}", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0));
    if let Some(a) = adapter {
        let _ = write!(l, ",\"gpu\":{},\"gpu_backend\":{},\"gpu_driver\":{}", text(&a.name), text(&format!("{:?}", a.backend)), text(&format!("{} {}", a.driver, a.driver_info)));
    }
    let _ = write!(
        l,
        ",\"body\":{},\"name\":{},\"radius_km\":{},\"seed\":{},\"glamour\":{},\"free_drones\":{},\"speed\":{},\"window\":[{},{}]",
        text(&o.body),
        text(&p.spec.name),
        num(p.radius / 1000.0),
        p.spec.seed,
        num(o.glamour),
        c.free_drones.map_or("null".into(), |n| n.to_string()),
        text(&speed_name(c.speed)),
        o.size.0,
        o.size.1
    );
    let _ = write!(
        l,
        ",\"names\":{{\"parts\":{},\"sim_prof\":{},\"sim_counts\":{}}}}}",
        list_s(&PARTS),
        list_s(&dft_sim::swarm::PROF_NAMES),
        list_s(&dft_sim::swarm::COUNT_NAMES)
    );
    let mut log = Log {
        file,
        started: Instant::now(),
        last: Instant::now(),
        frames: Vec::new(),
        main: Vec::new(),
        parts: [0.0; PARTS.len()],
        parts_max: [0.0; PARTS.len()],
        quads: 0,
        tiles_shown: 0,
        hole_frames: 0,
        holes_max: 0,
        built: 0,
        rebuilt: 0,
        prof0: [0.0; 8],
        counts0: [0; 6],
        time0: 0.0,
        tile0: tile_totals(),
        control: Some(c),
        target_reached: false,
        slow_this_second: 0,
        frames_total: 0,
        slow_total: 0,
        peak_drones: 0,
        peak_mem_gib: 0.0,
        update_start: None,
    };
    log.write(&l);
    commands.insert_resource(log);
}

fn speed_name(i: usize) -> String {
    let s = SPEEDS[i.min(SPEEDS.len() - 1)];
    if s.is_finite() { format!("{s}x") } else { "max".into() }
}

fn tile_totals() -> [u64; 5] {
    use crate::tiles::*;
    [BUILDS.load(Relaxed), BUILD_NS.load(Relaxed), LIVE_READS.load(Relaxed), WAIT_NS.load(Relaxed), HOLD_NS.load(Relaxed)]
}

impl Log {
    fn write(&mut self, line: &str) {
        if let Some(f) = &mut self.file {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }

    fn event(&mut self, what: &str, extra: &str) {
        let t = self.started.elapsed().as_secs_f64();
        let line = format!("{{\"kind\":\"event\",\"t\":{},\"what\":{}{}}}", num(t), text(what), extra);
        self.write(&line);
    }
}

fn mark_update_start(log: Option<ResMut<Log>>) {
    if let Some(mut log) = log {
        log.update_start = Some(Instant::now());
    }
}

/// Every frame: sum up; once a second, write a sample.
fn sample(windows: Query<&Window, With<bevy::window::PrimaryWindow>>, log: Option<ResMut<Log>>, mut perf: ResMut<Perf>, time: Res<Time>, run: Res<crate::Run>, view: Res<crate::view::View>, tiles: Res<crate::tiles::Tiles>, diags: Res<DiagnosticsStore>) {
    let Some(mut log) = log else { return };
    let dt = time.delta_secs_f64();
    let main = log.update_start.map_or(0.0, |s| s.elapsed().as_secs_f64());
    log.frames_total += 1;
    if log.frames_total > 3 {
        log.frames.push(dt);
        log.main.push(main);
    }
    for i in 0..PARTS.len() {
        log.parts[i] += perf.frame[i];
        log.parts_max[i] = log.parts_max[i].max(perf.frame[i]);
    }
    log.quads = log.quads.max(perf.quads);
    log.tiles_shown = log.tiles_shown.max(perf.tiles_shown);
    if perf.tile_holes > 0 {
        log.hole_frames += 1;
        log.holes_max = log.holes_max.max(perf.tile_holes);
    }
    log.built += perf.built;
    log.rebuilt += perf.rebuilt;
    let events = std::mem::take(&mut perf.events);
    for (what, extra) in events {
        log.event(&what, &extra);
    }
    // A slow frame: say what took the time (the first few each second).
    if dt > SLOW_FRAME_S && log.frames_total > 10 {
        log.slow_total += 1;
        if log.slow_this_second < 5 {
            log.slow_this_second += 1;
            let extra = format!(",\"frame_ms\":{},\"main_ms\":{},\"parts_ms\":{},\"quads\":{}", num(dt * 1000.0), num(main * 1000.0), list_f(&perf.frame.map(|v| v * 1000.0)), perf.quads);
            log.event("slow frame", &extra);
        }
    }
    perf.frame = [0.0; PARTS.len()];
    perf.quads = 0;
    perf.tiles_shown = 0;
    perf.tile_holes = 0;
    perf.built = 0;
    perf.rebuilt = 0;
    // Changes to speed, pause or economy, however they were made.
    let c = *run.0.control.lock();
    if let Some(old) = log.control {
        if old.speed != c.speed || old.paused != c.paused {
            let extra = format!(",\"speed\":{},\"paused\":{}", text(&speed_name(c.speed)), c.paused);
            log.event("speed", &extra);
        }
        if old.free_drones != c.free_drones {
            let extra = format!(",\"free_drones\":{}", c.free_drones.map_or("null".into(), |n| n.to_string()));
            log.event("economy", &extra);
            log.target_reached = false;
        }
    }
    log.control = Some(c);
    if log.last.elapsed().as_secs_f64() < 1.0 {
        return;
    }
    let real = log.last.elapsed().as_secs_f64();
    log.last = Instant::now();
    log.slow_this_second = 0;
    let (stats, tp, step_max) = {
        let mut out = run.0.out.lock();
        (out.stats.clone(), std::mem::take(&mut out.perf), out.step_max)
    };
    // A new body restarts the simulation's totals.
    if stats.time + 1e-9 < log.time0 || stats.counts[0] < log.counts0[0] {
        log.prof0 = [0.0; 8];
        log.counts0 = [0; 6];
        log.time0 = 0.0;
    }
    let game = stats.time - log.time0;
    let per_game = |v: f64| if game > 0.0 { v / game } else { 0.0 };
    let prof_ms: Vec<f64> = (0..8).map(|i| per_game(stats.prof[i] - log.prof0[i]) * 1000.0).collect();
    let counts: Vec<f64> = (0..6).map(|i| if i == 5 { stats.counts[i] as f64 } else { per_game((stats.counts[i] - log.counts0[i]) as f64) }).collect();
    log.prof0 = stats.prof;
    log.counts0 = stats.counts;
    log.time0 = stats.time;
    if let Some(n) = c.free_drones {
        if !log.target_reached && stats.drones as u64 + stats.packet >= n.saturating_sub(n / 100) {
            log.target_reached = true;
            let extra = format!(",\"drones\":{},\"game_s\":{}", stats.drones, num(stats.time));
            log.event("swarm grown", &extra);
        }
    }
    // Frames this second.
    let mut f = std::mem::take(&mut log.frames);
    let mut m = std::mem::take(&mut log.main);
    f.sort_by(|a, b| a.total_cmp(b));
    m.sort_by(|a, b| a.total_cmp(b));
    let pct = |v: &[f64], q: f64| if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f64 * q).round() as usize] };
    let avg = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    let n = f.len().max(1) as f64;
    let parts_ms: Vec<f64> = log.parts.iter().map(|v| v / n * 1000.0).collect();
    let parts_max_ms: Vec<f64> = log.parts_max.iter().map(|v| v * 1000.0).collect();
    let tt = tile_totals();
    let td: Vec<u64> = (0..5).map(|i| tt[i] - log.tile0[i]).collect();
    log.tile0 = tt;
    let mem = diags.get(&SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE).and_then(|d| d.value()).unwrap_or(0.0);
    let cpu = diags.get(&SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE).and_then(|d| d.value()).unwrap_or(0.0);
    log.peak_drones = log.peak_drones.max(stats.drones);
    log.peak_mem_gib = log.peak_mem_gib.max(mem);
    let (kept, building, level) = tiles.counts();
    let ThreadPerf { batches, steps, wait_s, hold_s, publishes, publish_s, publish_max_s, sleep_s } = tp;
    let mut l = String::new();
    let _ = write!(
        l,
        "{{\"kind\":\"sample\",\"t\":{},\"real_s\":{},\"frames\":{},\"fps\":{},\"frame_ms\":{{\"avg\":{},\"p50\":{},\"p95\":{},\"max\":{}}},\"main_ms\":{{\"avg\":{},\"p95\":{},\"max\":{}}}",
        num(log.started.elapsed().as_secs_f64()),
        num(real),
        f.len(),
        num(f.len() as f64 / real),
        num(avg(&f) * 1000.0),
        num(pct(&f, 0.5) * 1000.0),
        num(pct(&f, 0.95) * 1000.0),
        num(pct(&f, 1.0) * 1000.0),
        num(avg(&m) * 1000.0),
        num(pct(&m, 0.95) * 1000.0),
        num(pct(&m, 1.0) * 1000.0)
    );
    let _ = write!(l, ",\"parts_ms\":{},\"parts_max_ms\":{},\"quads\":{},\"tiles_shown\":{},\"hole_frames\":{},\"holes_max\":{},\"tiles\":{{\"kept\":{},\"building\":{},\"level\":{},\"first_builds\":{},\"rebuilds\":{},\"worker_builds\":{},\"worker_ms_each\":{},\"live_reads\":{},\"wait_ms\":{},\"hold_ms\":{}}}", list_f(&parts_ms), list_f(&parts_max_ms), log.quads, log.tiles_shown, log.hole_frames, log.holes_max, kept, building, level, log.built, log.rebuilt, td[0], num(if td[0] > 0 { td[1] as f64 / td[0] as f64 / 1e6 } else { 0.0 }), td[2], num(td[3] as f64 / 1e6), num(td[4] as f64 / 1e6));
    let _ = write!(
        l,
        ",\"view\":{{\"x\":{},\"y\":{},\"mpp\":{}}},\"sim\":{{\"game_s\":{},\"speed\":{},\"paused\":{},\"achieved\":{},\"cost_ms\":{},\"prof_ms\":{},\"counts\":{},\"drones\":{},\"packet\":{},\"hubs\":{},\"frontier\":{},\"dormant\":{},\"awake\":{},\"loose\":{},\"deaths\":{},\"eaten\":{},\"haulers\":{},\"haulers_away\":{},\"starved\":{},\"waiting\":{},\"flocks\":{},\"splits\":{},\"merges\":{},\"helpers\":{},\"explosions\":{},\"mother\":{},\"step_max_ms\":{},\"step_max_split\":{}}}",
        num(view.x),
        num(view.y),
        num(view.mpp),
        num(stats.time),
        text(&speed_name(c.speed)),
        c.paused,
        num(game / real),
        num(prof_ms.iter().take(4).sum::<f64>()),
        list_f(&prof_ms),
        list_f(&counts),
        stats.drones,
        stats.packet,
        stats.hubs,
        stats.frontier,
        stats.dormant,
        stats.awake,
        stats.loose,
        stats.deaths,
        num(stats.consumed),
        stats.haulers,
        stats.haulers_away,
        stats.starved,
        stats.waiting,
        stats.flocks,
        stats.splits,
        stats.merges,
        stats.helpers,
        stats.explosions,
        num(stats.mother_share),
        num(step_max.0),
        list_f(&step_max.1)
    );
    let _ = write!(
        l,
        ",\"sim_thread\":{{\"batches\":{},\"steps\":{},\"wait_ms\":{},\"hold_ms\":{},\"sleep_ms\":{},\"publishes\":{},\"publish_ms_avg\":{},\"publish_ms_max\":{}}},\"process\":{{\"mem_gib\":{},\"cpu_pct\":{}}}}}",
        batches,
        steps,
        num(wait_s * 1000.0),
        num(hold_s * 1000.0),
        num(sleep_s * 1000.0),
        publishes,
        num(if publishes > 0 { publish_s / publishes as f64 * 1000.0 } else { 0.0 }),
        num(publish_max_s * 1000.0),
        num(mem),
        num(cpu)
    );
    // Whether the window had focus (Windows and graphics drivers may slow a
    // window in the background).
    let focused = windows.single().map(|w| w.focused).unwrap_or(true);
    l.insert_str(l.len() - 1, &format!(",\"focused\":{focused}"));
    log.write(&l);
    log.parts = [0.0; PARTS.len()];
    log.parts_max = [0.0; PARTS.len()];
    log.quads = 0;
    log.tiles_shown = 0;
    log.hole_frames = 0;
    log.holes_max = 0;
    log.built = 0;
    log.rebuilt = 0;
}

fn write_end_on_exit(log: Option<ResMut<Log>>, mut exit: MessageReader<AppExit>) {
    let Some(mut log) = log else { return };
    if exit.read().next().is_none() {
        return;
    }
    let extra = format!(
        ",\"real_s\":{},\"frames\":{},\"slow_frames\":{},\"peak_drones\":{},\"peak_mem_gib\":{}",
        num(log.started.elapsed().as_secs_f64()),
        log.frames_total,
        log.slow_total,
        log.peak_drones,
        num(log.peak_mem_gib)
    );
    let line = format!("{{\"kind\":\"end\"{extra}}}");
    log.write(&line);
}

pub fn json_text(s: &str) -> String {
    text(s)
}

pub fn json_num(v: f64) -> String {
    num(v)
}
