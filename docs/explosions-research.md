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
   - Wider cracked zone (80 to 120 m/kt^1/3). (Done 27 Nov, see below.)
   - Ejecta speed falling off with launch distance, so the middle is fastest.
   - The held big chunks thrown as rock bodies (the game already has falling
     rock bodies), rather than as loose cells. (Done 27 Nov, see below.)
   - A shock that runs along soft layers and is stopped by caves (fast
     marching on a 4 m grid).
   - Rock over the cavity falling in after the blast (a rubble chimney).
     (Done 27 Nov, see below.)

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

## A wider cracked zone (27 Nov)

The shock now cracks rock out to about 80 m/kt^1/3, as underground tests
do, where it stopped at 45 to 50 before. Past the rock cracked all through
into small blocks (`CRACK_REACH`, 1.2 radii past where each ray stopped), a
thin far zone runs `CRACK_FAR` = 3.2 radii further:

- **Bigger blocks, fewer of them.** Three patterns of blocks for its inner,
  middle and outer thirds, 10, 14 and 20 m across at 10 kt and bigger as
  the square root of the blast radius (32, 44 and 63 m at 10,000 kt). A
  block cracks at most 75% of the time at the inner edge, less in softer
  rock, and never at the outer edge. (Changed after review: it now starts
  inside the near zone and more thinly; see "Review fixes" below.)
- **Only the cracks themselves.** In a block that cracks, only some of its
  edges are cracks (90% at the inner edge, 40% at the outer), and only those
  cells are cracked and drawn. The rock between them holds, as real far
  cracks do. Each edge is bent a little at its middle.
- **Decided a block at a time.** The near zone looks at every cell. Out
  here that would look at ten times as many cells as it cracks, so each
  block is decided once, when the front has passed it and its neighbours.
  Its outline is worked out from its neighbours' sites, and only the cells
  along the edges that crack are looked at. All the work is charged to the
  blast's budget.
- **A fix on the way.** Past the column's cracked band, the near zone still
  walked every row of each shell while the front ran on. That cost the last
  tick of a 10,000 kt blast 20 ms. It now stops there, with the same result.

Measured headless with `blast_crack_reach` (3 km barren test body; run with
`cargo test --release --test sapper_blast -- --ignored --nocapture`).
"Reach" is measured below the charge. Over it, the cracks beside the
loosened column run on up to the ground.

| Yield | Cells cracked before / after | Drawn as cracks before / after | Reach (farthest) before / after | Blast ticks before / after | Blast work in all before / after | Dearest tick before / after |
|---|---|---|---|---|---|---|
| 10 kt | 4,636 / 6,304 (1.36x) | 1,861 / 3,529 | 43 / 74 m/kt^1/3 | 9 / 15 | 43 / 47 ms | 25.6 / 25.7 ms |
| 1,000 kt | 56,875 / 66,312 (1.17x) | 21,483 / 30,792 | 49 / 83 | 60 / 63 | 394 / 435 ms | 10.7 / 10.8 ms |
| 10,000 kt | 211,066 / 228,838 (1.08x) | 78,228 / 96,003 | 50 / 87 | 231 / 240 | 1,822 / 1,930 ms | 13.9 / 12.5 ms |

The dearest 10 kt tick is the first one, when the crater is planned. That
is unchanged.

The cracked cells (which also go into saves) grow only 8 to 36%, well under
the 2 to 3 times allowed. At 10,000 kt on this small body, most of the far
zone is open space past the curve of the ground, so it adds little there.
At 10 kt the front now runs for 1.5 s instead of 0.9 s. A far block is
decided only once the front is past it and its neighbours, so the far
cracks appear a little behind the shock ring. If the far zone should show
more, `FAR_SHARE`, `FAR_EDGE` and `FAR_SIZES` in `src/units/sapper.rs` set
how thick it is.

## The rubble chimney (27 Nov)

After a real buried blast, the roof of the cavity falls in and leaves a
chimney of rubble over it. A sapper's blast already loosens a column up to
the surface, wider than any chimney, so nothing changes there. A blast
with no column now gets a chimney. Today that means a tunneler's 50 kt end,
or a sapper on the old depth rule.

- **How tall.** (After review: never closer than a melt radius under the
  ground; see "Review fixes" below.) At least 6 melt radii over the charge (`CHIMNEY_TALL`). The
  melt radius is the molten heart's, about 4 m/kt^1/3; real chimneys rise 4
  to 6 cavity radii, and our melt radius is at the small end of real
  cavities. In soft rock the rays already break the ground out to nearly 2
  blast radii, past that height (1.7 to 1.9 radii straight up on the test
  body). So the chimney also rises at least 2 melt radii (`CHIMNEY_OVER`)
  past where the rays going straight up stopped (the middle of those within
  15 degrees of up).
- **Shape.** 2.5 melt radii either side at the charge, narrowing to 1.5 at
  the top, with the top fifth rounded off and the edges frayed. Real
  chimneys are about as wide as the cavity, which is about 2 of our melt
  radii.
- **When.** It starts falling in 1 s after the blast (`CHIMNEY_DELAY_S`).
  It climbs from the top of the broken ground to the chimney's top over 1
  s (`CHIMNEY_CLIMB_S`), a 0.5 m band at a time. It waits until the rays
  are all done and the front has passed all of it, so the rays and cracks
  come out exactly as before. The work is charged to the blast's budget, so
  a big blast's chimney falls in at the budget's pace. A save finishes it
  at once along with the rest of the blast.
- **What it does.** Each solid rock cell in it that isn't already broken or
  loose is left loose where it is, drawn as rubble, and put in settling, so
  it slumps into whatever opened under it: the tunneler's own shaft, the
  cells the blast threw, or melt that ran off. Nothing is moved or made,
  so the books are unchanged (the new test
  `a_blast_with_no_column_leaves_a_rubble_chimney_and_keeps_the_books`
  checks them every 3 ticks and through a save).
- **No uranium in it.** The unreacted uranium is still left only through
  the blast's own rubble (the rock its rays broke), and the plume still
  blows only that rubble out. The chimney is rock from over the cavity
  that falls in later. It never saw the fireball, so it gets none. The
  doping also runs before the roof falls in, so it can't depend on how far
  the chimney has got.

Measured headless with `blast_chimney_reach` (3 km barren test body, a
charge with no column at the old rule's depth, a blast radius + 50 m:
80 m at 10 kt). "Before" has the chimney switched off. "Over the charge"
is the highest loose cell within a quarter radius of straight up. At
1,000 and 10,000 kt the charge (190 and 350 m deep) is under less ground
than the chimney's height, so it reaches the surface there, and the
figure is the rubble lying on the ground.

| Yield | Loose cells before / after (chimney) | Over the charge before / after | Blast ticks before / after | Blast work in all before / after | Dearest tick before / after |
|---|---|---|---|---|---|
| 10 kt | 6,588 / 7,037 (+449) | 1.68 / 2.24 radii (24 / 31 m/kt^1/3) | 15 / 20 | 59 / 60 ms | 30 / 30 ms |
| 50 kt | 16,972 / 17,902 (+930) | 1.82 / 2.23 radii | 24 / 24 | 41 / 40 ms | 18 / 17 ms |
| 1,000 kt | 90,129 / 95,730 (+5,601) | surface | 58 / 58 | 291 / 294 ms | 86 / 89 ms |
| 10,000 kt | 284,787 / 304,544 (+19,757) | surface | 132 / 140 | 1,227 / 1,256 ms | 136 / 135 ms |

These figures are from before the review. After it, the chimney stops a
melt radius under the ground, so the loose cells at the end are 7,050 /
17,902 / 95,383 / 290,754, and the loose ground over the charge reaches
1.90 and 1.76 radii at 1,000 and 10,000 kt instead of the surface. The
plume now goes on the budget too, so the 10,000 kt blast runs 182 ticks
(about 4 s more) and 1,490 ms of work in all, but no tick of it costs
more than 16 ms.

At 10 kt the blast job now runs 2 s instead of 1.5 s, because it waits
for the roof. The chimney's own work is lost in the noise up to
1,000 kt, and about 30 ms in all, spread over the ticks, at 10,000 kt.

**Found, not fixed** (fixed after review; see "Review fixes" below). The dearest tick of a blast with no column is its
plume, not the chimney: `way_up` searches up to 40,000 cells for a way to
the surface and looks up the ground (`ground_at`) as it goes, outside the
budget. It took 28 ms at 10 kt, 78 ms at 1,000 kt and 123 ms at
10,000 kt. A sapper's blast with a column throws a crater instead, so it
never runs this. The 50 kt blast on the test body happened to make no
plume (1 blast in 10 makes none), so it cost nothing there. A tunneler's
blast that does make one would cost something between the 10 kt and
1,000 kt figures.

## Held chunks thrown whole (27 Nov)

Hard rock that a blast holds together in big pieces (`Look::HELD`) used to
stay where it was, or, in the crater's ground, fly out cell by cell with
the rest of it. Now, on screen only, up to `CHUNKS_MAX` = 24 pieces a blast
fly out whole as tumbling rock bodies, the same bodies that overhangs make
when they break off.

- **Only rock harder than 6 is ever held** (granite, gabbro and
  anorthosite are 6; quartzite, olivine and iron-nickel are 7). That rule
  was there before. So on the barren test body and the dev planet's
  sediments and granite, a blast holds nothing and nothing changes. The
  numbers below are from the 3 km test body with its crust's rock made
  hardness 8 (in the tests only).
- **Pieces in the crater go first.** The crater lies far above where the
  rays reach, so the only thing that holds rock there is the column's
  rule, which depends only on the cell, its rock and the blast
  (`column_piece`). So the crater can tell, while it is planned, which of
  its cells are held and in which piece. A piece with at least
  `island_min_cells` (8) cells in the crater flies whole (after review:
  each joined part of 8 to 120 cells, when its lowest layer goes; see
  "Review fixes" below) when its top layer goes, as the ground over it has gone by then. It flies from where it
  lies, the way the ground there flies, at `CHUNK_SHARE` = 0.2 of the
  crater's middle speed. Bigger pieces go slower and smaller ones faster
  (60 cells is the middle), with some spin. The rest of the crater flies as
  before.
- **Then the pieces left behind.** Once the rays are done, the column is
  loosened and the crater is all out, the held pieces left are gone
  through highest first. One goes if the biggest joined part of it still
  held is at least 8 cells, some of its sides face open space (the crater,
  the cavity, the sky) and it can move 4 m (`CHUNK_CLEAR`) in a straight
  line without touching anything. The way it tries first is out from the
  charge, turned towards its open side. Then it tries the open side
  itself, that turned up to 60 degrees either way, and straight up. It
  never goes back towards the charge. (A piece sticking out of the crater's
  sloping wall is open on its side, but slid that way it hits the slope
  under it.) This work is charged to the blast's budget.
- **The books.** Each piece is lifted out of the grid with `lift_cell` and
  becomes a body through the same code as a chunk that breaks off
  (`Bodies::throw_chunk`). It is booked as in the air until it lands (as
  rubble), is grabbed, or leaves the body. If the view has moved off it,
  or 400 small bodies are already in the air, a crater piece flies in
  clumps like the rest of the crater instead. A save (`finish_blasts`) throws none of the pieces
  left behind; they stay held. The new test
  `held_pieces_are_thrown_out_whole_and_keep_the_books` checks the books
  every 3 ticks, through a save with pieces in the air, and when the view
  leaves them.

Measured headless with `blast_chunks` (the hard-rock body, over the blast
and the minute after it; "before" has both throws switched off). The counts
are the same every run.

| Yield | Pieces held | Thrown whole (crater + left behind) | Cells in them | Most rock bodies in the air before / after | Blast work in all before / after | Dearest step before / after | Dearest flying before / after |
|---|---|---|---|---|---|---|---|
| 10 kt | 32 | 6 (6 + 0) | 264 | 1 / 6 | 41 / 42 ms | 68 / 62 ms | 1.1 / 2.8 ms |
| 100 kt | 135 | 24 (19 + 5) | 2,202 | 2 / 24 | 104 / 103 ms | 73 / 78 ms | 3.8 / 4.5 ms |
| 1,000 kt | 368 | 24 (24 + 0) | 3,369 | 3 / 24 | 430 / 429 ms | 92 / 96 ms | 12.0 / 10.2 ms |
| 10,000 kt | 1,142 | 24 (24 + 0) | 3,058 | 53 / 59 | 1,899 / 1,897 ms | 115 / 107 ms | 11.1 / 23.1 ms |

The blast's own work hardly changes. The cost is in moving the bodies: at
10,000 kt the dearest tick of flying doubled to about 23 ms, with 59 bodies
in the air (about 35 of them chunks that broke off by themselves, as
before). The pieces fly at about 5 to 17 m/s at 10 kt and 18 to 65 m/s at
10,000 kt. On this small body that is past its escape speed of 4.5 m/s,
so most are still drifting away a minute on, as the crater's ground does;
on a body with more pull, more would come back down. At 10 kt the crater
swallows most of the pieces near the surface, and those left behind are
all shut in by rubble or can't move 4 m clear, so only the crater's 6 fly. If chunks should fly in common rock too, the rule that rock must be
harder than 6 to be held (`blast_broken`, `column_piece`) is the thing to
change; that would change the look of every blast in granite.

## Review fixes for the three above (27 Nov)

A review of the far cracks, the chimney and the thrown chunks found these,
all fixed:

- **Cracks stopped at the charge's level with no column.** The cracks
  stop at the ground over the charge, but with no column that was taken
  as 0 m, so a tunneler's blast cracked only the rock under its charge: a
  crack field cut off in a straight line, and a chimney rising into
  uncracked rock. The ground over the charge is now looked up for every
  blast. With no column, the cracks over and under the charge now lie
  about as thickly (10 kt, 8 blasts, drawn cracks per square metre 2.5 to
  3.5 radii out: 0.037, 0.023, 0.030, 0.024 under it; 0.039, 0.021, 0.022,
  0.020 over it, where over it was 0 or 1 cell a ring before).
- **A thin ring, then a thicker ring of cracks outside it.** The near zone
  faded to nothing, then the far zone started thick. The far zone now
  starts 0.4 of the way into the near zone (`FAR_FROM`) and comes in
  gradually over the near zone's fading end, with fewer blocks cracking
  (`FAR_SHARE` 0.2 at 10 kt, more for bigger blasts, as their blocks are
  bigger) and fewer edges (`FAR_EDGE` 0.7), thinning slowly outward.
  Drawn cracks per square metre below the charge, in rings a quarter
  radius wide from 2.25 radii out, at 10 kt (new ignored test
  `blast_crack_profile`): before (one blast) 0.072, 0.046, 0.066, 0.115,
  0.132, 0.123, 0.094, 0.048; now (averaged over 8 blasts) 0.108, 0.044,
  0.028, 0.027, 0.021, 0.024, 0.019, 0.014. At 1,000 kt (3 blasts) it now falls steadily too;
  the single 10,000 kt blast still has one small bump (0.017 then 0.025),
  within what one blast's big blocks vary by.
- **Far blocks judged twice could disagree.** A block's verdict is now
  kept the first time it is asked, so the crater lifting its site later
  can't change it.
- **The chimney reached the surface.** With nothing empty under it to
  sink into, that only turned a strip of surface to rubble, which then ran
  down slopes. It now stops a melt radius under the ground. It also waits
  until the plume has found its way up, so a save (which finishes a blast
  at once) finds the same way as play does.
- **The plume was the dearest tick of a blast with no column.** Its
  search for a way up looked up the ground about 50 to 160 times, each
  about half a millisecond, all in one tick, then went through all the
  rubble and lifted up to 500 cells at once. Now each ground lookup starts
  from the ground found beside it, and the search, the pass over the
  rubble and the lifting all go a budget's worth per tick. The stream's
  timing is counted from when the plume began, so it looks the same. The
  dearest blast tick with no column (`blast_chimney_reach`): 10 kt 30 ->
  22 ms (now the first tick, the front's own), 50 kt 18 -> 18, 1,000 kt
  89 -> 13, 10,000 kt 135 -> 16.
- **Crater pieces thrown too early, and in halves.** A held piece in the
  crater flew when its top layer went, sideways into the ground still
  standing beside its lower part. It now goes when its lowest layer goes.
  A piece cut by the sapper's shaft flew as one body with a 7 m gap in
  it; each joined part now goes on its own (new test
  `held_pieces_cut_by_a_shaft_fly_joined_up`). The held cells that don't
  go whole now fly in their normal clumps (they had been taken out of
  them and flew one by one, up to 10,000 drops at once at 10,000 kt). If a
  piece can't become a body when its time comes, its cells fly each as it
  would have with the crater.
- **Pieces left behind thrown across narrow gaps.** A piece must now be
  able to move clear as far as it flies in its first 0.3 s, and at least
  its own size, not just 4 m.
- **Pieces bigger than a tumbling body may be.** None over
  `island_max_cells` (120) is thrown whole now. (Moving them as slabs, as
  big break-offs move, was tried: a slab sheds a piece every so often as
  it moves, and a fast one drifting off a small body for minutes shed
  hundreds, filling the 400 bodies the game allows, with 147 ms ticks of
  flying at 10,000 kt.) So fewer and smaller pieces fly at big yields.
- **Smaller fixes.** Pieces left behind are judged only once the
  crater's last layer is out (one tick late). Rock that broke free as a
  body stayed listed as loose and cracked (up to 206 empty cells after a
  10 kt blast in hard rock); it is now taken off both lists.

Measured headless (3 km barren test body; held pieces on the hard-rock
copy), before the review fixes / after:

| Yield | Cracked cells | Farthest crack below the charge | Pieces thrown whole (cells) | Most rock bodies in the air | Dearest flying tick |
|---|---|---|---|---|---|
| 10 kt | 6,304 / 4,947 | 74 / 69 m/kt^1/3 | 6 (264) / 5 (254) | 6 / 5 | 2.8 / 3.0 ms |
| 100 kt | | | 24 (2,202) / 15 (771) | 24 / 17 | 4.5 / 6.2 ms |
| 1,000 kt | 66,312 / 61,141 | 83 / 88 | 24 (3,369) / 11 (503) | 24 / 14 | 10.2 / 10.9 ms |
| 10,000 kt | 228,838 / 224,632 | 87 / 87 | 24 (3,058) / 21 (1,312) | 59 / 56 | 23.1 / 26.1 ms |

`blast_timings` on the plain body is as before: the blast's own work at
most 11 to 13 ms a tick from 100 kt up (24 ms at 10 kt, its first tick,
planning the crater; it read 15 to 26 ms across runs before too), and the
crater the same.

**For Benjamin:** at 1,000 kt and up, most held pieces are bigger than
120 cells, so only the smaller ones fly whole. Cutting the big ones into
a few pieces each would let more fly; say if that is wanted.

