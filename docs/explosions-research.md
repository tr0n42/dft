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

## Step 4 and the ejecta speed (27 Nov)

Done: clumped ejecta, the crater cap lifted, and the ejecta speed falling
off with distance from the middle (one item of step 5).

- **Clumps.** A crater of up to 4,000 cells (`CRATER_PIECES`) still flies
  cell by cell, as before (10 and 100 kt look as they did). Past that, the
  cells that go in the same tick are grouped by small squares of the crater
  (2, 3 or 4 m across, in its own frame: across it and down from its top),
  and each square flies as one drop carrying up to `CLUMP_MAX` = 24 cells.
  The drop keeps each cell's own material, so the books balance cell by
  cell; it lands as a small pile round where it hits (the lowest open cells
  of each ring first; settling takes it on). The game draws a clump
  as its own cells in a little square, each in its own material. Drops
  alone (with no rigid body in the air) were not drawn at all before; now
  they are.
- **Cap.** `CRATER_MAX` went from 20,000 to 250,000 cells. The bowls come
  out at 15,000 (1000 kt), 26,000 (3000 kt) and 50,000 cells (10,000 kt),
  so none is cut short any more. A big crater is lifted over
  cells / 40,000 game seconds (`CRATER_LIFT_PER_S`), not always 0.6 s, so
  the lifting stays a few thousand cells a tick. Its planning gets what the
  front leaves of the budget too, and the one-off passes over all its cells
  are spread over ticks (the top layers picked with a selection, not a sort).
- **Speed.** (As first built; changed after filming, see "The throw put
  back" below.) Launch speed was `EJECTA_RIM` x sqrt(pull x half-width) at the
  rim, rising as (distance from the middle)^-1.7 inward, up to the blast's
  own speed (35 x power) in the middle; deeper ground slower, as before. The
  elevation rule (steeper than the wall beside it) is unchanged, except
  that it is kept under 80 degrees over the true level. Every piece gets at
  least the speed that carries it 10% past its rim over the ground as it
  lies (on a hillside, the slow rule alone dropped uphill ground back into
  the crater). Each piece starts as far along its path as it lay deep under
  its column's top, so a column leaves as a stream, not as one sheet
  hanging over the hole (slow ground thrown a tick apart from one mouth).
- **Bugs found on the way.** A drop that reached the edge of the world's
  grid stopped there, so a big crater's fast ground was left heaped at the
  edge of the grid, in space (at 1000 kt, all 14,000 pieces; this is why
  "0% left the body" before). Now it flies on and is lost past three radii.
  Out of sight, a drop now comes down where its path round the body
  (an ellipse) brings it back, and is lost if it would never come back or
  would rise past three radii, as in sight (it used to come down at most
  half a radian away, a big crater's fast ground in two towering heaps).
  In sight, a drop well over the ground going faster than escape speed is
  lost at once instead of after minutes of drifting out (it held a place
  among the 20,000 drops meanwhile). Out of sight drops are set down at
  most 120 a tick (moving the view off a big crater was a 200 to 400 ms
  tick). Drops are now saved (save version 17), so a crater in the air
  survives a quit and load; older saves still load. Rock bodies in the
  air are still dropped by a save, and a wrecked drone's scrap in the air
  is no longer booked as lost by it (it was never on the books). A clump
  lands only in open cells joined up with where it hit (it could fill a
  tunnel under a thin roof). Well over the highest ground, a drop's path
  is no longer looked along cell by cell.

Headless, `blast_timings` (3 km barren test body, pull 0.0033 m/s2,
escape 4.5 m/s), over the blast and the minute after it (the "after"
figures with the review fixes):

| Yield | Worst tick before | After | Of it: flying before / after | Crater cells before / after | Depth planned / middle after | Pieces in flight before / after |
|---|---|---|---|---|---|---|
| 10 kt | 62 ms | 69 | 0.2 / 0.2 | 935 / 935 | 16 / 16 m | 862 / 862 |
| 100 kt | 83 | 55 | 2.8 / 0.3 | 3,541 / 3,541 | 35 / 54 | 3,461 / 3,461 |
| 1,000 kt | 110 | 107 | 7.2 / 0.8 | 15,346 / 15,346 | 76 / 139 | 14,392 / 4,680 |
| 3,000 kt | 120 | 105 | 66.5 / 1.4 | 20,000 / 25,962 | 110 / 101 (77 before) | 19,995 / 3,560 |
| 10,000 kt | 123 | 110 | 92.2 / 1.2 | 20,000 / 50,086 | 164 / 161 (58 before) | 19,987 / 3,750 |

Where the crater's ground ends up, and moving the view away a minute on
(what is still in the air then set down or lost, 120 a tick):

| Yield | Left the body within the minute | In the end | Still flying a minute on | Set down out of sight: ticks, dearest tick |
|---|---|---|---|---|
| 10 kt | 13% | 47% | 803 of 862 | 7, 40 ms |
| 100 kt | 10% | 46% | 3,158 of 3,461 | 27, 51 ms |
| 1,000 kt | 14% | 55% | 4,005 of 4,680 | 34, 62 ms |
| 3,000 kt | 17% | 64% | 2,946 of 3,560 | 25, 24 ms |
| 10,000 kt | 25% | 64% | 2,761 of 3,750 | 24, 39 ms |

So on a body this small about half the crater, the fast middle, leaves the
body for good (everything thrown inside about a quarter of the half-width
is past escape speed, 4.5 m/s here). That is what happens on a real
asteroid. The figures first given for this step (1 to 10%) counted only
what had already got three radii out within the minute. Before this step
all of it was thrown past escape speed (35 x power), so on screen it all
left in the end. If a sapper should waste less of a small body, the
middle's speed can be held under escape speed for most of the bowl; a
decision for Benjamin.

The worst ticks left are not mostly the blast's: 40 to 80 ms of them is
the swarm's once-a-second work that happens to fall in them. At the 30 s
mark that includes the specialists' scans (the tunneler's among them),
which grow with the ground a blast opened: about 41 ms with no blast, 79
ms after a 10,000 kt one. Those scans are not budgeted yet. The blast's own
share is the settling of its rubble, about 21 to 25 ms a tick
(`SETTLE_PER_TICK`), and its front's work, at most 11 to 25 ms.

On the 5 km dev planet (pull 0.0048 m/s2; measured before the review
fixes), five game minutes after a 10 kt blast on dry ground: the crater 41 to 60 m deep in the middle, a blanket of
1 to 4 m from about 1.2 to 3 half-widths out, nothing past that; 527 of 862
pieces still in the air. At 1000 kt: 2,200 of 4,700 pieces still in the
air, the blanket 2 to 6 m just past the rim, 5,700 t lost to space. On a
body this small a slow throw is slow to come down: a piece from the rim is
up about two minutes, one from halfway in about seven. That is the body's
real pull; a faster blanket would need a pull the body doesn't have.

### The throw put back (27 Nov, after filming)

Filmed on the dev planet, the slow rim read badly: a 10 kt crater's
ground rose in two thin streams that hung over the crater for the whole
90 s film, like smoke, and a 10,000 kt one stood as two curtains for over
a minute. The fast fan Benjamin approved on 26 Nov was put back, gentler
at the rim: the middle at the blast's speed (35 x power), falling to
`EJECTA_RIM_SHARE` = 0.45 of it at the rim as (1 - f)^1.5, deeper ground
slower, never slower than clears the rim. On these small bodies that is
all past escape speed, so the crater's ground leaves the body, as it did
before step 4 (but now truly lost, not heaped at the grid's edge). The
clumps, the lifted cap, and the fixes to flight and saves stay.

**For Benjamin:** honest physics (a blanket round the crater that takes
minutes to come down on a small body, half the crater kept) or the fast
fan (all of it thrown off a small body for good, in a few seconds)? A
middle way would be to keep the fan's look but hold the rim under escape
speed, so the outer part comes back down later, out of sight of the blast.

