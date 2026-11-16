# The Dark Forest Theory: handoff for a new session

Read this first, then ROADMAP.md (what's built, where we are, what's next,
and a map of every document) and README.md (how the code is laid out). Last
updated 13 November 2026, on BKONEN-R16-W11.

## Working with Benjamin

- He is not a programmer: Claude makes every technical decision and writes
  all the code. He tests on his Windows PC and reports what he sees (often
  with videos or screenshots).
- He runs things by double-clicking `.bat` files, never from a command
  line: every test or run needs its own `.bat` with no arguments, showing
  live progress, not popping over his other work.
- He wants honest numbers, frequent short progress messages during long
  work, and plain words.
- The project is a git repo pushed to https://github.com/tr0n42/dft
  (branch `main`). He works on more than one PC; on each it is cloned in
  `Downloads\DarkForestTheory-source` and the work happens in Claude Code
  there (see CLAUDE.md). Commit and push only when he asks.

## Picking up on another PC

1. **Pull first:** `git pull` in the project folder. If the other PC has
   work not yet pushed, push it there first (as of 13 Nov the second PC's
   forester and sapper hull work and its 8:00 sim runs were not on GitHub).
2. **Tools** (installed on the first PC on 11 Nov): Rust stable (MSVC),
   the VS 2022 Build Tools (C++ workload), Python 3.12 for tools/*.py, git,
   and the GitHub CLI. If `git push` asks for a username, run
   `gh auth login` once, then `gh auth setup-git`.
3. **First builds are slow** (fat LTO, one codegen unit): `cargo build
   --release` at the root, `cd game && cargo build --release
   --ignore-rust-version` for the game. `cargo check --release` for quick
   compile checks.
4. **Check it plays the same:** `cargo run --release --bin samecheck`
   (about 4 minutes) prints a fingerprint per fixed game; compare with the
   lines in "The unit platform" below. They should match on any PC with
   the same commit (if not, that's a finding worth reporting).
5. **Benchmarks are per machine:** perf/history.csv keeps each PC's rows
   apart (by computer name); the first `bench-units.bat` on a new PC
   records its own baseline. Compare impacts on one PC, not across PCs.
6. **Claude's memory is per PC** (it lives in the user profile, not in
   git). The standing rules it holds are copied here so a fresh PC has
   them:
   - Art: everything in 1 m blocks, no smooth circles or gradients; one
     lighting engine (the mockup's emit/lightPass). Every machine is
     biomechanical (carapace, ribs, sinew, tendrils, eyes; no long rigid
     bars). One parts library classed by matter, picked by the planet;
     body-horror grafts switchable; life parts only where there is life.
     (See "Machine art" below.)
   - After publishing a claude.ai page (e.g. the foundry mockup), open it
     for Benjamin with the Artifact tool: he can't find links in chat.
   - Off screen can almost always be approximated: the sim must keep going,
     at least to the eye.
   - Performance: "we need performance impacts more than performance now".
     Measure every change per unit (bench-units.bat), no optimisation that
     isn't essential, each unit's code in its own file under src/units/.
   - The Dev planet must exercise every drone: each new drone's work is
     added to data/bodies/dev-planet.toml.
   - Life: plants first (L1e built, L1f panic next), then fauna; no
     planet-wide doom meter, each species reacts only to its own losses,
     most don't react; exodus rare; defences later; live captures worth
     more than cooked, each recorded for a menagerie.

## How the work is done

- Build and test directly on his PC, in the project folder. (Before Claude
  Code, the work was built in a cloud workspace and copied over with md5
  checks. That routine is retired.)
- Sim crate (repo root): `cargo build --release`, tests with
  `cargo test --release --no-fail-fast` (about 25 minutes; run it in the
  background). The game crate is `game/`:
  `cargo build --release --ignore-rust-version`.
- Generator changes change every body's fingerprint: re-lock with
  `BLESS=1 cargo test --release --test reference` once the change is meant.
- The game opens a maximized window by default; pass
  `--windowed` for a 1600 x 900 window. The stockpile window's collapse arrow is at
  about (1365, 28) on a 1600 x 900 screen.
- The test suite and the abundance tool both use a lot of memory, so don't
  run them at the same time.

## Design documents

ROADMAP.md opens with a map of every document (docs, mockups, pages, data).
In short:

Snapshots are in `docs/design/` (pulled 10 November 2026; images and
diagrams don't export). The originals are claude.ai docs in the
DarkForestTheory project, opened by link:

- **Swarm Production Design**: production in the swarm, robot-lore parts,
  factories, the director's hidden queue, energy, heat-sink bots and
  tunnelers, the magma barrier, build phases, between-run carry-over.
  https://claude.ai/artifact/3LKCygqRGZSwUVzDd1CktB
- **Element Abundance Master Table**: all 75 elements against Earth's
  crust, every body's specialties, the specialty catalogue and odds,
  rarity-priced drone parts, the meta-tree bias.
  https://claude.ai/artifact/2yb9kfVeL86XptSLn7KX7Y
- Older: "Life L1 - Procedural Flora Design" (`docs/design/flora-design.md`),
  https://claude.ai/artifact/Ss3ZUbdBKLF9Kk3dJbrS4o
- The master plan: "The Dark Forest Theory — Plan" (vision, architecture,
  milestone results M0 to M4, geology passes, decision log;
  `docs/design/game-design-plan.md`),
  https://claude.ai/artifact/4Wfryd7cBArfBR7cx6SkCr

## The unit platform and "500k cutters with X" (13 Nov)

Agreed with Benjamin 13 Nov: we need performance *impacts* more than
performance now. Every change is measured; no optimisation that isn't
essential; each unit's code lives in its own file so its cost shows on its
own.

- **Unit files** (`src/units/`, modules of `swarm`): `cutter.rs` (events,
  targeting, heat, wear), `hauler.rs`, `sapper.rs` (blast, rubble settling,
  charges), `tunneler.rs`, `heatsink.rs`, `forester.rs` (walkers, bags),
  `abductor.rs` (its hunt; it strides on the forester's code).
  `units/platform.rs` holds what they share (the `Units` list, `Skills`,
  the specialist director and once-a-second dispatch, ground helpers,
  views, saves, hashes). The tick, flocks, the cutter/hauler director and
  production stay in `swarm.rs`. New unit code goes in its unit's file;
  anything two units need goes in the platform.
- **The split changed nothing:** `samecheck` (src/bin/samecheck.rs) runs
  fixed games (every unit at work, on and off screen, through a save; the
  Dev planet added as the sixth) and prints fingerprints; identical before
  and after. Run it around any refactor that should not change play.
  Fingerprints at the end of 13 Nov (first column is the game):
  living world economy `f55405267d09fc16`, on screen `69415c042d6aa981`,
  lava world `2c106b7cc0ebf016`, 500k free `8e3e5151ba8f238e`, active
  planetoid `452ca9b91f3f602a`, dev planet `6a0bf4415c84448e`.
- **Per-unit timers:** `Sim::unit_prof`, named in `swarm::UNIT_NAMES`.
- **The workflow:** `bench-units.bat` (`src/bin/unitbench.rs`) runs 500,000
  cutters on the 35 km living world, zoomed out, 20 game min, for each stage
  up to `perf/stage.txt`: 1 cutters + haulers, 2 + sappers, 3 + tunnelers,
  4 + heat-sink bots, 5 + foresters and abductors. Each unit is designed
  to what Benjamin wants, then its stage turns on. Every run appends a row
  per stage to `perf/history.csv` (checked in: machine, commit,
  fingerprint, ms per game second for each unit and the platform's parts)
  and prints the change since the last run on that PC; the same
  fingerprint means only speed changed. Specialists in the bench are kept
  at the economy's `keep` counts for free (`Sim::free_units`), and the bays
  of units not in the stage are removed.
- **Stage 1 baseline (this PC):** 189 s real for 1,200 game s, ~200k
  cutters working at the end; per game second: cutters 90 ms, sand and
  heat 35, platform other 22, rock bodies 8, haulers 2, plant life 1.
- **Loose bolts noticed so far (not fixed):** cutter events get dearer as
  the swarm spreads (1.5 to 7 us each); sand and heat grows with the dug
  area; the heat-sink and forester work scans run whenever their bays
  exist; in stage 4 the bots found no open magma near the flocks of the
  living world (the Dev planet has plenty); drone drawing zoomed far out costs ~8 ms a frame at
  500k. A loaded save plays out a little differently from the same game
  left running (samecheck shows it; it was so before 13 Nov). On the Dev
  planet the heat-sink bots freeze nothing in the 5 minutes after a load
  (frozen stays at its saved value): their work state may not survive the
  save.
- **Fixed 13 Nov:** each drone event scanned every hauler (`hauler_at`)
  and every cut checked every hub (`work_near`); both indexed now (same
  results), 500k for 20 game min went from 251 s to 166 s.

### The Dev planet (13 Nov)

`data/bodies/dev-planet.toml` (`run-dev-planet.bat`, or pick "dev-planet" in
F1): a 5 km test world where every drone has work. **Benjamin's rule: every
new drone gets its work added here.** Thin sediments over granite and gneiss
(sappers), uranium shale and coal (charges), 40 small ore-ringed granite
stocks close under the ground (tunnelers), 60-80 open lava pools with no
crust (heat-sink bots), plant life (foresters), and a `[fauna]` section:
herds of placeholder animals (abductors). `unitcheck -- dev-planet` shows it
builds all five specialists; samecheck's sixth game ("dev planet, economy")
shows each at work.

- **Placeholder animals** (`src/fauna.rs`, a module of `swarm`): herds
  (3 per km of circumference, 4-9 animals of 40-400 kg) graze and drift;
  when a machine comes within 80 m (walker, sapper, bot, tunneler, working
  flock) or one of the herd is snatched, the whole herd flees at 7 m/s for
  25 s. They die if the ground is eaten 30 m from under them or turns to
  liquid. A new herd wanders in every 20 s while there are fewer than at
  the start, so they never run out. Saved (sim save version 14) and in the
  state hash only on bodies that have them. Timed on their own
  (`Sim::fauna_prof`, the bench's "animals" column). Drawn as block critters (`draw_animals` in
  game/src/units.rs) that trot, with a white tail up while fleeing.
- **Abductors** (`units/abductor.rs`): ordered while there are animals;
  stride to the nearest herd no other abductor is after, run down the
  nearest animal within 250 m at 10 m/s, snatch it within 12 m into a
  stasis sac, and drop full sacs (2 t) for the couriers. Live catches are
  credited at 2x their mass as flesh (`[units.abductor]` in economy.toml).
  Their hunt has its own timer (`U_ABDUCTOR`).
- **Cost on the Dev planet** (5 km, 20k free cutters, all five specialists,
  20 game min, this PC; ms per game second): cutters 17.9, animals 12.7
  (about 400 of them), sand and heat 8.6, platform other 3.1, rock bodies
  2.3, foresters 1.8, sappers 1.0, heat-sink bots 1.0, plant life 0.6,
  haulers 0.2, tunnelers 0.1, abductors under 0.05. **Loose bolt (not
  fixed):** each animal looks for the ground three times a second
  (`fauna_work`: `ground_at`, then `dry_ground` here and one step on; each
  walks down from 64 m over the surface), and the first two repeat each
  other.

## Where things stand (November 2026)

Most recent work (12 Nov), built and passing the tests, waiting on a PC look:

- **Sappers' charges (12 Nov, second session; test-fuser.bat).** Benjamin's
  answer to the uranium question:
  - **The fuser** (src/fuser.rs, `[fuser]` in data/economy.toml), a refinery
    module: fuses two atoms into one heavier one (atomic numbers add up,
    checked at load in a test; kg in = kg out; 250 E a kg, only from energy
    above the reserve plus `spare_energy`). It keeps 16 kg of refined
    uranium in stock. Any pair that makes uranium has a rare heavy partner,
    so short partners are fused first, one step down: Fe+Fe=Te, Ca+Ca=Zr,
    then Te+Zr=U (also Xe+Sr, Ba+Kr, Pb+Ne, Ta+K, Hf+Ca, with Fe+Ni=Xe,
    K+K/Si+Cr=Sr, Ca+S=Kr, N+Li/C+Be=Ne). It takes whichever route the
    stockpile can make the most of; in practice iron and calcium. 40 min on
    a 3 km lava world (no uranium): 72 sappers, 384 kg fused, 192k E.
  - **Conventional charges** (`[[units.sapper.charge]]`): only when no
    uranium can be had or fused, the sapper's bay casts a 5 kt charge
    straight from the stockpile (src/units/sapper.rs, `cast_charge`; the
    sapper's recipe then drops its nuclear charge): RDX (1.6x TNT, 3,125 t,
    needs ~1,180 t of nitrogen), else aluminium- or magnesium-oxygen slurry
    (counted at TNT strength, 5,000 t). Cast only while the stockpile holds
    as much again after (`charge_keep_back`). Same blast, same look.
  - **Uranium in rubble:** a 5 kt blast fissions ~0.3 kg (17 kt/kg); half
    the unreacted rest (`unreacted_doped`) is spread through the blast's
    rubble that stays put (`Units::dope`, moved as rubble settles); a
    cutter taking a doped cell carries its uranium up. Lost if the plume,
    a tunneler or a forester takes the cell.
  - F1: "sapper charges now", "uranium left in rubble", "fuser" lines.
    Save version 13 (12 still loads).

- **Plant life cycle (L1e, src/life.rs; tests/life.rs; test-life.bat).**
  Felled living tissue wilts to deadwood and leaf litter, which rot to
  humus. Where living plants are taken away, the 8 m square is remembered;
  every few minutes, any open ground with *living* plants within about a
  square spreads back: ground cover as the ground's top layer, new plants
  of the species growing beside it, waking the squares round it, until it
  is as wooded as before or nothing near is alive. Waits while drones still
  work there. Plant mass made or rotted is booked in the ledger's `grown`
  column. Timings in `[life.cycle]` (data/life.toml defaults in
  src/flora.rs `Cycle`). F1: "Life:" lines and a "show regrowth" button.
- **Plant sway (game/src/sway.rs):** near the camera (2+ px a metre) plants
  that move are left out of the tiles and drawn swaying (trees in rolling
  gusts, grass rippling, sea plants drifting). Benjamin approved it.
- **Forester and abductor, ported one for one from the swarm mockup**
  (game/src/units.rs `draw_tripod`, `gait_step`): shell, eye stalks, saw
  claws, rotor, bag; the abductor's head, eyes, fangs, whip palps and sac;
  the mockup's gait (planted feet, springy body). The body rides 50 m over
  the tallest plant within 30 m (src/units/forester.rs `walker_rise`), feet
  re-found on the ground every second. A full bag is dropped for couriers
  (drawn coming down for it); its contents are credited when they lift it.
- **Sapper blast, ported one for one from the proving ground**
  (src/units/sapper.rs `sapper_blast`): energy rays, cracks, pieces held
  together, molten heart, thrown rock, loose rubble and a plume out of the
  drill hole; goes off with the top of its blast 50 m down; keeps off spots
  other sappers are heading for; drill spoil spouts out and lands as rubble.
- **Procedural legs (game/src/rig.rs):** any number of legs and joints a
  leg. Feet planted on the ground as drawn (the screen's copy), stepping
  when the body leaves them too far behind, in gait groups that take turns;
  each leg solved by FABRIK with a pole so knees bow up and out; the body on
  springs. The forester is now a sideways crab (`draw_crab`): four legs in
  diagonal pairs, a big claw each side, the bag under the middle, eye stalks
  in the middle firing the laser. The abductor keeps the mockup's spider
  look on three curved four-bone legs. Rigs are defined in `rig_of`.
- **Forester behaviour:** claws and laser work while it walks too
  (`walker_hands`); a claw shears a ~4 m section off the nearest plant
  within 32 m; the laser sweeps 15-40 m of ground every 1.5-2.5 s; it leaps
  only real terrain (`terrain_at` ignores plants, dead plant matter and
  rubble; 20 m rise at 45 degrees within 24 m). Bag 80 t.
- **Sapper physics, as the proving ground:** rubble and melt settle each
  tick (`settle`), the plume pours out over ~1 s, caught sappers go off
  0.12-0.22 s later; buried blasts draw a shock ring, a white flash and a
  screen shake only when on screen, and a fading heat glow. Yield 5 kt;
  drills 25 m where it can't crawl deep enough.
- **Smooth drawing clock (game/src/drones.rs):** drones, haulers and units
  are drawn on a clock that advances every frame and eases onto the
  simulation's time (they were stepping when updates arrived late).
- `--watch <unit>` keeps the camera on the first unit of a kind.
- **Loose debris:** blast rubble, fallen and thrown rock, deadwood, leaf
  litter and humus take a cutter `harvester.loose_s` (1 s) a cell.
- **Looks** (src/world.rs `Look`): cracked, held, rubble, baked, melt,
  glass, kept per cell beside the material, carried to the screen's copy
  of the ground and tinted in the tiles. Melt cools to glass after ~25 s.
- **One lighting engine** (game/src/fx.rs): the mockup's emit/lightPass, a
  1 m light map near the camera in stepped bands; lights from the tripods'
  eyes and lasers, explosions, melt, the sapper's drill and blink. Also
  sparks, smoke and dust, and plume dust columns.
- Save version 13 since the sappers' charges (the world save 6).

Earlier (11 Nov), built and passing the tests, waiting on a PC look:

- **Universal Drone Replicator** (11 Nov, testing stand-in for the meta
  tree; `[units]` in data/economy.toml, src/units/*.rs (was src/specialists.rs),
  game/src/units.rs, tests/specialists.rs; test with test-replicator.bat).
  Extra factory bays build the specialists designed in the foundry mockup's
  Units page, only where their skill has work (`Skills`; report for every
  body: `cargo run --release --bin unitcheck`): sappers and tunnelers
  everywhere, heat-sink bots where there is magma or molten metal/sulfur
  (and only once an open pool is found near a flock), foresters where plants
  grow, abductors where there are animals (the Dev planet, since 13 Nov). Sappers: crawl down
  cracks/caves or drill, go off deep (2 kt, 18 m radius: a cavity and cracked
  rock cutters take in half the time); anything that destroys one (another
  blast, magma, blast heat, falling rock) sets it off, so they chain. Their
  nuclear charge needs uranium and nothing else (no Pu in the element table;
  uranium is kept out of "anything solid"); the fleet brings 200 kg, 25
  charges, and mined traces rarely make more. Tunnelers reuse the old
  Tunneler's boring with a 12 m shaft. Heat-sink bots freeze magma round
  their tentacles and beam power. Foresters walk on three legs, shear plants
  into their bags and set ground cover alight. New parts and recipes in
  tools/recipes.py (strict components: no "*" stand-in). Save version 11.
- **Haulers hover** (11 Nov): never land; six squid tentacles of sinew and
  bone instead of legs (game/src/drones.rs).
- The mockups in git are now the published versions (the foundry mockup
  with the Units and Replicators pages, and mockups/sapper-proving-ground.html).

0. **Production skeleton step 1** (10 Nov): the fleet arrival, the
   fabricator, the factory's bays and the orders queue (src/production.rs;
   tests/production.rs). The economy is always on in the game now.
   **Production phase 1, parts** (10 Nov): ten lore parts in data/economy.toml
   (from tools/recipes.py), a parts stock (`Parts` in src/recipes.rs), cutters
   and haulers built from parts, a Parts tab in the stockpile window. Test
   with test-barren.bat.
1. **Full element table**: 75 elements (to radium, no lanthanides, plus
   thorium and uranium), 33 new minerals, the stockpile window showing the
   whole periodic table with Raw / Refined, "not on this body" and
   "traces only" markers.
2. **Natural baseline**: each rock carries at most two trace elements;
   every ore forms in only a share of the places it could
   (`[geology.scarcity]` in data/geology.toml, tuned by
   `tools/tune_scarcity.py`), so the plain generator holds each element at
   roughly Earth's-crust levels.
3. **Planet specialties** (`data/specialties.toml`, `src/specialty.rs`):
   rolled from the seed; rich veins, a trace through all of a common rock,
   trade-off districts, rare jackpot lodes; asteroids and comets roll more
   and alone get 22 element caches (pit stops). Shown in the stockpile
   window ("This body:").
4. **Meta-tree bias**: every body spec has a `bias` (element weights,
   specialty weights, jackpot odds, extra rolls) for the meta tree to steer
   planet selection. Neutral for now.
5. **Rarity-priced recipes** (`tools/recipes.py` writes the recipe tables in
   data/economy.toml): each drone part's elements run rarest (best, 1 kg per
   kg) to commonest (steep markup), ending in "anything solid".

Measuring: `run-abundance.bat` (or `cargo run --release --bin abundance`,
with `--plain` for the generator without specialties, `--odds 200` for how
often each specialty rolls).

## Machine art: the mockup and its rules

The look of the swarm's machines is being worked out in a mockup page, not
the game: `mockups/swarm-foundry.html` (double-click `open-mockup.bat`). It
is also published on claude.ai at
https://claude.ai/artifact/F1JNmmbvsJ247V5VZPhQvi; update both together, and
after publishing, open the page for Benjamin (he can't find links in chat
text). Once he approves a design, it goes into the Bevy game.

Rules Benjamin set (10 Nov 2026), for every piece of art:
- **Chunky:** everything is 1 m blocks. No smooth circles, clean gradients,
  thin vector lines or visible seams. Bloom, smoke, lasers and liquids are
  made of blocks too.
- **One lighting engine:** emitters add light to a light map, applied in
  stepped bands (the mockup's `emit`/`lightPass` is the reference).
- **One piece:** every machine is contiguous, with no floating bits; a weld
  pass plus a connectivity check after generation.
- **Biomechanical** (standing mandate, Matrix-style machines): carapace
  plates, ribs, vertebrae as connectors, sinew cables, tendrils, eyes, horns.
  No long rigid bars. Body-horror extras (eyes, bones, heart, hands...) are
  separate switchable "grafts" so they can be pulled back.
- **One parts library**, every part classed by the matter it works best
  (metal, ore, fluid, gas, life). The planet's make-up picks the parts, so
  living worlds grow life-like machines. Life works appear only where the
  planet has life.
- **Storage ships** are cargo holds you can see into: boxy hulls, big
  windows split into bays of one matter each. No nose, tail, processing
  works, barge look or UFO curves. Bigger ships keep the same shape (lore: an
  efficiency measure) and gain bulkheads and fittings.
- **Bigger refineries** (medium up) are planned towns: spine corridors,
  cross corridors, hubs, and sectors with one job per row.

Not yet redesigned: the fabricator and factory ("later").

## Cutters: level of detail and formations (14 Nov)

Benjamin, 14 Nov: cutters must work exactly when their flock is zoomed in
on; every other flock may be approximated (sample-based or statistical);
simulate about twice the screen; and give flights some flocking beauty.

- **The focus** (`Sim::focus`, `set_focus`, same three states as
  attention): the game sets it to the screen plus half a screen each side
  while a cell is at least half a pixel (mpp <= 2), else nowhere
  (game/src/main.rs `attention`). Tests and tools default to everywhere,
  so they play exactly as before (samecheck fingerprints unchanged).
- **Far trips** (`far_trip` in units/cutter.rs): a cutter whose flock's
  hauler is outside the focus (by more than search_m + chain_m) does its
  whole trip at once: one search from the hauler, then the next cells from
  that same candidate list (best first, each within chain_m of the last, as
  a flown drone chains), each cut via `cut_at` (the old Outbound branch,
  moved out unchanged: mining, pay, heat, wear, losses), and one event, its
  arrival home (it sets off home when the last cut would be done;
  `head_home_after`). Cells are taken at the trip's start rather than over
  it. Counted in `far_trips` / `near_trips`.
- **Measured** (unitbench stage 1, 600 game s, NOSAVE): old way
  (`NOFOCUS=1`) 32.9 s real, cutters 48 ms per game s with 93k working;
  far trips 25.9 s, cutters 36 ms with 144k working, about half the cost
  per working drone. It mines 16% more by 10 min only because cells are
  taken at the start of a trip in a fast-growing swarm; the new test
  `cutters_out_of_sight_dig_alike_and_keep_the_books` (tests/swarm.rs) has
  them within 4% over 15 min on the tutorial body, books balanced. The
  benches now run with the focus nowhere, so the next bench-units.bat row
  shows a new fingerprint.
- **Formations** (game/src/drones.rs, drawing only): flights weave mostly
  on a wave shared by the flock and running back along the stream (a
  ribbon), and `formation` is boids for drones in flight: cohesion toward
  the neighbours flying the same way, separation under 1.6 m, smooth
  weights, at most 12 neighbours (starlings heed about 7), a grid of 7 m
  squares, faded to nothing at take-off and arrival so every drone still
  reaches its cell; at most 3 m off its line; off above 60k drones in
  flight on screen. Not yet seen moving by Benjamin.
- **Research behind it, and what's next on this road:** Reynolds' boids and
  spatial hashing (done); topological neighbours from the starling studies
  (done, as the neighbour cap); Sunshine-Hill and Badler's "alibi
  generation" (2010) for agents coming into view: next, give drones of a
  flock entering the focus mid far-trip a believable place (now they sit
  at their last cell until they set off home); hybrid macro/micro crowd
  models: next, a flock-level statistical mode for flocks far out of sight
  (one search and one event per flock per few seconds for all its drones,
  drones only counted) to cut the remaining per-trip cost.
- **Shoving** (Benjamin: show their voracity, competing despite their
  recklessness; rare but noticeable, about one every 15 s in any flock):
  each flock (keyed by its hauler's end of the flight) shoves once every
  12 to 18 s of real time; one of its drones in flight lunges at the
  nearest beside it within 3 m; the one hit is knocked 2.2 m aside and
  drops up to 4 m back along the stream, working its way in again over
  2.6 s (`Shoves` in `DrawMem`, `formation`; drawing only, faded at either
  end of the flight so arrivals stay exact).
- **Only cutters and haulers in the game** (Benjamin: no other drone until
  we work on it or have finished it): economy.toml `[units] game_deploys
  = []` lists the specialists the game builds; the game copies it into
  `Sim::deploy_only` (not saved; applies to loaded saves too) and the
  replicator orders nothing else. Tests, benches and tools still build all.
- **No production in the game yet** (Benjamin: just deduct the resources,
  fake-refining them, and the energy, and spawn): the game sets
  `Sim::instant_build`; every order is built the moment it's ordered from
  the stockpile, refined first and then raw (`Stock::plan`), and energy
  (`build_now`), with no fabricator or bay times. Tests keep the factory.
  Seen on the Dev planet at 6:40: 12,237 cutters built (930 with the
  factory), 72 haulers, 1,096 cutters waiting for haulers and 1,371 starved:
  the factory was the throttle; now growth is limited only by stock,
  energy and the director's orders, and haulers are the bottleneck.

## Haulers: the fleet commander (14 Nov)

Benjamin, 14 Nov: tie haulers to activity, not flocks; the director judges
each flock's coming volume and haulers are dispatched ahead of time
through a LIFO queue to points central to a flock, with a flock given two
or more drop-off points when it needs more than one hauler; haulers never
closer than one hauler's length; a fleet commander agent (air traffic
control) handles the calls so the director doesn't; haulers and waiting
cutters use the clear air, hundreds of metres of it.

- **`src/units/fleet.rs`** (the commander), `[fleet]` in economy.toml.
  Every 5 s (and at once for a new flock) `place_docks`: each live flock
  gets 1 to `max_docks` (4) drop-off points, as many as keep one hauler
  from filling in under `fill_min_s` (180 s) at its volume (`flock_rate`:
  kg cut lately / memory_s), spread `dock_spread_m` (60 m) apart along the
  face over its work, `dock_hover_m` (60 m) above the highest ground under
  it and a hauler's length either side; any point within `dock_gap_m`
  (24 m, a hauler's length clear) of one already placed is stacked a gap
  higher, up to `dock_stack_max_m` (300 m). Haulers at a point that's gone
  lift to unload (`release_hauler`).
- **Calls, LIFO:** each second a point calls (`Call`, one stack for the
  swarm) when it has no hauler there or coming, or when its hauler will be
  full (or its stay up) by the time one from the mother swarm could get
  there plus `call_margin_s`. Free (`Idle`) haulers wait at the mother
  swarm; the newest call is answered first by the nearest free one
  (`send_to_dock`). A relief arriving sends the one there up to unload.
  Haulers are built only for calls beyond those lifting/unloading/on
  order (at most 4 a second); one idle `idle_recycle_s` (600 s) is
  recycled (its mass back). Busy flocks call more and so get more of the
  fleet. F1 shows the commander's lines (`fleet_lines`).
- **Haulers** (units/hauler.rs): a new `Idle` state, a `dock`; after
  unloading they wait free instead of returning to "their" flock. The old
  director (`direct`), max_per_flock, spare-hauler blowing up, and the
  per-flock hauler sending on splits and merges are gone. Saves: the dock
  rides in the hauler's state byte (old saves load with point 0).
- **Cutters** (units/cutter.rs): each has a `dock`; heading home it picks
  its flock's nearest point with a hauler (`pick_dock`), else the nearest;
  arriving with none there but one at another point, it flies over; it
  waits in a cloud out to `wait_cloud_m` (30 m) round its point, in the
  air. Searches still start from the flock's landing point; flights out
  start from the drop-off point.
- **First look** (Dev planet, game, 6:40): old way 12,237 cutters built,
  1,096 waiting for haulers, 1,371 starved, swarm matter 247 kt; the
  commander's first run 3,342 cutters, 0 waiting, 24 starved, 51 drop-off
  points over 14 flocks, 64 haulers, 154 kt (flocks at their most
  drop-off points were then sent no more cutters; that limit is gone, and
  the next run reached 175 kt). The climb to the drop-off points costs
  cutter time.
- **Fixes after Benjamin's look:**
  - Haulers teleported: a working hauler was drawn at its drop-off point,
    which is placed afresh every 5 s. Now it keeps its own place and,
    when its point moves, glides there straight and eased at `drift_m_s`
    (6 m/s), bobbing slowly up and down while it waits (drawing). Its
    stay is timed from its arrival (`Hauler::since`). Test:
    `working_haulers_follow_their_point_smoothly` (no working hauler moves
    more than 9 m in a second, the eased glide's peak).
  - Haulers filled at once: a hauler now loads for at least `load_min_s`
    (5 s, Benjamin's figure) before it lifts, and its hold is drawn
    filling over that time.
  - A hauler that will be filled is unavailable at once: each cutter
    heading home counts its load against its point (`Sim::incoming`,
    `Drone::promised`) until it arrives, turns or is lost; a hauler whose
    load plus what's on its way reaches capacity is no longer picked
    (`pick_dock`), though those already coming still unload into it.
    `incoming_check` and the flocks test check the count matches the
    cutters actually flying.
  - Cutters chased a full hauler up: a full hauler now takes no more and
    doesn't count as a place to go (`hauler_at`, `hauler_at_dock`,
    `pick_dock`); the moment it fills (`hauler_filled`), cutters on their
    way to its point turn mid-flight for the nearest point with room or
    a hauler coming. To turn a drone mid-flight, every drone event now goes
    through `schedule`, which notes when it is due (`Drone::due`); an
    event at any other tick was overtaken and is skipped.
  - Cutters flew to where a point had been and piled into one dot there:
    the drawn flight ended at the point's old place (the renderer was
    never told the point moved), and the drawing got there early (it flew
    the straight line; the sim times the way round through open space),
    so arrivals sat stacked on the exact point until their event. Now:
    - `place_docks` keeps a point where it was if the new place is within
      half a hauler gap (12 m): small shifts cause no traffic.
    - When a point moves or goes (`docks_moved`), cutters flying to it
      turn mid-flight for its new place (`redirect`, which keeps a far
      trip's set-off time), and cutters waiting there fly over with their
      loads; idle ones take the new point.
    - A flight home is drawn timed to arrive when the sim says it does
      (speed clamped to 35-100% of cutter speed).
    - Drawing: a cutter flying home makes for its own place in the
      waiting cloud over the last 40 m (`CLOUD_EASE_M`), and one setting
      off from a point leaves from its place in the cloud (`Leg::cloud`,
      `Drone::off_dock`): no stacking, and waiting cutters circle the point.
- **Loose matter adrift is grabbed first (14 Nov).** Chunks cut free (plant
  matter above all, on the low-gravity Dev planet) floated off for good:
  cutters only took cells in the ground. Now each second `sort_adrift`
  hands every body in the air (`Bodies::adrift`) to the nearest flock
  with a drop-off point within 1.5 search radii (240 m), up to 2 holds of
  mass. An idle cutter takes one of those before looking for rock to cut
  (`take_adrift`, `go_grab`: aimed where the body will be), takes it whole
  out of the air on arrival (`Bodies::grab`, booked as mined), and goes for
  the next one near by or home. Dev planet, 600 game s with everything
  simulated: 2,335 bodies grabbed; matter lost to space 1,334 t against
  2,492 t without grabbing (`NOGRAB=1` simbench; that figure includes vents
  and blasts). About 350 bodies are still in the air at any moment (too far
  or too big). A reach of 3 search radii did worse (1,140 grabs, 2,684 t
  lost: cutters chased far chunks that drifted off), so 1.5 it is.
- **The sieve out of sight (14 Nov).** Benjamin's recording: zoomed out, a
  flock's face turned into a sieve dozens of metres deep with single
  cells hanging in the air, cleared only when he zoomed in. Cause: out of
  sight, cells next to new holes weren't checked for being cut free (they
  waited, in `Bodies::waiting`, until someone looked; that list is also
  dropped past 20,000), so chunks cutters had left hung in place. Now
  `break_free` also checks up to FAR_STARTS (300) such cells a step out
  of sight; what comes loose there is set straight down on the ground
  below (as `step` already did for bodies out of sight). And cutters take
  a speck (a cell with three or four sides open, too small to fall: under
  `island_min_cells`) as loose debris (`is_speck` in `is_loose`): quick to
  pick and never turned down as poor pay, so flocks clean them up.
  Dev planet, 2,400 game s, nothing in sight: the ground round a landing
  point went from hanging columns and strips to a clean face, and the
  run took no longer (52.9 s against 54.8 s).
- **Atmosphere haze (14 Nov, game/src/tiles.rs).** Each cell cut showed a
  blank, starry hole. Now:
  - The haze's floor is a smooth surface: the ground as generated, sunk by
    the depth eaten in each direction (from the published heights)
    averaged twice over 120 m either way (`air_floor`, `AIR_SMOOTH_M`). A
    shaft or tunnel sinks it a little over a wide stretch instead of
    pulling a sliver of haze down into it.
  - It reaches 100 m below its floor into eaten ground, fading out
    smoothly (`HAZE_DEEP_M`).
  - Its height, depth and strength shrink as the body is eaten, gone at
    half (`HAZE_GONE_AT`; strength by the square root), in 50 steps, each
    step redrawing the tiles with air in them.
- **Haulers carry 5,000 t (14 Nov).** `[hauler] capacity_kg` 1,000 t →
  5,000 t (Benjamin). Dev planet, 1,200 game s: about a third the hauler
  calls (213 against 671) and no calls left waiting (27 before), with half
  the drop-off points (30 against 59).
- **Loose matter out of sight settles at once (14 Nov, sandbox.rs).**
  Benjamin: zoomed in on water, it "explodes into more water" and plays its
  physics out as if the game had been paused. Cause: out of sight every
  powder, liquid and gas cell that would move waited, dormant, until seen,
  then all of it moved at once (and buried layers welling up holes cutters
  had dug waited too: that was the extra water). Now out of sight
  (`settle_far`, `move_far`): powder and liquid go straight down as far as
  they can, a cell down the slope either side, and liquid runs along level
  ground up to FAR_SPREAD (48) cells to the nearest drop, in one go, every
  step strictly nearer the centre; gas the same way up, lost to space once
  it rises off the body; with nowhere to go, a liquid still feels its
  layer's push (wells and vents, whose ejecta were already set down where
  they land out of sight). `Sandbox::far_settle` off (simbench `NOSETTLE`)
  gives the old freeze. Dev planet, 1,200 game s, nothing in sight: 64,652
  cells were left waiting to play out; now none, for about 1 s more
  sandbox time over the run (2.0 s of 28 s).
- **Zooming in on a flock (14 Nov).** Out of the cutter focus a cutter's
  trip is worked out at once (`far_trip`): its cells are cut as it sets
  off, and it is drawn waiting at its last cell till the trip would be
  done, which in view looks like idle cutters hanging at the face. Now, a
  second after a flock comes into the focus, `catch_up` (each second, from
  `fleet_work`) sends those cutters home at once from where they wait, with
  what they cut (a small gain, once per look, of the rest of that trip's
  cutting time); trips from then on are flown cell by cell. simbench
  `LOOK=n` zooms in on landing point n after the run: 852 caught up on the
  Dev planet at 600 s. And in the game (drones.rs `Catch`), a drone whose
  path changes under it is no longer drawn jumping onto the new one: it is
  drawn where it was, the gap closing at flight speed (beyond 3 km it
  jumps). That covers every path change, including idle cutters at a
  moved drop-off point and trips worked out at once.

## The big-swarm speed pass (night of 14 to 15 Nov)

Benjamin's ask: the game fell under 10x (down to 3x) past 100k cutters on
the 5 km Dev planet; aim for a million cutters on big worlds, the
simulation never under 15x (10x is the real floor, 5x headroom for the
other units). Anything out of sight or too small to see may be
approximated, as long as what is looked at behaves the same. Commits
6700b21 to 191b55d.

**Tools made for it:**
- `tools/prof` (its own workspace; `cd tools/prof && cargo run --release`):
  a sampling profiler. It runs the unitbench set-up and pauses the sim
  thread every millisecond, walking its stack (`WARM`, `SECS`, `BODY`,
  `KM`, `DRONES`; `CALLERS=name` lists who calls a function, `LINES=name`
  where in it the time goes). `attach.exe <program> [s]` samples another
  running program's threads by name (the game: build it with
  `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_TARGET_DIR=target-prof`
  for names, and pass `SYMPATH` to that folder).
- unitbench: `FINE=1` (cost split finer: events, searches, the per-second
  parts, the fleet commander, cuts), `us real per tonne mined` (the fair
  measure when a change shifts how much gets done), `NOSETTLE=1`, and the
  sandbox's out-of-sight counters. With a fixed swarm (benchmarks) flocks
  open 8x faster and haulers are built up to 64 a second, so a million
  cutters are at work in minutes.
- `stress-big-swarm.bat`: the real game, 300k cutters on the living world,
  the camera stepping through every zoom; prints speed and frame rate per
  shot.

**What was slow, and what changed:**
- The bottomless sea made water out of nothing out of sight (1.6 Mt in 5
  game minutes with 100k cutters), and the cutters drained it as fast as
  it came: an endless conveyor. Out of sight the sea no longer refills.
  Settling wakes only what could fall into the gap. Game, Dev planet, 100k
  cutters: 4.5x -> 13-19x.
- Benjamin's own log (the 3x session): zooming to 1.9 m a pixel woke a
  million cells waiting out of sight at once, and the sea poured; the sim
  fell to 0.3x. Both causes are gone (out-of-sight settling, no refill).
- The fleet commander: `hauler_filled` looked at every drone; drop-off
  points moved 41,527 times in 800 s (12.7 million cutters turned to follow
  them); placing points, taking calls and `pick_dock` / `hauler_at_dock`
  were all quadratic or scanned every hauler. Now indexed (an index of
  cutters flying to each point, the per-flock hauler index), points stay
  put while within 40 m of their ideal spot and clear (`DOCK_KEEP_M`), and
  grids instead of all-pairs. Waiting cutters out of focus are heated or
  cooled every 10 s per flock instead of one by one every second.
- Shared caches: the sandbox's ambient-heat cache was one lock for all its
  threads (now 64 parts, worked out outside the lock); the world now keeps
  what the generator gave for untouched cells (`World::gen_cache`); reads
  of neighbours go through a leaf-caching reader; three hot maps used
  Rust's slow default hasher.
- Out of sight, plain rock round a new hole stays asleep (`wake_opened`).
- The drones due in a tick make their searches for work side by side on
  the sim's threads (`prefetch_searches`; read only, then each drops what
  the ones before it took). A fresh leaf's 4,096 cells are generated row by
  row side by side.
- Tried and dropped: sharing one search among a flock's cutters out of
  focus (cheaper searches, but costlier sand and heat: worse per tonne);
  plant-free lookups for out-of-sight settling (no gain).
- Game, zoomed out: each flock is one mark past 5 m a pixel (`ICON_MPP`),
  a hauler under 8 px is one mark (each was drawn block by block, hull and
  tentacles: 16 ms of a frame with 2,000 in view), hauler spacing looks
  only at grid neighbours (it compared every pair six times a frame),
  tiles rebuild more slowly the further out (`REBUILD_ZOOM_MPP`), and the
  sim runs above normal priority on its own helper threads. Living world,
  300k cutters, whole planet in view: 14.5 -> 33 fps.

**Measured (headless unitbench, stage 1, everything out of sight, this
PC; speed = game seconds per real second at the end of the run):**

| World | Cutters | Working at the end | Speed | Before tonight |
|---|---|---|---|---|
| Dev planet 5 km | 100k | 89k | 17x (15x over the run) | 5.7x |
| Dev planet 5 km | 300k | 164k | 10x | - |
| Living world 35 km | 300k | 243k (159k waiting) | 13x | - |
| Living world 35 km | 1M | 493k (318k waiting) | 6.5x | 3.2x (800 s run 134.8 s -> 79.1 s) |
| Ocean world 40 km | 1M | 144k (80k waiting) | 16x | - |
| Earth 6,371 km | 300k | 13k (10k waiting) | idle | - |
| 4x Earth 25,484 km | 300k | 22k, all waiting | idle | - |

Cost now about 5-6.5 us of real time per tonne mined. 15x holds up to
roughly 250k working cutters; at 500k working it is 6.5x.

Real game, living world, 300k cutters (stress-big-swarm.bat): 7.1-7.4x
with the whole planet in view, 9.7x close up, 8.5x at mid zoom.

**Where the time goes now (1M, profiler):** cutter events 36% (the far
trips' cuts 21%, searches 7%), sand and heat 33% (out-of-sight settling of
water 13%), waiting on its own helper threads 14%, the rest under 5% each.
Next steps for a million at 15x: simulate flocks out of sight as a whole
(no event per trip, cut in batches), and settle out-of-sight liquids at a
slower cadence.

**Zoom:** the swarm's size sets the speed far more than the zoom. Zoomed
out the sim runs about a quarter slower than zoomed in with the same work
(7.1-7.4x against 9.7x at 300k cutters): the drawing and the tile builders
share the CPU's cache with it. Not thread contention (capping the game's
pools and raising the sim's priority changed nothing), nor tile locks.

**Big worlds are held back by the game, not the speed:**
- Earth size: after 10 game minutes only 13k of 300k cutters work. New
  ground is surveyed only either side of each flock, so after the first
  rounds no new flock can open (0 candidates most rounds). And every
  hauler climbs to the mother swarm, which circles 400 m over the highest
  ground on the whole body, kilometres up: 32 of 34 haulers "on the way".
- 4x Earth: all 21,934 working cutters waiting for haulers by 200 game s;
  mining stops. Both need a design answer (mother swarm near the work,
  landing zones as a purchase), not a faster sim.
- Even on the 35 km world, with a million cutters a third or more wait for
  haulers: the commander only builds one when the haulers on their way up
  won't be free soon, and on a big world that isn't soon.

## Covering a planet: the far field (15 Nov)

Benjamin's goal, restated: not fixed drone counts but a planet covered in
working drones at 15x (he will cap the game at 10x; the 5x is headroom for
the units and features still to come), on every body up to 4x Earth, which
comes late in the meta progression. Accuracy only where the player looks
("quantum"); far off, anything that looks right when he zooms in. Memory is
the only real limit he accepts.

**Coverage (what "covered" means).** A flock takes as many cutters as the
rock face in its reach has room for. On the 35 km living world the swarm
levels off at about 1.35 million working (350k of them waiting for
haulers): about 6 cutters per metre of surface (`COVER_PER_M`). So:

| Body | Surface | Covered |
|---|---|---|
| Dev planet 5 km | 31 km | 190k |
| Living world 35 km | 220 km | 1.3M |
| Ocean world 40 km | 251 km | 1.5M (its land only works) |
| Earth 6,371 km | 40,030 km | 240M |
| 4x Earth 25,484 km | 160,121 km | 960M |

Earth when the swarm could cover it: the ground eaten by then is 150 m
(benchmark growth) to 2.5 km (the real economy's pace, 1,240 t mined per
cutter alive) off a 6,371 km radius, so the surface is 0.002-0.04% shorter:
still about 240 million cutters.

**Two gameplay changes (Benjamin's asks):**
- The mother swarm arrives over the highest ground on the body
  (`highest_dir`), and the ring of landing points starts there.
- Haulers speed up on long trips: no trip takes much over 1.5x a flat-body
  trip (drop-off point straight up to the mother swarm), smoothly
  (`hauler_trip_s`, `[hauler] trip_cap`). Earth size, 300k cutters, 10
  game minutes: 13k -> 53k working, 315 kt -> 2,262 kt mined; 4x Earth no
  longer stalls.

**The far field (src/units/far.rs, world::FarCut).** The surface is cut
into sectors about 1 km wide (by angle). Near the camera the swarm is what
it was: flocks of cutters flown one by one. Beyond what the sim can afford
one by one (a budget that follows the sim's own cost, aiming at 50 ms per
game second, never trimming what the camera shows), whole sectors go far:
their flocks fold into a count, haulers go up to unload, and each far
sector is eaten as a whole, a bite every few seconds (staggered, at most
~400 bites a second, worked out side by side): its level comes down by what
that many cutters would cut (hold, dig time of the rock at the top, pace
0.6), highest ground first, delivered straight to the mother swarm.
- What is eaten: every cell further from the centre than its sector's
  level. Untouched ground is never generated for it (the rule is applied
  when read: `FarCut::cut`); cells already generated are emptied
  (`World::far_lower`). Not a flat plate: toward a higher neighbour the
  level ramps up over half a sector (an S-curve), and the face is pitted
  (bowls 300 m and 80 m wide, bites 15 m and 4 m, up to 24 m deep) so it
  reads as dug, not chopped. Untouched-cell reads, materialising leaves,
  the game's ground copy (`Mirror::get`) and the cave check all honour it.
- Loose matter in far sectors (water, sand) waits until someone looks
  (dormant): otherwise lakes woken by flocks before they folded drained
  across the eaten floor cell by cell, 140 ms a game second.
- A far sector next to a sea stops at the sea's surface (a shore is left).
- Sectors the camera shows (or within a screen of it, three screens across)
  closer than 5 m a pixel are always flocks: their cutters come back down,
  up to 20,000 a game second (`UNFOLD_PER_S`), over the ground as it now
  stands, into the flocks already there or new ones every 200 m.
- New cutters past the budget go straight to far sectors with room;
  sectors open beside the swarm's reach as it fills them.
- Saved (sim save version 15; older saves load with no far field).
- No camera (tests, plain benchmarks): nothing goes far; everything is as
  before. `FAR=1` (unitbench, tools/prof) gives a zoomed-out camera,
  `FAR=close` a close one, `FARSWITCH=t` comes in close at t,
  `FARBUDGET=n` fixes the budget, `FARPAN=m` (prof) pans.

**Measured, headless, zoomed out (FAR=1), this PC:**

| Body | Cutters | Speed once spread |
|---|---|---|
| Living world 35 km | 1.5M (covered) | 28-43x |
| Earth 6,371 km | 10M | 14-37x |
| Earth 6,371 km | 203M working (~covered) | 43x |
| 4x Earth 25,484 km | 815M working (~covered) | 23-26x |

Before: the living world with 1.35M working ran at about 1x; Earth and 4x
Earth couldn't be covered at all.

**In the game, 10x asked (stress-big-swarm.bat; sim speed and frames per
shot):**

| Body, cutters | Whole body | Mid zoom | Close up | Fast pan |
|---|---|---|---|---|
| Living world, 1.5M | 10x, 50 fps | 9.5x | 9.3-10x, 60 fps | 7.2x |
| Earth, ~240M | 10x, 60 fps | 9.9x, 57 fps | 10x, 47-55 fps | 10x, 52 fps |
| 4x Earth, ~960M | 10x, 56 fps | 8.8x | 10x | 10x |

Top speed (the same shots asked for 30x; the budget of cutters flown one by
one adapts to keep the sim near 50 ms a game second):

| Body, cutters | Whole body | Mid zoom | Close up | Fast pan |
|---|---|---|---|---|
| Living world, 1.5M | 24x | 17x | 24.5x | 13.5x |
| Earth, ~240M | 22-25x | 23.5x | 21.6x | 23.3x |

Zoomed out the sim spends 2-7 ms a game second. At mid zoom on the living
world 133k cutters come down (19 sectors on and around the screen) and it
still holds 9.5x. The benchmark's first 100 s run at 4-5x while it makes
millions of cutters at once (not something the game ever does).

What it took in the game: the mother swarm's currents were worked out along
the whole 40,000 km of it every frame (0.4 fps), far bites flooded the tile
redraws, and the fog charted every flock that ever was; unfolded cutters
found nothing to do (flow fields took eaten ground for rock, the first
search before a flock's field marked it dry for good, landing points left
hanging high over eaten ground), and a sector's level ran away when one
held millions (now no more work than its surface has room for, the rest
spread to other sectors; no sector more than 300 m under a neighbour).

**Camera (Benjamin's suggestion):** the wheel's zoom now glides (at most 4x
a second, eased) with the point under the cursor held still, and the sim is
told where the view is heading (a zoom's end, a pan a second on), so the
sectors there are flocks before they come into sight (`view::Glide`,
`view::Heading`). Drags stay one to one.

**Memory:** the benchmarks used to start a big swarm as hundreds of
millions of single drones (21 GB for an Earth); with a camera no more are
flown one by one than the budget, the rest wait for far sectors (Earth
covered: 0.4 GB headless, 4x Earth 0.5 GB). The game's fog of war kept a
hash map entry per charted cell on 26 grids (5 GB on an Earth in 5 minutes):
now a bit a cell, and times only for what was seen in the last 1.5 s
(Earth in the game: 6.6 -> 1.7 GB). Saves write the charted cells as deltas,
compressed: an Earth's worth may be tens of MB.

**Budget target:** 60 ms a game second (16.7x). The dev planet with 100k
cutters never goes far: all of them flown one by one, 10x in every shot at
about 40 ms a game second.

**To watch it:** stress-big-swarm.bat (living world covered) and
stress-earth.bat (an Earth covered), both at 10x with pictures of each
shot in logs.

**Open:** a fast pan along the surface (half a screen a second at mid
zoom) still dips to about 7x on the living world while sectors come down in
turn; the economy's growth is far too slow to cover anything big (living
world: 11k cutters after 4 game hours, held back by how fast flocks open,
with 11 Mt unused in the stockpile): a design question.

## What's next

See ROADMAP.md "Where we are (13 Nov)" for the reassessment and the proposed
order (a PC-look session, the units stage by stage, then production, then
life). The production plan below still stands for step 3 of that order.

- **Production plan** (agreed with Benjamin 10 Nov; it refines the Swarm
  Production Design doc):
  - The swarm arrives as a fleet: a storage ship with refined elements, a
    refiner, a fabricator, and a factory that carries and releases the first
    1,000 cutters. No haulers at first: the factory builds them.
  - Two ways things are built, both block by block from refined elements:
    ground units (cutters, haulers, later heat-sink bots, tunnelers...) in
    the factory, by lasers, from parts; swarm-dwelling machines (storage
    ships, refiners, fabricators, factories, factory bays) by the swarm
    itself, like storage ships. Factories build only ground units.
  - The factory is modular and procedural: bays and gantries for each unit
    class, floating over the swarm. More bays is the growth lever.
  - The swarm gets several of every machine as it wraps the planet. Where a
    factory goes matters: near the work (flocks) and its supply chain.
  - Couriers are only the picture of the logistics: they pop out of the
    swarm, fly a route and vanish. The simulation moves material as flows
    with a delay set by distance, never load by load.
  - The parts list grows as ground units are defined.
  - Step 1 (built): arrival, fabricator, factory bays, orders. Step 2:
    placement along the mother swarm, distance delays, swarm-built
    machines. Step 3: drawing. Step 4: hauler landing spots. Then energy,
    heat-sink bots, tunnelers.
- **Hauler landing spots** (with production): give each hauler its own
  landing spot so no two land on top of each other. In flight, haulers pass
  freely in front of and behind each other; Benjamin doesn't want them
  pushed apart in the air. For now the drawing only spaces out landed
  haulers (game/src/drones.rs), as a stopgap. Seen 10 Nov (rubble asteroid,
  32 min): every hauler of a flock lands on the one hub point
  (`site_pos`), so busy hubs pile up 10+; 1,315 haulers for 244 flocks with
  984 cutters still waiting, so the director also builds too many: size
  each flock's haulers from its round trip instead.
- Open questions for Benjamin: which generation-bending perks go in the
  meta tree first; planet ranking for selection (parked until he decides).
- Also waiting: proper map icons for the zoomed-out view (flocks and
  haulers are plain marks for now, see the speed pass), the lava flow
  bug (lava not running into hollows), warm ground and decay with
  radioactivity (technetium, polonium, astatine and francium are on no body
  until then).

## Last session's report (15 November 2026)

When Benjamin asks for a "handoff digestion", repeat this section to him
as it stands.

**The ask:** not fixed drone counts but a planet covered in working drones
at 15x (the game will be capped at 10x; the rest is room for what's still
to come), on every body up to 4x Earth; the mother swarm arriving over the
highest ground; haulers speeding up on long trips (about 1.5x a flat trip
at most); and how many drones it takes to cover an Earth. Later: predict
where the camera goes, smooth the zoom, and look at screenshots so eaten
ground reads as dug, not chopped.

**Coverage:** about 6 cutters per metre of surface at work (measured on the
living world). Dev planet 190k, living world 1.3M, Earth 240M, 4x Earth
960M. Earth when the swarm could cover it: still about 240 million, as it
will have eaten only 150 m to 2.5 km off a 6,371 km radius by then.

**The far field (see "Covering a planet" above):** away from the camera,
and only once there are more cutters than the sim can fly one by one at
15x, whole 1 km stretches of surface are worked as a count of cutters
eating the ground from the top down: highest ground first, ramps between
stretches, pitted so it reads as dug. What the camera shows is always
real flocks; stretches come back as flocks as the camera arrives.

**Top speeds (game, asked for 30x):** living world covered 17-24x, its
fast pan 13.5x; Earth covered 22-25x in every shot. So 15x holds, with room,
except a fast pan on the living world.

**Speeds now (game, asked for 10x):**
- Living world covered (1.5M): 10x zoomed out, mid zoom and close up; a
  fast pan along the surface dips to about 8x.
- Earth covered (~240M): 10x and 50-60 fps in every shot, panning included.
- 4x Earth (~960M): 10x zoomed out, close up and panning; 8.8x at mid zoom
  while the sectors there come down. (Its giant craters, 46 km deep bowls,
  needed the generator's crater-aware ground height for landing spots.)
- Headless, everything off camera: living world 28-43x, Earth 43x, 4x
  Earth 23-26x. Before today the living world with 1.35M working ran at
  about 1x and Earth couldn't be covered at all.
- Small worlds under the budget (the dev planet with its usual swarm) play
  exactly as before: nothing goes far.

**Also:** mother swarm over the highest ground (Earth size, 10 game
minutes: 13k -> 53k cutters at work), long hauls capped at 1.5x, the
wheel's zoom glides and the sim gets ready where the view is heading, and
on Earth the game went from 0.4 to 60 fps zoomed out.

**Open:** the economy grows the swarm far too slowly to cover anything big
(living world: 11k cutters after 4 game hours, 11 Mt unused in the
stockpile): flocks open one at a time. A design question.

**Memory:** an Earth covered runs in 1.7 GB in the game (was 12 GB): big
swarms no longer start as millions of single drones, and the fog of war
keeps a bit per charted cell instead of a map entry.

**To watch:** stress-earth.bat (new) and stress-big-swarm.bat.

**Tests:** full suite 112 passed, 0 failed, 3 ignored (as before), with 3
new far-field tests. Commits ea57dd1 onward; not pushed.

## Previous session's report (14 to 15 November 2026)

**What we did (14 Nov, with Benjamin):** the fleet commander for haulers
(drop-off points, calls, relief), cutters following moved points and
circling while they wait, cutters grabbing loose matter adrift first, the
atmosphere haze reaching into eaten ground and thinning as the body is
eaten, the low-zoom "checkerboard" fixed (chunks cut free out of sight now
fall), loose matter settling out of sight instead of piling up to play out
when looked at, zooming in on a flock catching its cutters up, drones
flying (not jumping) onto a changed path, and haulers carrying 5,000 t.

**Overnight (Benjamin away, asked for a speed pass):** see "The big-swarm
speed pass" above for every change and number. In short:
- Dev planet, 100k cutters: 5.7x -> 15-17x headless; in the game 4.5x ->
  13-19x. The worst slowdown was the bottomless sea making water out of
  sight for the cutters to drain, and a backlog of loose matter waking all
  at once when zooming in.
- A million cutters on the 35 km living world: 800 game s took 134.8 s,
  now 79.1 s; about 500k working there at 6.5x. 15x holds up to about
  250k working cutters.
- Zoomed out, the game draws flocks and small haulers as single marks
  (whole-planet view with 300k cutters: 14.5 -> 33 fps); zoom changes the
  sim speed by about a quarter, not more.
- Earth size and 4x Earth: the swarm can't spread or haul (new ground
  only opens beside flocks; haulers climb kilometres to the mother swarm),
  so few cutters work. A design question for Benjamin, not speed.
- New tools: a sampling profiler (tools/prof), finer bench numbers, and
  stress-big-swarm.bat for the real game.

**Tests:** full suite 109 passed, 0 failed, 3 ignored (the two old tutorial timing tests and one other kept off as before). Game builds. Commits 1f3a0dc to the last of the night; not
pushed (the safety check blocked pushing; Benjamin can push).
