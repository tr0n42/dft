//! The Dark Forest Theory: the game.
//!
//! A native client over the simulation library (dft-sim). The simulation runs
//! on a thread of its own (sim.rs); this side draws the body as tiles built at
//! the level of detail the zoom needs (tiles.rs), the drones moving smoothly
//! between the simulation's reports (drones.rs), and, for development only, a
//! panel of numbers and pickers on F1 (dev.rs). Everything is drawn relative
//! to the camera, so the picture stays sharp millions of metres from the centre.
//!
//! Options (all optional):
//!   --body <type>         a file in data/bodies (default rubble-asteroid)
//!   --seed <n>            the body's seed (default the file's own)
//!   --radius-km <r>       resize the body
//!   --glamour <g>         the glamour pass, 0 to 1 (default 0.4)
//!   --speed <1-8>         the starting speed key (default 1, real time)
//!   --paused              start paused
//!   --run-s <t>           run to game time t as fast as possible, then pause
//!   --drones <n>          economy off: the swarm grows to n drones (default 20000)
//!   --economy             play the real economy instead
//!   --dev                 open the F1 panel at the start
//!   --director            start with the action cam on (C toggles it in play;
//!                         off by default: the camera only moves when you move it)
//!   --zoom-limit          limit zooming out until the body is charted (off by default)
//!   --no-fog              start with the fog of war off (F2 toggles it)
//!   --full-detail         move loose matter everywhere, not only on screen
//!   --camera [plan]       benchmark: a scripted camera (see bench.rs; the
//!                         default plan without one), closing at its end
//!   --quit-after <s>      close by itself after s seconds of real time (benchmarks)
//!   --screenshot <file>   draw, save a PNG of the window and quit
//!   --frames <n>          with --screenshot: n pictures, 4 frames apart
//!   --view <x>,<y>,<mpp>  where to look (metres, and metres per pixel)
//!   --size <w>x<h>        the window size (default 1600x900)
//!   --continue            load the newest save (saves folder)
//!   --load <file>         load a save (a path, or a name in the saves folder)
//!   --no-autosave         don't autosave (benchmarks and screenshots never do)
//!   --background          open without taking focus, behind other windows
//!
//! Controls: drag with either mouse button (or the arrow keys / WASD) to
//! pan, the wheel (or Page Up / Page Down) zooms toward the cursor, Home
//! shows the whole body, C turns the action cam on and off. Space pauses, 1 to 8 pick a speed (1x, 2x, 5x, 10x,
//! 30x, 100x, 300x, as fast as possible), + and - step it. F1: dev panel.
//! F2: fog of war on and off. F5 saves, F9 loads the newest save. The game
//! also autosaves every five minutes and on quitting (the last three kept).
mod bench;
mod camera;
mod dev;
mod drones;
mod eyedropper;
mod finder;
mod fog;
mod log;
mod saves;
mod sim;
mod stars;
mod stockpile;
mod storage;
mod swarmview;
mod lights;
mod cargo;
mod rnd;
mod tiles;
mod view;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use std::path::PathBuf;
use std::sync::Arc;

/// What the command line asked for.
#[derive(Resource, Clone, Debug)]
pub struct Options {
    pub body: String,
    pub seed: Option<u64>,
    pub radius_km: Option<f64>,
    pub glamour: f64,
    pub speed: usize,
    pub paused: bool,
    pub run_s: Option<f64>,
    pub dev: bool,
    pub free_drones: Option<u64>,
    pub quit_after: Option<f64>,
    pub full_detail: bool,
    pub camera: Option<String>,
    pub no_fog: bool,
    pub no_director: bool,
    pub free_zoom: bool,
    pub screenshot: Option<PathBuf>,
    /// With --screenshot: this many pictures, a few frames apart (name-0.png, ...).
    pub frames: u32,
    pub view: Option<(f64, f64, f64)>,
    pub size: (u32, u32),
    /// A window of `size` (--size, --windowed) rather than borderless full screen.
    pub windowed: bool,
    /// A save to load at the start: "last" for the newest, or a file.
    pub load: Option<String>,
    /// Autosave every few minutes and on quitting (off for benchmarks).
    pub autosave: bool,
    /// Open without taking focus, behind other windows (benchmarks).
    pub background: bool,
}

impl Options {
    fn parse() -> Result<Options, String> {
        let mut o = Options {
            body: "rubble-asteroid".into(),
            seed: None,
            radius_km: None,
            glamour: 0.4,
            speed: 0,
            paused: false,
            run_s: None,
            dev: false,
            free_drones: Some(20000),
            quit_after: None,
            full_detail: false,
            camera: None,
            no_fog: false,
            // While the game is being built the player drives: the action
            // cam and the zoom limit are opt-in (--director, --zoom-limit).
            no_director: true,
            free_zoom: true,
            screenshot: None,
            frames: 1,
            view: None,
            size: (1600, 900),
            windowed: false,
            load: None,
            autosave: true,
            background: false,
        };
        let a: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        let val = |i: usize| a.get(i + 1).cloned().ok_or(format!("{} needs a value", a[i]));
        while i < a.len() {
            let mut used = 2;
            match a[i].as_str() {
                "--body" => o.body = val(i)?,
                "--seed" => o.seed = Some(val(i)?.parse().map_err(|_| "--seed takes a number")?),
                "--radius-km" => o.radius_km = Some(val(i)?.parse().map_err(|_| "--radius-km takes a number")?),
                "--glamour" => o.glamour = val(i)?.parse().map_err(|_| "--glamour takes a number")?,
                "--speed" => {
                    let s: usize = val(i)?.parse().map_err(|_| "--speed takes 1 to 8")?;
                    o.speed = s.clamp(1, sim::SPEEDS.len()) - 1;
                }
                "--run-s" => o.run_s = Some(val(i)?.parse().map_err(|_| "--run-s takes seconds")?),
                "--paused" => {
                    o.paused = true;
                    used = 1;
                }
                "--drones" => o.free_drones = Some(val(i)?.parse().map_err(|_| "--drones takes a number")?),
                "--quit-after" => o.quit_after = Some(val(i)?.parse().map_err(|_| "--quit-after takes seconds")?),
                "--camera" => {
                    // The plan is optional: the next word, unless it's another option.
                    match a.get(i + 1).filter(|v| !v.starts_with("--")) {
                        Some(p) => o.camera = Some(p.clone()),
                        None => {
                            o.camera = Some(bench::DEFAULT_PLAN.into());
                            used = 1;
                        }
                    }
                }
                "--no-director" => {
                    o.no_director = true;
                    used = 1;
                }
                "--director" => {
                    o.no_director = false;
                    used = 1;
                }
                "--free-zoom" => {
                    o.free_zoom = true;
                    used = 1;
                }
                "--zoom-limit" => {
                    o.free_zoom = false;
                    used = 1;
                }
                "--no-fog" => {
                    o.no_fog = true;
                    used = 1;
                }
                "--full-detail" => {
                    o.full_detail = true;
                    used = 1;
                }
                "--economy" => {
                    o.free_drones = None;
                    used = 1;
                }
                "--continue" => {
                    o.load = Some("last".into());
                    used = 1;
                }
                "--load" => o.load = Some(val(i)?),
                "--background" => {
                    o.background = true;
                    used = 1;
                }
                "--no-autosave" => {
                    o.autosave = false;
                    used = 1;
                }
                "--dev" => {
                    o.dev = true;
                    used = 1;
                }
                "--screenshot" => o.screenshot = Some(PathBuf::from(val(i)?)),
                "--frames" => o.frames = val(i)?.parse().map_err(|_| "--frames takes a number")?,
                "--view" => {
                    let v: Vec<f64> = val(i)?.split(',').filter_map(|x| x.parse().ok()).collect();
                    if v.len() != 3 {
                        return Err("--view takes x,y,mpp".into());
                    }
                    o.view = Some((v[0], v[1], v[2]));
                }
                "--size" => {
                    let v: Vec<u32> = val(i)?.split('x').filter_map(|x| x.parse().ok()).collect();
                    if v.len() != 2 {
                        return Err("--size takes WIDTHxHEIGHT".into());
                    }
                    o.size = (v[0], v[1]);
                    o.windowed = true;
                }
                "--windowed" => {
                    o.windowed = true;
                    used = 1;
                }
                other => return Err(format!("unknown option {other}")),
            }
            i += used;
        }
        // A screenshot is of a still picture: paused unless asked to run first.
        if o.screenshot.is_some() && o.run_s.is_none() {
            o.paused = true;
        }
        // Benchmarks and screenshots leave the player's saves alone.
        if o.camera.is_some() || o.screenshot.is_some() || o.quit_after.is_some() {
            o.autosave = false;
        }
        Ok(o)
    }
}

/// The body being played (the generator), shared with the tile workers, and
/// what it was built from (for saves).
#[derive(Resource, Clone)]
pub struct Body(pub Arc<dft_sim::planet::Planet>, pub Arc<Source>);

/// What a body was built from: its type's name and the body file's text as
/// used (resized if asked), with the seed and glamour applied on top.
#[derive(Clone, Debug)]
pub struct Source {
    pub name: String,
    pub text: String,
    pub seed: u64,
    pub glamour: f64,
}

/// The running simulation.
#[derive(Resource, Clone)]
pub struct Run(pub Arc<sim::Shared>);

/// Where the game's data lives: next to the executable's crate when run
/// with cargo, or a `data` folder beside the executable.
pub fn data_dir() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("data");
    if here.exists() {
        return here;
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("data"))).unwrap_or_else(|| PathBuf::from("data"))
}

/// What to build: a body type, and optionally its seed, size and glamour.
#[derive(Clone, Debug)]
pub struct Recipe {
    pub body: String,
    pub seed: Option<u64>,
    pub radius_km: Option<f64>,
    pub glamour: f64,
}

/// A game ready to play: the body, its simulation, what the body was built
/// from, and (when loaded from a save) where things were left.
pub struct Start {
    pub planet: Arc<dft_sim::planet::Planet>,
    pub sim: dft_sim::swarm::Sim,
    pub source: Source,
    pub restore: Option<saves::Restore>,
}

/// The body from its source, and the economy, from the data folder.
pub fn build_body(src: &Source) -> Result<(Arc<dft_sim::planet::Planet>, dft_sim::economy::Economy), String> {
    let data = data_dir();
    let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy())?;
    let mut spec = dft_sim::spec::BodySpec::parse(&src.text)?;
    spec.seed = src.seed;
    spec.glamour = Some(src.glamour);
    // Life is on in the game wherever a body's chemistry allows it.
    if spec.life.is_none() {
        spec.life = Some(1.0);
    }
    let planet = Arc::new(dft_sim::planet::Planet::new(spec, mats)?);
    let eco = dft_sim::economy::Economy::load(&data.join("economy.toml").to_string_lossy())?;
    Ok((planet, eco))
}

/// Build the body and a fresh simulation over it.
pub fn make(r: &Recipe) -> Result<Start, String> {
    let data = data_dir();
    let mut text = std::fs::read_to_string(data.join("bodies").join(format!("{}.toml", r.body))).map_err(|e| format!("{}: {e}", r.body))?;
    if let Some(km) = r.radius_km {
        text = dft_sim::spec::resize_text(&text, km * 1000.0)?;
    }
    let seed = match r.seed {
        Some(s) => s,
        None => dft_sim::spec::BodySpec::parse(&text)?.seed,
    };
    let source = Source { name: r.body.clone(), text, seed, glamour: r.glamour };
    let (planet, eco) = build_body(&source)?;
    let sim = dft_sim::swarm::Sim::new(dft_sim::world::World::shared(planet.clone()), eco);
    Ok(Start { planet, sim, source, restore: None })
}

pub fn title(p: &dft_sim::planet::Planet) -> String {
    format!("The Dark Forest Theory · {} · {:.0} km · seed {}", p.spec.name, p.radius / 1000.0, p.spec.seed)
}

fn main() {
    // The simulation's parallel work gets two thirds of the machine.
    let threads = dft_sim::use_sim_threads();
    println!("simulation threads: {threads}");
    let o = match Options::parse() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let recipe = Recipe { body: o.body.clone(), seed: o.seed, radius_km: o.radius_km, glamour: o.glamour };
    let started = match &o.load {
        Some(which) => saves::find(which).and_then(|p| saves::read_file(&p)).map_err(|e| format!("couldn't load the save: {e}")),
        None => make(&recipe).map_err(|e| format!("couldn't build the body: {e}")),
    };
    let start = match started {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let Start { planet, sim, source, restore } = start;
    let mut control = sim::Control { speed: if o.run_s.is_some() { sim::SPEEDS.len() - 1 } else { o.speed }, paused: o.paused, pause_at: o.run_s, free_drones: o.free_drones, attention: None, full_detail: o.full_detail, quit: false };
    if let Some(r) = &restore {
        control.free_drones = r.free_drones;
    }
    let script = match o.camera.as_deref().map(bench::parse) {
        Some(Ok(steps)) => Some(bench::Script::new(steps)),
        Some(Err(e)) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
        None => None,
    };
    let run = sim::start(sim, control);
    let mut app = App::new();
    if let Some(s) = script {
        app.insert_resource(s);
    }
    if let Some(r) = restore {
        app.insert_resource(saves::Restoring(Some(r)));
    }
    app
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: title(&planet),
                resolution: WindowResolution::new(o.size.0, o.size.1),
                // Full screen in a borderless window, unless asked for a window.
                mode: if o.windowed || o.background { bevy::window::WindowMode::Windowed } else { bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current) },
                focused: !o.background,
                window_level: if o.background { bevy::window::WindowLevel::AlwaysOnBottom } else { bevy::window::WindowLevel::Normal },
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb_u8(6, 7, 12)))
        .insert_resource(Body(planet, Arc::new(source)))
        .insert_resource(Run(run))
        .insert_resource(o)
        .add_plugins((view::ViewPlugin, tiles::TilesPlugin, drones::DronesPlugin, dev::DevPlugin, log::LogPlugin, bench::BenchPlugin, stars::StarsPlugin, fog::FogPlugin, camera::CameraPlugin, saves::SavesPlugin, eyedropper::EyedropperPlugin, stockpile::StockpilePlugin, finder::FinderPlugin))
        .add_plugins((storage::StoragePlugin, swarmview::SwarmViewPlugin, lights::LightsPlugin))
        .add_systems(Startup, |mut c: Commands| {
            c.spawn(Camera2d);
        })
        .add_systems(Update, (speed_keys.run_if(not(bevy_egui::input::egui_wants_any_keyboard_input)), screenshot_when_ready, quit_after, attention))
        .run();
}

/// With --quit-after: close once that much real time has passed.
fn quit_after(o: Res<Options>, time: Res<Time<Real>>, mut exit: MessageWriter<AppExit>) {
    if o.quit_after.is_some_and(|s| time.elapsed_secs_f64() >= s) {
        exit.write(AppExit::Success);
    }
}

/// Loose matter only moves where it can be seen: on screen, and only when
/// cells there are at least half a pixel across. Elsewhere it waits, and
/// moves once the view comes over it.
fn attention(view: Res<view::View>, run: Res<Run>) {
    const MARGIN: f64 = 128.0;
    let a = if view.mpp <= 2.0 {
        let (l, b, r, t) = view.bounds();
        Some([(l - MARGIN).floor() as i64, (r + MARGIN).ceil() as i64, (b - MARGIN).floor() as i64, (t + MARGIN).ceil() as i64])
    } else {
        None
    };
    let mut c = run.0.control.lock();
    if c.attention != Some(a) {
        c.attention = Some(a);
    }
}

/// Space pauses; 1 to 8 pick a speed; + and - step it.
fn speed_keys(keys: Res<ButtonInput<KeyCode>>, run: Res<Run>) {
    let mut c = run.0.control.lock();
    if keys.just_pressed(KeyCode::Space) {
        c.paused = !c.paused;
    }
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8];
    for (i, k) in digits.iter().enumerate() {
        if keys.just_pressed(*k) {
            c.speed = i;
            c.paused = false;
        }
    }
    if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
        c.speed = (c.speed + 1).min(sim::SPEEDS.len() - 1);
    }
    if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
        c.speed = c.speed.saturating_sub(1);
    }
}

/// In screenshot mode: once the simulation has paused and every tile in view
/// is drawn, save the window and quit.
fn screenshot_when_ready(mut commands: Commands, o: Res<Options>, time: Res<Time>, tiles: Res<tiles::Tiles>, run: Res<Run>, mut state: Local<(u32, bool, u32)>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = &o.screenshot else { return };
    let shoot = |commands: &mut Commands, n: u32| {
        let p = if o.frames > 1 {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            path.with_file_name(format!("{stem}-{n}.png"))
        } else {
            path.clone()
        };
        commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(p));
    };
    if state.1 {
        state.0 += 1;
        // More pictures, a few frames apart.
        if state.2 < o.frames && state.0 % 4 == 0 {
            shoot(&mut commands, state.2);
            state.2 += 1;
        }
        if state.2 >= o.frames && state.0 > 20 + 4 * o.frames {
            exit.write(AppExit::Success);
        }
        return;
    }
    // (Wait a moment at the start too: the GPU builds its pipelines in the
    // background and draws nothing until they're ready.)
    let paused = run.0.control.lock().paused;
    if paused && tiles.settled() && time.elapsed_secs() > 2.0 {
        state.0 += 1;
    } else {
        state.0 = 0;
    }
    if state.0 >= 10 {
        shoot(&mut commands, 0);
        state.1 = true;
        state.0 = 0;
        state.2 = 1;
    }
}
