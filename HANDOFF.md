# The Dark Forest Theory: handoff for a new session

Read this first, then ROADMAP.md (what's built and what's next) and README.md
(how the code is laid out). Last updated 11 November 2026.

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
  11 November 2026 it is cloned on his PC (Windows on ARM64) in
  `Downloads\dft`, and the work happens in Claude Code there (see
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

Most recent work, all built and passing the tests, waiting on a PC look:

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
- Also waiting: map icons for the zoomed-out view, life L1e, the lava flow
  bug (lava not running into hollows), warm ground and decay with
  radioactivity (technetium, polonium, astatine and francium are on no body
  until then).

## Last session's report (11 November 2026)

When Benjamin asks for a "handoff digestion", repeat this section to him
as it stands (he didn't get to read it at the end of the session).

All the drones are now in the game, built by a Universal Drone Replicator.
To see them, double-click `test-replicator.bat`. It opens a 5 km living
world, which gets every specialist type except abductors, and F1 shows what
each bay is building and what the specialists have done.

**Testing so far:** the full test suite passes: 96 tests, none failing,
including 4 new ones. In headless 20-minute runs, sappers went off and
cracked rock, the tunneler bored shafts, heat-sink bots froze 12,600 t of
magma on a lava world, and foresters took 380 t of plants on a living world.
Nobody has watched any of it on screen yet, so the drawings and animations
are untested until Benjamin looks.

**Which planets get which drones:** cutters, haulers, sappers and tunnelers
are built on every body.

| Specialist | Built on |
|---|---|
| Heat-sink bot | Bodies with magma or molten metal or sulfur: active planetoid, barren, dry, hothouse, iron, lava, living and sulfur worlds, plus the 3 km test planetoid and two demo bodies. It's only built once an open pool turns up near the cutters. |
| Forester | Bodies that grow plants: living, dry, ocean, rogue and ice worlds, and the methane world only at the 5 km test size. |
| Abductor | Nowhere yet: no body has animals. Its recipe and bay are ready for when they exist. |

**How each one acts:**
- **Sapper:** drops where the digging is hardest, crawls down cracks and
  caves (or drills if there are none) and goes off deep. That leaves a
  cavity and a zone of cracked rock that cutters dig in half the time.
- **Benjamin's chaos rule:** anything that destroys a sapper sets it off.
  That includes another blast (so they chain), magma, ground still hot from
  a blast, or rock falling on it. On the lava world, 20 of the 25 sappers
  went off this way.
- **Tunneler:** bores a 12 m shaft and leaves staging points for cutters
  along it.
- **Heat-sink bot:** hovers over a magma pool, grows tentacles down through
  it and freezes it to stone, which powers the swarm.
- **Forester:** a three-legged crab that walks to the thickest plants,
  shears them into its bag and sets the ground cover alight with its laser.

**Parts and costs:**
- **Sapper (157 kg of parts):** needs a nuclear charge, detonator, blast
  casing and eye cluster. The charge needs 8 kg of uranium with no
  substitute.
- **Uranium limits sappers:** plutonium isn't in the element table, and
  other parts are stopped from using uranium as filler. The fleet brings
  200 kg (25 charges), and in 20-minute test runs mining never brought back
  enough for another charge, so sappers stop after 25. Open question: a
  bigger allowance, or plutonium added?
- **Forester (471 kg) and abductor (416 kg):** a multi-job lattice core,
  three legs, a cutting laser and eye clusters. The forester adds shear
  claws, an intake rotor and a bag; the abductor adds grapple whips and a
  stasis sac. These are made from rare elements like rhenium, germanium,
  gallium and tellurium, and can fall back to common ones at a steep cost.

**Also done:** haulers now hover and never land, with six squid tentacles
instead of legs. The published mockups are now in git.

**Two things to know:**
- **Old saves:** this build saves in a newer format (version 11), and saves
  made by it won't load in older builds.
- **Tools:** Python is needed for the parts-pricing script
  (tools/recipes.py); `unitcheck` prints the planet report
  (`cargo run --release --bin unitcheck`).
