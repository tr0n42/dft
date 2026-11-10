# The Dark Forest Theory: handoff for a new session

Read this first, then ROADMAP.md (what's built and what's next) and README.md
(how the code is laid out). Last updated 10 November 2026.

## Working with Benjamin

- He is not a programmer: Claude makes every technical decision and writes
  all the code. He tests on his Windows PC and reports what he sees (often
  with videos or screenshots).
- He runs things by double-clicking `.bat` files, never from a command
  line: every test or run needs its own `.bat` with no arguments, showing
  live progress, not popping over his other work.
- He wants honest numbers, frequent short progress messages during long
  work, and plain words.
- The project lives on his PC in `Downloads\DarkForestTheory-source`, a git
  repo pushed to https://github.com/tr0n42/dft. Since 10 November 2026 the
  work happens in Claude Code, running on his PC in that folder (see
  CLAUDE.md). Rust stable (MSVC) is installed there.

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
