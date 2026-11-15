//! attach: sample another running program's threads (Windows), e.g. the
//! game while it runs a benchmark. Every couple of milliseconds each thread
//! is paused, its stack walked, and the functions counted, per thread
//! (threads named by their description: "main", the simulation, the
//! render thread, workers). Waiting threads are counted apart.
//!
//! Run: attach.exe <program name or pid> [seconds] [top]

use std::collections::HashMap;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Diagnostics::Debug::*;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
use windows_sys::Win32::System::Threading::*;

#[repr(C, align(16))]
struct Ctx(CONTEXT);

fn find_pid(name: &str) -> Option<u32> {
    if let Ok(p) = name.parse() {
        return Some(p);
    }
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        let mut pe: PROCESSENTRY32W = std::mem::zeroed();
        pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut pe) != 0;
        while ok {
            let n = String::from_utf16_lossy(&pe.szExeFile).trim_end_matches('\0').to_lowercase();
            if n == name.to_lowercase() {
                CloseHandle(snap);
                return Some(pe.th32ProcessID);
            }
            ok = Process32NextW(snap, &mut pe) != 0;
        }
        CloseHandle(snap);
    }
    None
}

fn threads(pid: u32) -> Vec<u32> {
    let mut v = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        let mut te: THREADENTRY32 = std::mem::zeroed();
        te.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut ok = Thread32First(snap, &mut te) != 0;
        while ok {
            if te.th32OwnerProcessID == pid {
                v.push(te.th32ThreadID);
            }
            ok = Thread32Next(snap, &mut te) != 0;
        }
        CloseHandle(snap);
    }
    v
}

fn thread_name(th: HANDLE, tid: u32) -> String {
    unsafe {
        let mut p: windows_sys::core::PWSTR = std::ptr::null_mut();
        if GetThreadDescription(th, &mut p) >= 0 && !p.is_null() {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            windows_sys::Win32::Foundation::LocalFree(p as _);
            if !s.is_empty() {
                return s;
            }
        }
    }
    format!("thread {tid}")
}

fn walk(proc_: HANDLE, th: HANDLE) -> Vec<u64> {
    let mut out = Vec::new();
    unsafe {
        let mut c: Ctx = std::mem::zeroed();
        c.0.ContextFlags = CONTEXT_FULL_AMD64;
        if SuspendThread(th) == u32::MAX {
            return out;
        }
        if GetThreadContext(th, &mut c.0) != 0 {
            let mut f: STACKFRAME64 = std::mem::zeroed();
            f.AddrPC.Offset = c.0.Rip;
            f.AddrPC.Mode = AddrModeFlat;
            f.AddrFrame.Offset = c.0.Rbp;
            f.AddrFrame.Mode = AddrModeFlat;
            f.AddrStack.Offset = c.0.Rsp;
            f.AddrStack.Mode = AddrModeFlat;
            for _ in 0..96 {
                let ok = StackWalk64(
                    0x8664, // IMAGE_FILE_MACHINE_AMD64
                    proc_,
                    th,
                    &mut f,
                    &mut c.0 as *mut CONTEXT as *mut core::ffi::c_void,
                    None,
                    Some(SymFunctionTableAccess64),
                    Some(SymGetModuleBase64),
                    None,
                );
                if ok == 0 || f.AddrPC.Offset == 0 {
                    break;
                }
                out.push(f.AddrPC.Offset);
            }
        }
        ResumeThread(th);
    }
    out
}

fn tidy(s: &str) -> String {
    let s = match s.rfind("::h") {
        Some(i) if s.len() - i == 19 => &s[..i],
        _ => s,
    };
    s.replace("dft_sim::", "").replace("core::", "").replace("alloc::", "").replace("std::", "")
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let name = a.first().cloned().unwrap_or("dark-forest.exe".into());
    let secs: f64 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(20.0);
    let top: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    let Some(pid) = find_pid(&name) else {
        eprintln!("no running {name}");
        std::process::exit(1);
    };
    let proc_ = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
    unsafe {
        SymSetOptions(SYMOPT_UNDNAME | SYMOPT_DEFERRED_LOADS | SYMOPT_LOAD_LINES);
        // (SYMPATH: where the program's .pdb files are, if not found.)
        let path = std::env::var("SYMPATH").ok().map(|p| std::ffi::CString::new(p).unwrap());
        if SymInitialize(proc_, path.as_ref().map_or(std::ptr::null(), |p| p.as_ptr() as *const u8), 1) == 0 {
            eprintln!("SymInitialize failed: {}", windows_sys::Win32::Foundation::GetLastError());
        }
    }
    println!("attach: {name} (pid {pid}), sampling {secs:.0} s");
    let mut stacks: HashMap<String, Vec<Vec<u64>>> = HashMap::new();
    let t0 = std::time::Instant::now();
    let mut ths: Vec<(u32, HANDLE, String)> = Vec::new();
    let mut refreshed = -1.0;
    while t0.elapsed().as_secs_f64() < secs {
        let now = t0.elapsed().as_secs_f64();
        if now - refreshed >= 1.0 {
            refreshed = now;
            for (_, h, _) in ths.drain(..) {
                unsafe { CloseHandle(h) };
            }
            for tid in threads(pid) {
                let h = unsafe { OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, tid) };
                if !h.is_null() {
                    let n = thread_name(h, tid);
                    ths.push((tid, h, n));
                }
            }
            unsafe { SymRefreshModuleList(proc_) };
        }
        for (_, h, n) in &ths {
            let st = walk(proc_, *h);
            if !st.is_empty() {
                stacks.entry(n.clone()).or_default().push(st);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let mut names: HashMap<u64, String> = HashMap::new();
    let mut func = |pc: u64| -> String {
        if let Some(s) = names.get(&pc) {
            return s.clone();
        }
        let mut buf = vec![0u64; (std::mem::size_of::<SYMBOL_INFO>() + 2048) / 8 + 1];
        let si = buf.as_mut_ptr() as *mut SYMBOL_INFO;
        let s = unsafe {
            (*si).SizeOfStruct = std::mem::size_of::<SYMBOL_INFO>() as u32;
            (*si).MaxNameLen = 2000;
            let mut d = 0u64;
            if SymFromAddr(proc_, pc, &mut d, si) != 0 {
                tidy(&String::from_utf8_lossy(std::slice::from_raw_parts((*si).Name.as_ptr() as *const u8, (*si).NameLen as usize)))
            } else {
                format!("?{pc:x}")
            }
        };
        names.insert(pc, s.clone());
        s
    };
    let waiting = ["ZwWaitFor", "NtWaitFor", "ZwRemoveIoCompletion", "NtRemoveIoCompletion", "ZwDelayExecution", "NtDelayExecution", "WaitOnAddress", "ZwWaitForAlertByThreadId", "NtUserMsgWaitForMultipleObjectsEx", "ZwUserMsgWaitForMultipleObjectsEx", "SleepConditionVariable"];
    let mut by_thread: Vec<(String, Vec<Vec<u64>>)> = stacks.into_iter().collect();
    // Busiest threads first (samples not in a wait).
    let mut busy: Vec<(String, usize, usize, Vec<Vec<u64>>)> = Vec::new();
    for (n, v) in by_thread.drain(..) {
        let mut b = Vec::new();
        let total = v.len();
        for st in v {
            let f0 = func(st[0]);
            if !waiting.iter().any(|w| f0.contains(w)) {
                b.push(st);
            }
        }
        busy.push((n, total, b.len(), b));
    }
    busy.sort_by(|a, b| b.2.cmp(&a.2));
    for (n, total, nb, st) in busy.iter().take(6) {
        if *nb == 0 {
            continue;
        }
        println!("\n=== {n}: busy {:.0}% of {total} samples", *nb as f64 / *total as f64 * 100.0);
        let mut incl: HashMap<String, u64> = HashMap::new();
        let mut selff: HashMap<String, u64> = HashMap::new();
        for s in st {
            *selff.entry(func(s[0])).or_default() += 1;
            let mut seen = std::collections::HashSet::new();
            for (i, &pc) in s.iter().enumerate() {
                let f = func(if i == 0 { pc } else { pc.saturating_sub(1) });
                if seen.insert(f.clone()) {
                    *incl.entry(f).or_default() += 1;
                }
            }
        }
        // Lines in the program's own code (LINES=part of a function name):
        // the innermost frame inside a function whose name has it.
        let want = std::env::var("LINES").unwrap_or_default();
        let mut lines: HashMap<String, u64> = HashMap::new();
        if !want.is_empty() {
            for s in st {
                for (i, &pc) in s.iter().enumerate() {
                    let pc = if i == 0 { pc } else { pc.saturating_sub(1) };
                    if func(pc).contains(&want) {
                        let mut l: IMAGEHLP_LINE64 = unsafe { std::mem::zeroed() };
                        l.SizeOfStruct = std::mem::size_of::<IMAGEHLP_LINE64>() as u32;
                        let mut d = 0u32;
                        let at = unsafe {
                            if SymGetLineFromAddr64(proc_, pc, &mut d, &mut l) != 0 && !l.FileName.is_null() {
                                let f = std::ffi::CStr::from_ptr(l.FileName as *const i8).to_string_lossy().to_string();
                                format!("{}:{}", f.rsplit(['/', std::path::MAIN_SEPARATOR]).next().unwrap_or(""), l.LineNumber)
                            } else {
                                "?".into()
                            }
                        };
                        *lines.entry(at).or_default() += 1;
                        break;
                    }
                }
            }
        }
        for (title, m) in [("inclusive", &incl), ("self", &selff), ("lines", &lines)] {
            let mut v: Vec<(&String, &u64)> = m.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            println!("  -- {title} (share of its busy samples)");
            for (k, c) in v.iter().take(top) {
                let k = if k.len() > 140 { &k[..140] } else { k.as_str() };
                println!("    {:5.1}%  {k}", **c as f64 / *nb as f64 * 100.0);
            }
        }
    }
}
