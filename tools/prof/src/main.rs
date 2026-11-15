//! dft-prof: a sampling profiler for the simulation (Windows only).
//!
//! Runs the "500k cutters" bench set-up (cutters and haulers, zoomed out)
//! and, once it has warmed up, pauses the simulation's thread every
//! millisecond, walks its stack, and counts where it was. At the end it
//! prints the functions it was most often in (self) and under (inclusive),
//! and the source lines most often running.
//!
//! Environment: BODY (living-world), KM (35), DRONES (1000000), WARM (game
//! seconds before sampling, 600), SECS (game seconds sampled, 200), TOP (40).
//! Build and run: cd tools/prof && cargo run --release

use dft_sim::economy::Economy;
use dft_sim::production::Unit;
use dft_sim::swarm::Sim;
use dft_sim::world::World;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Diagnostics::Debug::*;
use windows_sys::Win32::System::Threading::*;

const DEPTH: usize = 128;

/// CONTEXT must be 16-byte aligned for GetThreadContext.
#[repr(C, align(16))]
struct Ctx(CONTEXT);

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// One stack: the program counters, innermost first.
unsafe fn sample(th: HANDLE, out: &mut [u64; DEPTH]) -> usize {
    let mut c: Ctx = unsafe { std::mem::zeroed() };
    c.0.ContextFlags = CONTEXT_CONTROL_AMD64 | CONTEXT_INTEGER_AMD64;
    if unsafe { SuspendThread(th) } == u32::MAX {
        return 0;
    }
    let mut n = 0;
    if unsafe { GetThreadContext(th, &mut c.0) } != 0 {
        while n < DEPTH {
            let pc = c.0.Rip;
            if pc == 0 {
                break;
            }
            out[n] = pc;
            n += 1;
            let mut base = 0u64;
            let fe = unsafe { RtlLookupFunctionEntry(pc, &mut base, std::ptr::null_mut()) };
            if fe.is_null() {
                // A leaf with no unwind data: its return address is on top.
                if c.0.Rsp == 0 {
                    break;
                }
                c.0.Rip = unsafe { *(c.0.Rsp as *const u64) };
                c.0.Rsp += 8;
            } else {
                let mut hd = std::ptr::null_mut();
                let mut est = 0u64;
                unsafe { RtlVirtualUnwind(UNW_FLAG_NHANDLER, base, pc, fe, &mut c.0, &mut hd, &mut est, std::ptr::null_mut()) };
            }
        }
    }
    unsafe { ResumeThread(th) };
    n
}

struct Names {
    proc_: HANDLE,
    funcs: HashMap<u64, String>,
    lines: HashMap<u64, String>,
}

impl Names {
    fn new() -> Self {
        let proc_ = unsafe { GetCurrentProcess() };
        unsafe {
            SymSetOptions(SYMOPT_UNDNAME | SYMOPT_DEFERRED_LOADS | SYMOPT_LOAD_LINES | SYMOPT_DEBUG);
            let exe = std::env::current_exe().unwrap();
            let dir = exe.parent().unwrap().to_string_lossy().to_string();
            let path = std::ffi::CString::new(format!("{dir};{dir}/deps")).unwrap();
            if SymInitialize(proc_, path.as_ptr() as *const u8, 1) == 0 {
                eprintln!("SymInitialize failed: {}", windows_sys::Win32::Foundation::GetLastError());
            }
        }
        Names { proc_, funcs: HashMap::new(), lines: HashMap::new() }
    }

    fn func(&mut self, pc: u64) -> String {
        if let Some(s) = self.funcs.get(&pc) {
            return s.clone();
        }
        let mut buf = vec![0u64; (std::mem::size_of::<SYMBOL_INFO>() + 2048) / 8 + 1];
        let si = buf.as_mut_ptr() as *mut SYMBOL_INFO;
        let name = unsafe {
            (*si).SizeOfStruct = std::mem::size_of::<SYMBOL_INFO>() as u32;
            (*si).MaxNameLen = 2000;
            let mut disp = 0u64;
            if SymFromAddr(self.proc_, pc, &mut disp, si) != 0 {
                let p = (*si).Name.as_ptr() as *const u8;
                let s = std::slice::from_raw_parts(p, (*si).NameLen as usize);
                tidy(&String::from_utf8_lossy(s))
            } else {
                static ONCE: std::sync::Once = std::sync::Once::new();
                ONCE.call_once(|| eprintln!("SymFromAddr failed: {}", windows_sys::Win32::Foundation::GetLastError()));
                format!("?{pc:x}")
            }
        };
        self.funcs.insert(pc, name.clone());
        name
    }

    fn line(&mut self, pc: u64) -> String {
        if let Some(s) = self.lines.get(&pc) {
            return s.clone();
        }
        let mut l: IMAGEHLP_LINE64 = unsafe { std::mem::zeroed() };
        l.SizeOfStruct = std::mem::size_of::<IMAGEHLP_LINE64>() as u32;
        let mut disp = 0u32;
        let s = unsafe {
            if SymGetLineFromAddr64(self.proc_, pc, &mut disp, &mut l) != 0 && !l.FileName.is_null() {
                let f = std::ffi::CStr::from_ptr(l.FileName as *const i8).to_string_lossy().to_string();
                let short = f.rsplit(['\\', '/']).take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/");
                format!("{short}:{}", l.LineNumber)
            } else {
                "?".into()
            }
        };
        self.lines.insert(pc, s.clone());
        s
    }
}

/// Shorter names: drop the hash and the crate paths.
fn tidy(s: &str) -> String {
    let s = match s.rfind("::h") {
        Some(i) if s.len() - i == 19 => &s[..i],
        _ => s,
    };
    s.replace("dft_sim::", "").replace("core::", "").replace("alloc::", "").replace("std::", "")
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let data = root.join("data");
    let body: String = env("BODY", "living-world".to_string());
    let km: f64 = env("KM", 35.0);
    let drones: u64 = env("DRONES", 1_000_000);
    let warm: f64 = env("WARM", 600.0);
    let secs: f64 = env("SECS", 200.0);
    let top: usize = env("TOP", 40);

    let text = std::fs::read_to_string(data.join("bodies").join(format!("{body}.toml"))).unwrap();
    let text = dft_sim::spec::resize_text(&text, km * 1000.0).unwrap();
    let mut spec = dft_sim::spec::BodySpec::parse(&text).unwrap();
    if spec.life.is_none() {
        spec.life = Some(1.0);
    }
    let mats = dft_sim::materials::MaterialTable::load(&data.join("materials.toml").to_string_lossy()).unwrap();
    let planet = dft_sim::planet::Planet::new(spec, mats).unwrap();
    let mut e = Economy::load(&data.join("economy.toml").to_string_lossy()).unwrap();
    e.seed_swarm.start_harvesters = drones as u32;
    e.units.bay.retain(|b| matches!(b.unit, Unit::Cutter | Unit::Hauler));
    let mut s = Sim::new(World::new(planet), e);
    s.free_drones = Some(drones);
    s.set_attention(Some(None));
    if std::env::var("NOFOCUS").is_err() {
        s.set_focus(Some(None));
    }
    println!("dft-prof: {body} {km} km, {drones} drones; warming {warm:.0} game s, then sampling {secs:.0} game s");
    let t0 = std::time::Instant::now();
    s.run(warm);
    println!("  warm in {:.1} s real; {} working", t0.elapsed().as_secs_f64(), {
        let (_, w, o, h) = s.phase_counts();
        w + o + h
    });

    let me = Arc::new(AtomicU32::new(unsafe { GetCurrentThreadId() }));
    let stop = Arc::new(AtomicBool::new(false));
    let (me2, stop2) = (me.clone(), stop.clone());
    let sampler = std::thread::spawn(move || {
        let th = unsafe { OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, me2.load(Ordering::Relaxed)) };
        let mut stacks: Vec<Vec<u64>> = Vec::new();
        let mut buf = [0u64; DEPTH];
        while !stop2.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(1));
            let n = unsafe { sample(th, &mut buf) };
            if n > 0 {
                stacks.push(buf[..n].to_vec());
            }
        }
        unsafe { CloseHandle(th) };
        stacks
    });
    let t1 = std::time::Instant::now();
    s.run(warm + secs);
    let real = t1.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let stacks = sampler.join().unwrap();
    println!("  sampled {secs:.0} game s in {real:.1} s real ({:.1}x); {} samples", secs / real, stacks.len());

    let mut names = Names::new();
    let n = stacks.len().max(1) as f64;
    let mut selff: HashMap<String, u64> = HashMap::new();
    let mut incl: HashMap<String, u64> = HashMap::new();
    let mut lines: HashMap<String, u64> = HashMap::new();
    for st in &stacks {
        let f0 = names.func(st[0]);
        *selff.entry(f0.clone()).or_default() += 1;
        let l0 = names.line(st[0]);
        *lines.entry(format!("{l0}  [{f0}]")).or_default() += 1;
        let mut seen = std::collections::HashSet::new();
        for &pc in st {
            // (Return addresses point just past the call.)
            let f = names.func(pc.saturating_sub(1));
            if seen.insert(f.clone()) {
                *incl.entry(f).or_default() += 1;
            }
        }
    }
    let show = |title: &str, m: &HashMap<String, u64>| {
        let mut v: Vec<(&String, &u64)> = m.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        println!("\n{title}");
        for (k, c) in v.iter().take(top) {
            let k = if k.len() > 150 { &k[..150] } else { k.as_str() };
            println!("  {:5.1}%  {k}", **c as f64 / n * 100.0);
        }
    };
    show("inclusive (in it or under it)", &incl);
    show("self (running in it)", &selff);
    show("lines (running)", &lines);
}
