# The Dark Forest Theory

An idle game in which a drone swarm eats planets. The simulation core is the Rust crate at the
repo root (`dft-sim`), and the native Bevy game is `game/`. Read HANDOFF.md (how we work, where
things stand), then ROADMAP.md (what's built, what's next). The design docs are in
`docs/design/` (snapshots of the claude.ai docs; see its README for the live links).

## Working with Benjamin

- He isn't a programmer. Claude makes the technical decisions and writes all the code; he tests
  on this PC and reports what he sees.
- He runs things by double-clicking `.bat` files, never from a command line. Every test or run he
  needs gets its own `.bat` with no arguments and live progress, and it doesn't pop up over his
  other work.
- He wants honest numbers, short progress messages during long work, and plain words.

## Building and testing (this PC, Windows, Rust stable MSVC)

Claude Code runs on his PC in this folder, so build and test here directly. The old
cloud-workspace routine (stage files, copy them over, check md5) is gone.

- Sim crate: `cargo build --release`. Tests: `cargo test --release --no-fail-fast`, which takes
  about 25 minutes, so run it in the background. `run-tests.bat` runs it group by group into
  `test-results.txt`.
- Game: `cd game && cargo build --release --ignore-rust-version`, or `run-game.bat`.
- A generator change changes every body's fingerprint. Once the change is intended, re-lock with
  `BLESS=1 cargo test --release --test reference` (in PowerShell: `$env:BLESS=1; cargo test ...`).
- Release builds use fat LTO and one codegen unit, so the first build is slow. Use
  `cargo check --release` for quick compile checks.
- Data lives in `data/*.toml`. Some tables are generated (`tools/catalogue.py`,
  `tools/recipes.py`, `tools/tune_scarcity.py`), so edit the script, not its output.

## Git

GitHub remote `origin` = https://github.com/tr0n42/dft, branch `main`. Commit and push only when
he asks. Build output, logs, saves, `out/` and run reports are ignored (`.gitignore`).
