# The Dark Forest Theory: handoff for a new session

Read this first, then ROADMAP.md (what's built and what's next) and README.md
(how the code is laid out). Last updated 12 November 2026.

## Working with Benjamin

- He is not a programmer: Claude makes every technical decision and writes
  all the code. He tests on his Windows PC and reports what he sees (often
  with videos or screenshots).
- He runs things by double-clicking `.bat` files, never from a command
  line: every test or run needs its own `.bat` with no arguments, showing
  live progress, not popping over his other work.
- He wants honest numbers, frequent short progress messages during long
  work, and plain words.
- The project is a git repo pushed to https://github.com/tr0n42/dft. Since
  11 November 2026 it is cloned on his PC in
  `Downloads\DarkForestTheory-source`, and the work happens in Claude Code there (see
  CLAUDE.md). Rust stable (MSVC, aarch64), the VS 2022 Build Tools and
  Python 3.12 (for tools/*.py; `%LOCALAPPDATA%\Programs\Python\Python312-arm64`)
  were installed on 11 Nov.

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

## Where things stand (November 2026)

Most recent work (12 Nov), built and passing the tests, waiting on a PC look:

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
  the tallest plant within 30 m (src/specialists.rs `walker_rise`), feet
  re-found on the ground every second. A full bag is dropped for couriers
  (drawn coming down for it); its contents are credited when they lift it.
- **Sapper blast, ported one for one from the proving ground**
  (src/specialists.rs `sapper_blast`): energy rays, cracks, pieces held
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
- Save version 12 (the world save 6).

Earlier (11 Nov), built and passing the tests, waiting on a PC look:

- **Universal Drone Replicator** (11 Nov, testing stand-in for the meta
  tree; `[units]` in data/economy.toml, src/specialists.rs,
  game/src/units.rs, tests/specialists.rs; test with test-replicator.bat).
  Extra factory bays build the specialists designed in the foundry mockup's
  Units page, only where their skill has work (`Skills`; report for every
  body: `cargo run --release --bin unitcheck`): sappers and tunnelers
  everywhere, heat-sink bots where there is magma or molten metal/sulfur
  (and only once an open pool is found near a flock), foresters where plants
  grow, abductors nowhere (no body has animals yet). Sappers: crawl down
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

## Last session's report (12 November 2026)

When Benjamin asks for a "handoff digestion", repeat this section to him
as it stands.

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

**Open questions:** uranium for sappers (bigger allowance or plutonium?);
then plant panic (species-by-species losses; most species don't react;
exodus rare, about one living world in three).
