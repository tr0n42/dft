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
- Also waiting: map icons for the zoomed-out view, the lava flow
  bug (lava not running into hollows), warm ground and decay with
  radioactivity (technetium, polonium, astatine and francium are on no body
  until then).

## Last session's report (13 November 2026)

When Benjamin asks for a "handoff digestion", repeat this section to him
as it stands.

**What we did:** pushed and pulled between the two PCs, ran the first
500k-cutter stress test, fixed the biggest slowdown it found, split every
unit's code into its own file, built the "500k cutters with X" benchmark,
and made the Dev planet with placeholder animals for the abductor.

**500k stress test:** with 500,000 cutters the cost grew faster than the
swarm: every drone event looked through every hauler, and every cut
through every hub. Both are indexed now: 20 game minutes went from 251 s to
166 s with exactly the same game.

**Unit platform:** each unit's code is in its own file (src/units/:
cutter, hauler, sapper, tunneler, heatsink, forester, abductor, plus
platform.rs for what they share). A new tool, samecheck, proved the game
plays exactly the same after the move.

**"500k cutters with X":** bench-units.bat runs 500k cutters for 20 game
minutes with the units designed so far, times each unit, and keeps every
run in perf/history.csv so we see what each change costs. Stage 1 (cutters
+ haulers) baseline: 189 s real; per game second cutters 90 ms, sand and
heat 35, the rest of the main loop 22, falling rock 8, haulers 2. The plan:
design cutters and haulers to what you want, then add sappers, tunnelers,
heat-sink bots, foresters and abductors one stage at a time.

**Dev planet (run-dev-planet.bat):** a 5 km test world where every drone
has work: hard rock and uranium shale (sappers), ore close under the ground
(tunnelers), dozens of open lava pools (heat-sink bots), plants (foresters),
and herds of placeholder animals (abductors). The animals are block
critters that graze, and run with their white tail up when a machine comes
within 80 m or one of the herd is snatched. New herds wander in so they
never run out. Abductors stride to a herd, run an animal down and snatch it
alive into a stasis sac; couriers carry full sacs up, worth twice their
mass. In 30 game minutes: 35 blasts, 1,100 t of lava frozen, 9,900 t of
plants, 430 m of shaft, 633 animals caught, 445 still roaming. Every new
drone gets its work added to this planet.

**Costs found (measured, not fixed):** cutter events get dearer as the swarm
spreads; sand and heat grows with the dug area; the animals cost 12.7 ms a
game second for about 400 of them (each looks for the ground three times a
second), about 70% of what 20,000 cutters cost.

**Roadmap:** ROADMAP.md now opens with a map of every document and a
reassessment: what the last three days built, where it fits the plan, what
was skipped (production steps 2-4, energy, plant panic, real animals, the
tech trees), and a proposed order to confirm: a PC-look session, then the
units stage by stage, then production, then life.

**Tests:** full suite 104 passed, 2 failed, both on the new planet file
(its generator fingerprint wasn't locked yet, and it switched plant life
on in its own file, which no body does); both fixed and passing. Game
builds. Committed and pushed.

## Previous session's report (12 November 2026)

**What we did:** plants now live and die, the forester became a sideways
crab, walkers got procedural legs, the sapper's blast is ported one for
one from the proving ground, and the game has its lighting engine.

**Plants:**
- Felled trees die and rot: deadwood and leaf litter, then dark humus soil.
- Where plants are cut, eaten or burned, life spreads back from living
  plants nearby, all the time: ground cover first, then new plants of the
  species beside it, creeping out over eaten ground until it is as wooded
  as before. Nothing grows back where nothing nearby is alive, and not
  while drones are still working there.
- F1 shows "Life:" counts and a "show regrowth" button that jumps the camera
  to a plant growing back.
- Plants near the camera sway (you said it looks good).

**Forester:** now a sideways crab: a claw each side, the bag slung in the
middle, four legs (two a side, stepping in diagonal pairs), lasers from eye
stalks centred on the body. Its body stands 50 m over the tallest plant
around it and the legs scale to reach the ground. It cuts with its claws
and burns with its laser while it walks, not just when it stops, and it
leaps only over steep terrain (at least 20 m of rise), never over trees,
fallen trunks or rubble. A full bag is dropped; couriers come down for it
and carry it up, and only then does it count. In a 20 min test it took
1,675 t of plants, fired 1,894 times and leapt 25 times.

**Abductor:** ported from the mockup on three long curved legs; it still
has nothing to catch (that waits for animals).

**Procedural legs (new, game/src/rig.rs):** one system for any number of
legs and joints, built from the usual game method (FABRIK reach solving,
feet planted on the ground until they lag too far, legs stepping in turns,
a springy body). It replaced the old hand-made gait for both walkers.

**Sapper:** the blast breaks rock the way the proving ground does (rays,
cracks, held chunks, a molten heart that cools to glass, rubble that
settles, thrown rock, and a plume out of the drill hole that lasts several
seconds), with the shock ring, the heat glow and chain blasts timed as on
the bench. Yield is 5 kt by default; a sapper that would go off near the
surface digs down about 25 m first. The flash and screen shake happen only
when the blast is on screen. In a test, 28 blasts cracked about 100,000
cells.

**Smoother motion:** drones, haulers and walkers are now drawn from a
smoothed clock, so they move at the screen's frame rate instead of
stepping at about 30 per second.

**Loose debris** (rubble, fallen rock, dead plants) takes a cutter about one
second a cell, whatever it is.

**Lighting:** one engine, as in the mockup: eyes, lasers, explosions, melt
and drill sparks light the blocks around them in stepped bands.

**Tests:** full suite 99 passed, 0 failed. Game builds. Everything is
committed; not yet pushed.

**Later the same day (second PC): sappers' charges.** Uranium no longer
runs out. A fuser in the refinery fuses two atoms into one heavier one; it
makes uranium from iron and calcium (iron + iron = tellurium, calcium +
calcium = zirconium, the two = uranium), at a high energy cost. Where no
uranium can be had or made, a sapper gets a conventional 5 kt charge instead,
cast from 3,125 t of RDX or 5,000 t of aluminium-oxygen slurry, looking the
same. Every nuclear blast leaves about 3.9 kg of unreacted uranium in its
rubble for the cutters. Test with test-fuser.bat (a lava world, no uranium
of its own; the F1 panel's "sapper charges", "uranium left in rubble" and
"fuser" lines). In 40 min on a lava world: 72 sappers instead of 25, 384 kg
of uranium fused. With the fuser off, conventional charges are slow to come
(only one in 40 min: it waits for about 5,300 t of refined aluminium).
Full suite: 106 passed, 0 failed. Not committed yet.

**Later still: the slowdown.** Benjamin noticed the game eating far more
CPU. Measured (same 5 km living world run to 30 game min as fast as it
goes): before the 12 Nov commit 8 s, after it 22 s, with the fuser 25 s.
The cost was the specialists' new work done in full even off screen: sapper
blasts (thrown rock and the plume, then rubble settling cell by cell; the
molten heart's heat), and the foresters' first plant scan building the
ground of 256 places round the body at once (a 1.7 s stall). Benjamin's
rule now (memory): off screen can almost always be approximated. Fixed:
out of sight a blast's heart goes straight to glass and nothing is thrown
or blown out (it lies as rubble where it broke); rubble out of sight waits
and settles when the view comes over it (`settle_wait`); falling chunks and
thrown rock land at once when zoomed out too (bodies.rs); the plant scan
looks at 8 of its 256 directions per scan (plus over every flock). Now 13 s
in the game (10 s headless). `cargo run --release --bin simbench` times the
sim alone with features switched off (VERBOSE=1 for per-2-minute stage and
finer timings, `Sim::prof2`); the game log now also times lights and plant
sway. Still to look at: drawing drones got about 10x dearer per drone
since 11 Nov (about 2 ms a frame at 1,000 drones, 3.8 ms at 3,000).

**Last: blasts on screen.** Benjamin: the sim must keep going, at least to
the eye; simplify the boom so it doesn't hitch. The blast's own maths was
never the cost (about 20 ms); the stall was flying every thrown cell as a
physics piece (often 1,000+ per blast) and then settling them. Now a blast
on screen throws at most 200 pieces and blows at most 200 out of its plume
(`THROW_MAX`, `PLUME_MAX` in src/units/sapper.rs, a fixed random share); the
rest lies as rubble where it broke. Watched everywhere at once
(`simbench ... all`), 10 game min went from about 40 s to 28 s.

**On "500k drones used to cost 150 ms".** Measured per drone, the drones
themselves are no dearer: about 0.5 microseconds per drone per game second
(1,200 drones, 0.6 ms per game second; 500k would be about 250 ms, about
what it was). What is new is fixed cost that doesn't scale with drones:
the specialists, life, falling sand and heat, and fires. Zoomed out it is
now about 4 to 6 ms per game second in all (`simbench`, VERBOSE shows a
new "specialists" timer, `prof2[11]`). Next, if the game still feels slow:
run the game with a big swarm and read the log's stage timings to see which
of those it is in that case, and look at the drone-drawing cost above.

**Open questions:** plant panic (species-by-species losses; most species don't react;
exodus rare, about one living world in three).
