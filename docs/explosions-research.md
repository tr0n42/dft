# Big explosions: research and a plan (26 Nov 2026)

Benjamin asked how to make large shockwaves cheap, since the sapper is the
testbed for bigger destruction later. This note covers what other games and
the physics literature do, where our blast's time goes today, and a plan in
order.

## Where the time goes today

`Sim::sapper_blast` does everything in one tick. Measured headless on a 3 km
barren world (`tests/sapper_blast.rs`, `blast_timings`, run with
`cargo test --release --test sapper_blast -- --ignored --nocapture`).
Headless counts everything as on screen, so these numbers are the worst
case; in the game a 10 kt blast's step was about 35 ms.

| Yield | All | Rays | Pieces sorted | Underground throw | Column | Crater | Cracks |
|---|---|---|---|---|---|---|---|
| 10 kt | 136 ms | 4 | 1 | 23 | 8 | 90 | 10 |
| 100 kt | 273 ms | 18 | 4 | 20 | 25 | 147 | 56 |
| 1,000 kt | 871 ms | 83 | 11 | 32 | 88 | 354 | 301 |
| 3,000 kt | 1,552 ms | 177 | 17 | 36 | 140 | 523 | 654 |
| 10,000 kt | 3,237 ms | 409 | 95 | 61 | 306 | 845 | 1,514 |

At 10,000 kt it touches about 200,000 broken cells, 400,000 cells of loosened
column and 218,000 cracked cells, one by one, in a single tick.

- **Drawing is not the limit.** With the caps lifted to 150,000 pieces, frames
  stayed under 6 ms. The limit is this one tick of simulation work.
- **Crater (90 ms at 10 kt).** Almost all of it is ground-height lookups: about
  330 calls to `ground_at`, about 0.5 ms each. Each walks down cell by cell
  through untouched air, and every untouched cell is worked out from the
  generator. Caching one lookup per column, and starting the walk near the
  known ground, should take it to a few ms.
- **Cracks.** It visits every cell of a big box round the blast (8 million
  samples at 10,000 kt), with a block lookup per cell.

## What others do

- **Noita** (falling sand). An explosion casts rays from its centre. Each ray
  has "ray energy" that it spends on the HP of each pixel it passes,
  according to the material's hardness, until it runs out. A durability cap
  makes some materials immune. The world is split into 64 x 64 chunks, each
  with a "dirty rectangle" of pixels that still need simulating, so only
  active areas cost anything. Our rays already work this way, with energy
  spent by hardness. [Noita wiki: Durability](https://noita.wiki.gg/wiki/Durability),
  [Hardness](https://noita.wiki.gg/wiki/Hardness),
  [GDC talk](https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design),
  [talk notes](https://braindump.jethro.dev/posts/gdc_vault_exploring_the_tech_and_design_of_noita/)
- **Cellular-automaton pressure** (Tom Forsyth). A blast is a large pressure
  value placed in one cell and spread cell to cell, so confined spaces hit
  harder than open ones. Forsyth notes that real shocks move at the speed of
  sound, and that how often each cell updates can depend on its state. This
  is close to Benjamin's frame-by-frame shockwave idea.
  [Cellular Automata for Physical Modelling](https://tomforsyth1000.github.io/papers/cellular_automata_for_physical_modelling.html)
- **The Powder Toy** keeps air pressure and velocity on a separate grid that
  is coarser than its particles, and particles read it.
  [Powder Toy](https://www.bryanbraun.com/2021/01/18/the-powder-toy)
- **Arrival time with fast marching.** Fast marching computes when a front
  reaches each cell of a grid where the speed differs from cell to cell, in a
  single Dijkstra-like pass. With speed taken from the rock, that gives a
  shock that runs along soft layers and is stopped by caves.
  [survey](https://arxiv.org/pdf/1506.03771),
  [heterogeneous media](https://doi.org/10.1007/s10596-023-10204-7)
- **Voxel destruction** (Roblox and Teardown-style projects). Simulation, not
  drawing, is the usual bottleneck. Teams chunk the world, spread the work
  over frames with a queue, freeze debris once it settles, and use bigger
  pieces for big blasts.
  [dissertation](https://gradex.staffs.ac.uk/wp-content/uploads/2025/05/5301_Aiden-Knight-Dissertation-pdf.pdf),
  [Roblox thread](https://devforum.roblox.com/t/how-to-do-optimized-voxel-destruction/2918590)
- **Pre-broken pieces and a stress graph.** Red Faction: Guerrilla breaks
  buildings into pieces ahead of time and collapses them by stress
  ("precomputed destruction"). NVIDIA Blast joins pieces with bonds in a
  "support graph" and solves stress over the bonds: a bond breaks when its
  pressure beats the material's strength.
  [Geo-Mod 2.0](https://www.redfactionwiki.com/wiki/GeoMod_2.0),
  [Blast stress solver](https://docs.nvidia.com/gameworks/content/gameworkslibrary/blast/1.1/api_docs/files/pageextstress.html)
- **GPU falling sand.** Block rules (Margolus neighbourhoods) let a GPU update
  very large fields in parallel. Not needed while drawing isn't our limit.
  [falling-sand projects](https://github.com/PieKing1215/FallingSandEngine),
  [Metalrain](https://metalraindev.itch.io/metalrain-gpu-tests)

## What the physics says

- **Underground blast zones** grow as the cube root of the yield: a melt
  cavity of 4 to 12 m/kt^1/3, crushed rock to 30 to 40 m/kt^1/3, and cracked
  rock to 80 to 120 m/kt^1/3. The cavity roof then falls in, making a rubble
  chimney 4 to 6 cavity radii tall. Ours today: broken to about 14 m/kt^1/3
  and cracked to about 31. The cracked zone could be 3 times wider, which is
  affordable if cracks are decided per block (step 2).
  [Underground nuclear testing](https://www.Wikipedia.org/wiki/Underground_nuclear_test),
  [Rainier and Neptune geology](https://www.osti.gov/biblio/4171913)
- **Ejecta** (Housen and Holsapple's point-source scaling). Launch speed falls
  as a power of the distance from the centre: fast near the middle, slow at
  the rim. Too little mass flies fast in their simple law near the centre,
  per later work. The ejecta blanket thins steeply outside the rim (about
  r^-3, from general knowledge, not checked here). Ours: one speed, slower
  with depth.
  [ejecta scaling](https://arxiv.org/pdf/2006.00732),
  [erosion by cratering](https://arxiv.org/pdf/2104.04981)

## Plan, in order

1. **Quick wins (small).** Cache the crater's ground heights (90 ms down to a
   few at 10 kt). Decide cracks per block rather than per cell: one test per
   Voronoi block, cells visited only along block edges. Expect the blast step
   to drop several times at every yield.
2. **The shock as a moving front (the big one).** This is Benjamin's idea. A
   blast becomes a job that advances each tick:
   - Each tick, the front moves out a shell (say 100 to 300 m a second, for
     looks).
   - The rays step only as far as the front, and the rock in that shell is
     broken, cracked or loosened then.
   - The crater (already queued) and the column follow behind it.
   - A fixed time budget per tick (2 to 3 ms) bounds the cost at any yield,
     and the shell the screen sees is done first.
   - It also looks right: the cracks race outward with the ring the game
     already draws.
3. **Zones as shapes, not lists of cells.** The column, the cracked zone and
   the loose zone are discs and wedges round a blast, so keep them as shapes
   and test a cell against them when asked. Write per-cell entries only where
   something actually changes (a cell cut, moved or drawn differently). At
   10,000 kt that saves writing about 600,000 cells, the `loose` and
   `fractured` sets stay small, and saves shrink.
4. **Clumped ejecta past 20,000 pieces.** One flying piece carries 4 to 16
   cells' mass and drops a small pile where it lands, sized for the zoom (at
   2 m a pixel, single cells are invisible anyway). This lifts the crater cap
   without more pieces in the air.
5. **Look and physics (later).**
   - Wider cracked zone (80 to 120 m/kt^1/3).
   - Ejecta speed falling off with launch distance, so the middle is fastest.
   - The held big chunks thrown as rock bodies (the game already has falling
     rock bodies), rather than as loose cells.
   - A shock that runs along soft layers and is stopped by caves (fast
     marching on a 4 m grid).
   - Rock over the cavity falling in after the blast (a rubble chimney).

Steps 1 and 2 together should let any yield go off without a hitch. Step 3
keeps memory and saves sane for planet-scale blasts.
