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
- The game opens a borderless fullscreen window by default; pass
  `--windowed` for a window. The stockpile window's collapse arrow is at
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

## What's next

- **Production phase 1** (Swarm Production Design): the robot-lore parts
  catalogue in a data file, a parts stock, unit recipes in parts, a Parts
  tab in the stockpile window. Then refiner and fabricator ships (phase 2),
  factories and the director's hidden queue (3), energy (4), heat-sink bots
  (5), tunnelers (6).
- Open questions for Benjamin: which generation-bending perks go in the
  meta tree first; planet ranking for selection (parked until he decides).
- Also waiting: map icons for the zoomed-out view, life L1e, the lava flow
  bug (lava not running into hollows), warm ground and decay with
  radioactivity (technetium, polonium, astatine and francium are on no body
  until then).
