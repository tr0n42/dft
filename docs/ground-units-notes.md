# Ground units: implementation notes (run of 25-26 November 2026)

A working journal for the ground-units run: what was found in the code, what
was decided and why, what went wrong and how it was fixed. If the run has to
be redone from the restore point (git tag `pre-ground-units`), read this
first. The agreed behaviour is `docs/design/ground-units.md`.

## Starting point (what the code had)

- `src/units/platform.rs`: the specialist director (`order_specialists`)
  ordered every specialist as a fixed `keep` count from economy.toml (sappers
  only with a charge to hand; tunnelers one per 1,500 cutters up to 2; heat
  sinks up to the open magma pools found near flocks; walkers while there were
  plants / animals). Everything specialists do steps once a game second
  (`specialists_work`).
- Sapper (`src/units/sapper.rs`): a faithful port of the proving ground blast
  (rays, cracks, held pieces, melt, thrown rock, plume, chain blasts) with an
  off-screen shortcut (no throwing, heart straight to glass). Depth was
  "blast radius + 50 m" (24 m + 50 m at 5 kt), drill limit 25 m. Ordered as
  `keep = 3`; went to the flock with the worst dig seconds per kg. Blasts hurt
  no drone.
- Tunneler (`src/units/tunneler.rs`): the old Tunneler's 12 m shaft borer,
  started under a random working flock, steering toward "ore smell" within
  the lean limit; quit at lava or 600 m; credited only `spoil_recovery = 0.5`
  of what it dug (mass lost); left shaft hubs every 150 m.
- Heat sink (`src/units/heatsink.rs`): flew to an open surface pool near a
  flock, froze 4 cells a second nearest its spot (downward first) out to 30 m,
  beamed power, burned out after 900 s.
- Forester (`src/units/forester.rs`): walkers drop from the mother swarm,
  stride to stands of plants, shear sections into bags, laser-light the
  ground cover (the sandbox's burning then spreads it on screen), leap cliffs,
  drop full bags for couriers. Abductor (`abductor.rs`) walks on the same
  code and hunts placeholder animals (`src/fauna.rs`).
- Game drawing (`game/src/units.rs`): forester redrawn on 12 Nov as a
  sideways four-legged crab on FABRIK legs (`rig.rs`), not the mockup.

## Plan

1. Sapper: depth from yield; loosened chimney up to the surface; need-driven
   ordering with a rate cap; blasts kill cutters and damage haulers; far
   sectors done by math; keep clear of tunnelers; plume pixel threshold.
2. Tunneler: rewritten as the opener (value targeting with fog sight, curved
   shafts, magma caps, hard strata, spoil plume and crew, drill heat,
   self-destruct).
3. Heat sink: two roles (small pools, deep passages under caps), its crew,
   dies idle.
4. Forester and abductor: flight past the swarm edge, death-by-distance,
   off-screen state, fire spread off screen, ranged-attack fauna, gore;
   visuals ported from the mockup.
5. Couriers: check they match (visual only, bags and sacs).
6. Testbeds, benches, the v2 mockup page, handoff.

## Journal

### Sapper (25 Nov, evening)

- **Depth from yield**: `depth_m_per_cbrt_kt = 46` (79 m at 5 kt). The old
  rays break only about the blast radius (24 m at 5 kt), so a new
  **loosened column** (`sapper_column`) turns every solid cell from the charge
  up to the surface into loose rubble (hard rock now and then held in big
  cracked pieces): blast-wide at the bottom, `loosen_top = 1.6` radii each
  side at the top. About 5,000 cells at 5 kt. The plume then finds its way up
  through the column almost every time (on screen only).
- **Made to order**: `specialist_want` (platform.rs) replaces the fixed keep
  count. A flock is *struggling* when it has cut at least 3 loads lately and
  its seconds per kg are `stall_ratio = 3` times those of hardness-3 rock
  (`flock_stall`). A sapper is ordered only while one is struggling, at most
  one every `order_gap_s = 90`, at most `keep = 3` out.
  - Found: on the Dev planet nothing struggles for 15+ min, because cutters
    pick the cheapest cells and there are 120 m of sediments over the granite.
    That's right; a testbed with hard rock near the top was needed:
    `data/bodies/test-sappers.toml` (3 km, 8 m of soft rock over granite and
    gabbro). There flocks hit 4.1x at about 4 min and sappers follow every 90 s.
- **Bug: sappers went off at the surface** ("set off by being destroyed"):
  their own drill spoil fell back on them and counted as burial. Fixed:
  burial never destroys a sapper (a charge doesn't mind being sealed in; one
  buried while crawling drills on); only magma or a blast's heat sets one off
  early.
- **Bug: the column came out short** (50 m, once 1 m): the surface was found
  by looking straight down the radius, i.e. down the sapper's own open shaft.
  Now looked for either side of the shaft (`ground_at` at +-0.5r, +-r).
- **Harm** (`blast_harm`): cutters in the column (plus `kill_margin_m = 4`)
  are wrecked (`wreck_drone`, scrap kept); haulers within `hauler_harm_r = 3`
  radii lose life (dead within the first third).
  - Bug: off screen almost nothing died, as off-screen cutters do whole trips
    at once and sit at their hauler. Now a cutter is caught if where it is,
    where its trip goes or comes from, or where it last cut is in the column.
    Off screen about 140-190 cutters a blast on the testbed, like on screen.
- **Tunnelers**: `tunneler_in_blast`; an armed sapper with a tunneler in reach
  drills sideways away (up to 200 m), and failing that waits armed.
- Cost on the testbed (3 km, 20k cutters): sappers 0.3-2 ms per game second;
  a blast's drone sweep is one pass over all drones (rare, so cheap).
- Far sectors (out of sight, big swarms): `far_stalled` finds far sectors
  whose top ground cuts `stall_ratio` slower than soft rock;
  `far_sapper_blast` books the column (depth x r x (1 + loosen_top) cells)
  as `far.loose_kg`, eaten first at the loose pace in `far_plan`, and kills
  the share of the sector's cutters where it breaks the surface. Near
  struggling flocks are served first.
- Plumes (game/src/fx.rs): a plume about 126 m x sqrt(power) high is drawn
  only when that is 20+ pixels on screen; zoomed out its dust comes in blocks
  of about two pixels, so big blasts read from far out.

### Tunneler: the opener (25-26 Nov, night)

New file `src/units/opener.rs` (the old Tunneler's `Bore` stays in
tunneler.rs for the old perk and old saves).
- **Value** (`opener_values`, every 60 s): per material, `space_weight` x
  (multiplier - 1) + `need_weight` x sum over its elements of how short the
  stockpile is of what cutters' and haulers' parts use (demand / (demand +
  stock)).
- **Targets** (`opener_target`): first, columns every 96 m across the mother
  swarm's reach and 3 km past it, down to 2 km, sampled every 24 m (it sees
  through the fog); score = worth / (1 + depth / 300 m). A magma layer met on
  the way down scores `magma_score`; hard rock under struggling flocks
  `hard_score`. From its head: a cone within the lean, 400 m on down.
  Nothing behind a capped breach until a passage is through.
- **Steering**: turns at most 0.6 degrees a metre toward the target, never
  past 20 degrees off straight down: curved shafts.
- **Crew**: all its spoil goes to its crew (mass kept whole, credited as
  cleared); it waits while more than `crew_backlog_t` is waiting.
  - Found: with a crew of 30 (3.6 t/s) the drill (130 t/s possible) spent the
    run waiting: 156 m in 15 min. Crew 200 x 150 kg/s = 30 t/s; then the
    drill was the limit (10 m a minute in granite); drill_power 100 -> 400 =
    0.8 m/s in granite, matching the crew. Now about 450 m in 10 minutes.
- **Magma**: magma within its drill's reach -> `opener_cap`: the magma
  within cap_m frozen to stone (glass look; mass kept), a `HeatJob`
  (with a copy of its shaft's line) for the heat sinks.
  - Found: right after capping it found nothing further down (all behind the
    cap) and blew itself up, wasting the unit and risking the cap. Now it
    backs up to its mouth and searches the whole reach again (a new shaft);
    only with nothing anywhere does it sink back down its shaft (stopping
    80 m short of the bottom) and go off (`unit_blast`, 50 kt, no column).
- **Depth readouts**: `ground_beside` (the surface either side; straight
  down a shaft finds its bottom), used by sappers and openers.
- Cost on the testbed (3 km, 20k cutters, 2 openers): 0.2-0.4 ms per game s.

### Heat sink (26 Nov, small hours)

`src/units/heatsink.rs` rewritten around the two roles.
- `scan_molten` (every 30 s near flocks) sorts open pools: **small** (at most
  1,500 cells, all within its 30 m reach: `pool_small`, a flood fill) are
  pools to cool; **wide** ones under open space become deep jobs of their own
  (a `HeatJob` whose "shaft" is the open space over it), 40 m apart, as many
  bots as fit.
- **Director**: as many as there is work for (pools no bot is on + deep jobs
  untaken), at most `keep`. Deep jobs first.
- **Freezing**: `freeze_per_s = 40` cells' worth a second; a cell costs 1 at
  its freezing point, up to 4 when hotter (`freeze_cost`; Benjamin: heat only
  sets how long). No power beamed.
- **Deep job** (`bot_passage`): down the shaft's line to the cap; the front
  freezes a slab across the passage (a tunneler's 12 m + `wall_m = 3` each
  side) a metre at a time; its crew (`crew = 60` x 150 kg/s, simulated by
  the unit, drawn by the game) digs out only the middle 12 m behind it (the
  walls stay). Through when the front has gone `CLEAR_M = 6` m with no hot
  liquid, past the cap, and the crew has caught up; the job's shaft is
  extended to the passage's bottom and openers launch down it (`Descend`)
  to bore on.
  - Found: "through" after 4-6 m (the tunneler's cap had already frozen the
    first metres: no hot liquid there). Clear metres now count only past the
    cap.
  - Found: passages of 400 m through a 90 m layer. The front kept running
    through the solid ground under the layer while waiting for the crew.
    Now it stops once clear and the crew catches up: passages 119-135 m.
  - Found: 5 bots wrecked by magma: slanting passages left gaps in their
    walls (slab sampled every 0.7 m on one row). Now two rows a metre, 0.5 m
    apart: none wrecked since.
  - Found: a tunneler capped the same breach 17 times, 3 m deeper each time
    (the passage wasn't really through). A cap within 40 m of a known job
    now reopens that job.
  - Testbed: the lower mantle was 1,700 K, so disturbed peridotite melted:
    an endless "layer". Cooled to 1,000 K (`test-tunnelers.toml`).
- **End**: idle 10 s with no next job: it powers down, left as scrap
  (`bodies.wreck`) for cutters; magma reaching its body wrecks it.
- Cost on the testbed: tunnelers + heat sinks under 1 ms per game second.

### Forester and abductor (26 Nov, early morning)

- **Visuals** were handed to a helper (in a worktree, merged as 241b8d7):
  the mockup's forward-facing three-legged crab, bezier legs with high knees,
  the stride cycle with sag/jolt/surge; FABRIK legs removed from rig.rs.
- **Flight** (`WalkState::Fly`): launched over the best target anywhere on
  the body (the thickest stand of plants; for abductors the densest fauna,
  counting animals within 200 m), flying folded at `fly_speed = 40` m/s,
  `fly_alt_m = 120` over the ground, then dropping. The game draws it as
  the package with a wobble and a ring of lights (view state 0, c = 3).
- **Death by distance** (`walker_go_or_retire`): with nothing in reach, the
  next place's walk time (`stride_speed`) against build time (its bay's
  `build_s`) + a new one's flight from the mother swarm + 8 s; if walking is
  slower, it retires: its bag and its body (as drone scrap, the unit's parts'
  mass, `unit_mass`) are dropped for the couriers. Seen: 2-3 retirements in
  15 minutes on the Dev planet, each replaced.
- **Fire out of sight** (`Fire`, `fires_work`): cells a forester's laser
  lights where the sandbox doesn't see become a fire front burned as math,
  30 cells a second, spreading to the plant cells round each (8 neighbours),
  mass lost to the sky as the sandbox's fires; in view it becomes real fire.
  Seen: about 5,000 plant cells burned out of sight in 15 minutes.
- **Abductor**:
  - Gore: no blood or flesh materials exist (adding materials would change
    every body's fingerprint), so a lasered animal's mass is credited as
    biomass at once (FLESH make-up) and the game draws a red burst (sparks,
    red dust). `live_multiplier` 2 -> 1.
  - Ranged animals (`ranged_share = 0.15` on the Dev planet): spit every
    6 s at machines within 60 m: a walker loses 5 hp (abductor 100,000), else
    a flock within reach loses a cutter.
  - Found: it lasered every runner and caught nothing alive (animals panic
    the moment it arrives). Now: spitters first (a shot per 2 s), then
    palps for anything in reach, runners out of reach only every 3 s.
    About 70-90 caught alive and 240-290 burst in 15 min.
- **Herds out of sight**: calm animals unseen and over 400 m from any walker
  step every 10 s (10 s worth at once).

### The lava world (26 Nov, early morning)

The old specialist tests on the lava world (3 km, magma ocean 75 m under an
obsidian skin) went from minutes to over 50 minutes.
- Found: the heat simulation ("sand and heat") went from about 5 to 300-424
  ms per game second once tunnelers or heat sinks opened the magma ocean:
  a hole through the skin floods, and a passage's walls woken beside 1,900 K
  magma melt back. Tests (and `unitlab` by default) treat the whole body as
  on screen, the worst case.
- Fixes: frozen cells are chilled to 300 K (`CHILL_K`) and left at rest, not
  woken. The charge tests (fuser, conventional) build only sappers now.
- Measured: off screen everything stays under 12 ms; with the camera
  following the units at 1 m a pixel (`SEEN=follow` in unitlab, a screen of
  1,920 x 1,080 m) the heat simulation is 1-43 ms per game second. The
  whole-planet-in-detail case stays expensive: that is the sandbox's own
  liquid cost when a magma ocean floods (cutters breaking the skin do the
  same, less often).
- Heat sinks on the lava world: wrecked when the pools rose over their job
  spots. Now a job spot is found by walking up to open air, bots stop 6 m
  short of it, never move into magma, give up a job whose spot has filled
  ("stale jobs"), and a job that wrecks two bots is dropped. 15 wrecks in
  20 minutes down to 8; the rest are magma rising onto a bot at work.

### Flight paths and the 500k benchmark (26 Nov, morning)

- Found (code review): flying units moved in a straight line, so a tunneler
  flying to a new shaft far along the surface, or a walker flying to the far
  side, would cut through the rock. `fly_over` (platform.rs): up to a
  cruising height over the higher ground, round the body, then in.
- Benchmark (bench-units.bat: 500,000 cutters, living world 35 km, 20 game
  min; clean baseline from the tag on the same PC, run alone):
  - Stage 1 identical work (same kt mined, same cutters working); the
    fingerprint differs only because the units' hash now mixes heat jobs.
  - Stage 2: no sapper ordered (no flock struggles on the living world in
    20 min): made to order, as designed.
  - Found: tunnelers cost 23 ms per game second (baseline 0.2). The look
    through the fog sampled up to 256 columns x 83 depths x 5 cells every
    30 s, and on a 35 km body most of that ground had never been built, so
    each look built it. Now: scanned only while fewer than `keep` are out,
    at most 96 columns, every 32 m, 3 cells. Under 0.5 ms; stage 3 280 s
    real against the baseline's 278.
  - Heat sinks 1.4 ms (baseline 2.7), foresters 0.65 (1.25), sappers 0.15.

### The Watch panel and cutters through the planet (26 Nov, afternoon)

Benjamin asked for a button that makes the right specialist at once and
follows it, and reported cutters flying through the planet, clearing great
swathes of fog.

**Watch panel** (`game/src/watch.rs`): a button per specialist, bottom left.
`Control::summon` asks the sim thread; `Sim::summon` (platform.rs) launches
through the normal launch code, free (`Units::summoning`: a sapper casts its
own charge rather than taking a stocked one; no parts, no queue), and
answers with the id or why not (no magma, no plants...) in `Out::summoned`.
The camera follows (`camera::Follow`, ahead of the director): a 1.5 s glide
in position and log zoom to 0.25 m/px, then locked on, at the drones' own
clock (`advance_clock`, moved to PreUpdate so the camera and the drawing see
the same moment). Drag, keys, Home or middle click let go; the wheel zooms
about the unit. A unit gone (blew up, retired) holds 3 s, then lets go.
Pitfall: `UnitView.lift` is a walker's height but a tunneler's or heat
sink's crew size; the view is raised by it only for walkers.
`--summon <unit>` presses the button 30 game s in (for screenshots).

**Cutters through rock** (`Leg::drawn_at`, swarm.rs; measured with
`src/bin/legcheck.rs`, which counts metres of rock along every flying leg):
not new (14 Nov drop-off points onward), just seen now. On the Dev planet,
20k cutters, at any moment 100-1,500 legs were drawn straight through rock,
worst 5.3 km:
- drops from the mother swarm to a flock round the body (its nearer end):
  kilometres; now up, round and down as a hauler flies, in the same time;
- legs to and from drop-off points (40-100 m from the flock's hub, 60 m+ up,
  above the field's 40 m of swept sky): the game only looked for a field
  within 8 m of a leg's end, so they went straight. Now each leg names its
  flock (`Leg::flock`); each end comes into the field (a drop-off point
  straight down into it); the way runs down from one end to where the two
  ways toward the hub meet (`FlowField::meet_m`) and up to the other;
- ends no field takes (a far flock, work past the field's edge): out the
  way its field goes, over the top, and in.
After: 10-50 legs at a time, worst ~160 m (stale fields: rock settling
into dug ground in the 15 s between sweeps; field approximations near the
surface; a cutter deep in a cave with no field). Costs: each square's way
down is stored once (`FlowField::down`), ~0.75 us a flying drone. The fog
charts the full way near the screen zoomed in, else `Leg::charted_at`
(drops over the top, else the nearer end: never deep rock); zoomed out the
drawing uses `drawn_at_coarse` (short flights straight). The sim's own
results don't change (only drawing uses these).
