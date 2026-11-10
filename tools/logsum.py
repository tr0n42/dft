#!/usr/bin/env python3
"""Summarize a game performance log (logs/game-*.jsonl).

    python3 tools/logsum.py [log file]        (default: the newest log)
    python3 tools/logsum.py --all             (one line per log)
    python3 tools/logsum.py --suite [N]       (the last N benchmark runs, a
                                               table per run, a row per camera shot)

Shows the machine and body, then the run in phases (split wherever the
speed, the economy setting or the body changed, and where the swarm finished
growing), each with frame times, what the main thread spent them on, and what
the simulation cost and where. All times are milliseconds unless marked.
"""
import json, os, sys, glob, statistics as st

HERE = os.path.dirname(os.path.abspath(__file__))
LOGS = os.path.join(HERE, "..", "logs")


def load(path):
    rows = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                rows.append({"kind": "broken", "text": line[:200]})
    return rows


def mean(v):
    v = [x for x in v if x is not None]
    return st.fmean(v) if v else 0.0


def fmt(v, d=1):
    return f"{v:.{d}f}"


def phases(rows):
    """Split the samples where the run's setting changed."""
    out, cur, label = [], [], "start"
    for r in rows:
        k = r.get("kind")
        if k == "event" and r["what"] == "camera" and r["shot"] == "end":
            continue
        if k == "event" and r["what"] in ("speed", "economy", "new body", "loaded", "swarm grown", "camera"):
            if cur:
                out.append((label, cur))
            cur = []
            if r["what"] == "speed":
                label = f"speed {r['speed']}{' (paused)' if r['paused'] else ''}"
            elif r["what"] == "economy":
                label = f"economy {'off, ' + str(r['free_drones']) + ' drones' if r['free_drones'] else 'on'}"
            elif r["what"] == "new body":
                label = f"new body {r['body']} {r['radius_km']} km (built in {r['build_s']:.1f} s)"
            elif r["what"] == "camera":
                label = f"camera {r['shot']} ({r['secs']} s)"
            else:
                label = f"swarm grown to {r['drones']}"
        elif k == "sample":
            cur.append(r)
    if cur:
        out.append((label, cur))
    return out


def summary(path):
    rows = load(path)
    start = next((r for r in rows if r.get("kind") == "start"), {})
    names = start.get("names", {})
    parts, prof, counts = names.get("parts", []), names.get("sim_prof", []), names.get("sim_counts", [])
    print(f"== {os.path.basename(path)}")
    print(f"machine: {start.get('cpu')} ({start.get('cores')} cores, {start.get('threads')} threads, {start.get('memory')}), {start.get('os')}")
    print(f"gpu:     {start.get('gpu')} via {start.get('gpu_backend')}")
    print(f"body:    {start.get('name')} {start.get('radius_km')} km, seed {start.get('seed')}, glamour {start.get('glamour')}; "
          f"economy {'off, grow to ' + str(start.get('free_drones')) if start.get('free_drones') else 'on'}; speed {start.get('speed')}; window {start.get('window')}")
    for r in rows:
        if r.get("kind") == "panic":
            print(f"!! PANIC in thread {r['thread']} at {r['at']}: {r['message']}")
        if r.get("kind") == "broken":
            print(f"!! unreadable line: {r['text']}")
    for label, s in phases(rows):
        if not s:
            continue
        sims = [x["sim"] for x in s]
        print(f"\n-- {label}: {len(s)} s real, game {fmt(sims[0]['game_s'], 0)} -> {fmt(sims[-1]['game_s'], 0)} s, "
              f"drones {sims[0]['drones']} -> {sims[-1]['drones']}")
        fps = mean([x["fps"] for x in s])
        fa = mean([x["frame_ms"]["avg"] for x in s])
        f95 = mean([x["frame_ms"]["p95"] for x in s])
        fmax = max(x["frame_ms"]["max"] for x in s)
        ma = mean([x["main_ms"]["avg"] for x in s])
        print(f"   frames: {fmt(fps)} fps, avg {fmt(fa)}, p95 {fmt(f95)}, worst {fmt(fmax)}; main update avg {fmt(ma)} "
              f"-> {'mostly main thread' if ma > 0.6 * fa else 'mostly drawing/GPU (render side)'}")
        pm = [mean([x["parts_ms"][i] for x in s]) for i in range(len(parts))]
        pmax = [max(x["parts_max_ms"][i] for x in s) for i in range(len(parts))]
        print("   main thread per frame: " + ", ".join(f"{parts[i]} {fmt(pm[i], 2)} (worst {fmt(pmax[i], 1)})" for i in range(len(parts)) if pmax[i] >= 0.05))
        if any('hole_frames' in x for x in s):
            hf = sum(x.get('hole_frames', 0) for x in s)
            fr = sum(x.get('frames', 0) for x in s)
            print(f"   blank holes on screen: in {hf} of {fr} frames, at most {max(x.get('holes_max', 0) for x in s)} tiles at once")
        print(f"   drawn: up to {max(x['quads'] for x in s)} drone/hub squares, {max(x['tiles_shown'] for x in s)} tiles; "
              f"tile builds {sum(x['tiles']['worker_builds'] for x in s)} ({fmt(mean([x['tiles']['worker_ms_each'] for x in s if x['tiles']['worker_builds']]), 2)} each), "
              f"rebuilds {sum(x['tiles']['rebuilds'] for x in s)}, live reads {sum(x['tiles']['live_reads'] for x in s)} "
              f"(waited {fmt(sum(x['tiles']['wait_ms'] for x in s))}, held {fmt(sum(x['tiles']['hold_ms'] for x in s))} in all)")
        ach = mean([x["achieved"] for x in sims])
        cost = mean([x["cost_ms"] for x in sims])
        print(f"   sim: asked {sims[-1]['speed']}, achieved {fmt(ach)}x; cost {fmt(cost)} per game second "
              f"({fmt(1000 / cost if cost else 0, 0)}x real time on one core)")
        pr = [mean([x["prof_ms"][i] for x in sims]) for i in range(len(prof))]
        print("   sim cost per game second: " + ", ".join(f"{prof[i]} {fmt(pr[i])}" for i in range(len(prof))))
        cn = [mean([x["counts"][i] for x in sims]) for i in range(len(counts))]
        print("   per game second: " + ", ".join(f"{counts[i]} {fmt(cn[i], 0)}" for i in range(len(counts) - 1)) + f"; {counts[-1]} {fmt(cn[-1], 0)}")
        if cn[1] > 0:
            print(f"      each search: {fmt(pr[4] / cn[1] * 1000, 1)} us, {fmt(cn[3] / cn[1], 0)} cells weighed; {fmt(100 * cn[2] / cn[1], 0)}% find nothing")
        th = [x["sim_thread"] for x in s]
        print(f"   sim thread per real second: held the world {fmt(mean([t['hold_ms'] for t in th]), 0)}, waited for it {fmt(mean([t['wait_ms'] for t in th]), 1)}, "
              f"slept {fmt(mean([t['sleep_ms'] for t in th]), 0)}; publishing {fmt(mean([t['publish_ms_avg'] for t in th]), 2)} each (worst {fmt(max(t['publish_ms_max'] for t in th), 1)})")
        print(f"   swarm: frontier {sims[-1]['frontier']}, awake cells {sims[-1]['awake']}, waiting out of sight {sims[-1].get('dormant', 0)}, loose bodies {sims[-1]['loose']}, eaten {fmt(100 * sims[-1]['eaten'], 3)}%; "
              f"memory {fmt(max(x['process']['mem_gib'] for x in s), 2)} GiB, process CPU {fmt(mean([x['process']['cpu_pct'] for x in s]), 0)}%")
    slow = [r for r in rows if r.get("kind") == "event" and r["what"] == "slow frame"]
    others = [r for r in rows if r.get("kind") == "event" and r["what"] not in ("slow frame",)]
    if others:
        print("\nevents: " + "; ".join(f"{fmt(r['t'], 0)} s {r['what']}" + (f" ({r.get('speed', '')}{', paused' if r.get('paused') else ''})" if r['what'] == 'speed' else "") for r in others))
    if slow:
        worst = sorted(slow, key=lambda r: -r["frame_ms"])[:3]
        print(f"slow frames (>50 ms) logged: {len(slow)}; worst: " + "; ".join(
            f"{fmt(r['frame_ms'], 0)} ms at {fmt(r['t'], 0)} s (main {fmt(r['main_ms'], 0)}: " + ", ".join(f"{parts[i]} {fmt(v, 1)}" for i, v in enumerate(r['parts_ms']) if v >= 0.5) + ")" for r in worst))
    end = next((r for r in rows if r.get("kind") == "end"), None)
    if end:
        print(f"end: {fmt(end['real_s'], 0)} s, {end['frames']} frames, {end['slow_frames']} slow, peak {end['peak_drones']} drones, peak memory {fmt(end['peak_mem_gib'], 2)} GiB")
    else:
        print("end: (no end line: the game was closed forcibly, crashed, or is still running)")


def suite(files):
    """One table per benchmark run: a row per camera shot."""
    for p in files:
        rows = load(p)
        start = next((r for r in rows if r.get("kind") == "start"), {})
        if not any(r.get("kind") == "event" and r.get("what") == "camera" for r in rows):
            continue
        print(f"\n== {os.path.basename(p)}: {start.get('name')} {start.get('radius_km')} km, "
              f"{start.get('free_drones') or 'economy'} drones, speed {start.get('speed')}"
              f"{'' if any(r.get('kind') == 'end' for r in rows) else '  (NO END LINE)'}")
        for r in rows:
            if r.get("kind") == "panic":
                print(f"   !! PANIC {r['thread']} {r['at']}: {r['message']}")
        print(f"   {'shot':<6} {'fps':>5} {'p95ms':>6} {'worst':>6} {'main':>5} {'drw':>5} {'tiles/s':>7} {'x real':>7} "
              f"{'ms/gs':>6} {'drn%':>4} {'snd%':>4} {'rck%':>4} {'drones':>7} {'awake':>7} {'waiting':>8} {'GiB':>5} {'flk':>4} {'hlr':>4}")
        cur, label = [], None
        def flush():
            if not cur or label is None:
                return
            sims = [x["sim"] for x in cur]
            prof = [mean([x["prof_ms"][i] for x in sims]) for i in range(4)]
            tot = sum(prof) or 1
            print(f"   {label:<6} {mean([x['fps'] for x in cur]):>5.0f} {mean([x['frame_ms']['p95'] for x in cur]):>6.1f} "
                  f"{max(x['frame_ms']['max'] for x in cur):>6.0f} {mean([x['main_ms']['avg'] for x in cur]):>5.1f} "
                  f"{mean([x['parts_ms'][4] for x in cur]):>5.1f} {mean([x['tiles']['worker_builds'] for x in cur]):>7.0f} "
                  f"{mean([x['achieved'] for x in sims]):>7.1f} {mean([x['cost_ms'] for x in sims]):>6.1f} "
                  f"{100*prof[0]/tot:>4.0f} {100*prof[1]/tot:>4.0f} {100*prof[2]/tot:>4.0f} {sims[-1]['drones']:>7} "
                  f"{sims[-1]['awake']:>7} {sims[-1].get('dormant', 0):>8} {max(x['process']['mem_gib'] for x in cur):>5.2f} "
                  f"{sims[-1].get('flocks', 0):>4} {sims[-1].get('haulers', 0):>4}")
        for r in rows:
            if r.get("kind") == "event" and r.get("what") == "camera":
                flush()
                cur, label = [], (r["shot"] if r["shot"] != "end" else None)
            elif r.get("kind") == "sample" and label:
                cur.append(r)
        flush()
        for r in rows:
            if r.get("kind") != "event":
                continue
            w = r.get("what")
            if w == "save begun":
                print(f"   save: held the simulation {fmt(r['hold_s'] * 1000, 0)} ms copying {fmt(r['raw_mb'], 1)} MB")
            elif w == "saved":
                print(f"   saved {r['file']}: {fmt(r['file_mb'], 2)} MB on disk, {fmt(r['write_s'], 2)} s compressing and writing")
            elif w == "loaded":
                print(f"   loaded in {fmt(r['build_s'], 2)} s: game time {fmt(r['game_s'], 0)} s, {r['drones']} drones")
            elif w in ("save failed", "switch failed"):
                print(f"   !! {w}: {r.get('error')}")


def main():
    args = sys.argv[1:]
    files = sorted(glob.glob(os.path.join(LOGS, "game-*.jsonl")))
    if args and args[0] == "--suite":
        n = int(args[1]) if len(args) > 1 else len(files)
        suite(files[-n:])
        return
    if args and args[0] == "--all":
        for p in files:
            rows = load(p)
            s = next((r for r in rows if r.get("kind") == "start"), {})
            n = sum(1 for r in rows if r.get("kind") == "sample")
            print(f"{os.path.basename(p)}  {s.get('name')} {s.get('radius_km')} km  {n} s  economy {'off ' + str(s.get('free_drones')) if s.get('free_drones') else 'on'}")
        return
    path = args[0] if args else (files[-1] if files else None)
    if not path:
        print("no logs yet")
        return
    summary(path)


if __name__ == "__main__":
    main()
