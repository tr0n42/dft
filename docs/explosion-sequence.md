# How a blast unfolds: research and a design (27 Nov 2026)

Benjamin, 27 Nov: the crater shouldn't form before the shock reaches the
surface; the shock reaching the surface shouldn't "do a bunch of nothing";
plants are left standing with their roots blown off; the blast should carry
on above the surface (through the air) and blow apart what's there. He
asked for a study of the sequence of events in an explosion and of nuclear
effects (vaporisation, radiation, thermal pulse and so on), and a blast for
both conventional and nuclear charges that follows reality, now that the
performance is there. Flying rock was also drawn at the sim's 10 ticks a
second; that is fixed (7eee2b5).

Sources (three research passes, 27 Nov): Glasstone and Dolan, *The Effects
of Nuclear Weapons* (1977); nuclearweaponarchive.org "Underground Effects"
(Sedan's timeline); Nordyke and Vortman's cratering reports (OSTI); the
Kinney-Graham air-blast fit; USBM blasting vibration rules. Figures marked
(rough) are fits or recollections not checked against the original.

## What really happens (a buried cratering shot)

Scaled depth λ = depth / W^(1/3), in m/kt^(1/3). Our sappers bury at about
80% of the natural depth: 10 kt at 79 m is λ ≈ 37, close to the optimum for
a crater (about 40 to 50).

Sedan (104 kt, 194 m down, λ ≈ 41), the best-recorded shot:

| Time | What happens |
|---|---|
| 0 to 1 ms | X-rays vaporise about 70 t of rock a kt and melt about 700 to 1,100 t. A shock leaves at about 5 km/s. |
| 0.24 s | The shock reaches the surface. The ground over it jumps up at 32 m/s; the reflected tension tears off slabs (spall). A dust ring races out across the ground. |
| 0.45 to 2 s | The spalled ground rises as a dome. The cavity's gas pushes it on to about 40 m/s. |
| just under 4 s | The dome, about 90 m high, bursts. Gas and glowing rock vent: the only "fireball" a buried shot shows. |
| 4 to 15 s | The rock flies on ballistic paths; most falls back into the hole or within two crater radii (the lip, about a quarter of the crater's depth high, and the blanket). |
| 10 s to minutes | A low cloud of dust (and steam) rolls outward along the ground: the base surge, kilometres at Sedan's size. The main cloud rose 3 to 5 km. |
| minutes to days | The lip slumps; deeper shots get a chimney and a subsidence crater later. |

Times scale with W^(1/3) for the early part: at 10 kt the shock reaches the
surface at about 0.1 s, and the dome bursts at about 1.8 s (0.85 W^(1/3)).

What the surface sees from a buried shot: no flash, no heat pulse, no prompt
radiation (each half metre of rock cuts gamma tenfold), and a weak air
blast (5 to 10% of a surface burst's at the optimum depth). The air blast
it does make comes from the ground rising like a piston: about 0.4 kPa per
m/s of rise, so about 13 kPa over Sedan's ground zero. Trees there are
knocked down by the ground being thrown, not by the air.

How the result depends on depth (nuclear, alluvium; hard rock about 0.85x):

| λ' = depth/W^0.294 | Crater radius / W^0.294 | Depth / W^0.294 | Kind |
|---|---|---|---|
| 0 (surface) | 13 m | 6 m | shallow bowl; most energy into the air |
| 5 | 37 | 15 | |
| 20 | 43 | 30 | |
| 45 (optimum) | 47 | 25 | Sedan |
| 70 | 25 | 10 | subsidence |
| 90 and more | none | none | retarc mound, or contained |

Containment is about 92 to 122 m/kt^(1/3). Chemical charges: the same shape
with the optimum about 1.3 times as deep, and nearly the same crater.

## A blast in the open (surface or above)

| Effect | Scaling (W kt, D km) | Thresholds |
|---|---|---|
| Fireball | surface: up to 0.088 W^0.4 km; full size at about 1 s (W/20)^0.4 | inside it everything is vaporised |
| Thermal pulse | peak 0.042 W^0.44 s; 80% out by 10x that. Q ≈ 0.86 W / D² cal/cm² (surface burst) | leaves and grass ignite at about 3 W^0.11 cal/cm²; wood chars at 10 to 15; animals' third-degree burns at about 8 |
| Air blast | Kinney-Graham, ΔP/P0 vs Z = r / W_TNT^(1/3); nuclear W_TNT ≈ 0.5 W, surface 1.8x | trees: 30% down at 20 to 28 kPa, 90% at 35 to 40 kPa; animals: hurt from about 20 kPa, half killed at 280 to 400 kPa (thrown debris kills well before) |
| Prompt radiation | 500 rem at about 1.2 (W/10)^0.14 km; none if buried | lethal to animals; electronics fail at 10³ to 10⁴ rad |
| EMP | surface burst: strong within 3 to 8 km; none if buried | upsets or burns electronics |
| Fallout | 40 to 70% falls locally for a surface burst; cratering shots carry it in the base surge; dose falls as t^-1.2 (10x less for every 7x longer) | |

With no atmosphere: no air blast and no fireball as we know it; most of the
yield leaves as X-rays in straight lines, flash-vaporising the faces of the
ground and machines in sight. On small worlds the horizon (√(2 R h)) cuts
off the flash before distance does.

## Conventional against nuclear

A conventional charge (TNT, RDX, the slurries in `economy.toml`): no
vaporised cavity or melt, no radiation, no fallout, no EMP, a small and
short flash (a few charge radii, milliseconds), and its hot gas does more of
the work, so the dome vents sooner (about half the time) with a strong
second air pulse and fumes. Buried, it makes about the same crater for the
same energy. A shallow charge in many holes (quarry blasting) heaves and
breaks the rock into a muck pile a few metres out at 10 to 30 m/s instead
of throwing it far.

## What our blast does now, and what is wrong

- The game's flash and shock ring are drawn on the wall clock (0.5 s, out
  to about 2.9 blast radii), not from the sim.
- The crater is thrown as soon as the work front reaches the ground's
  height (by the clock, 200 m/s), all at its full fan speed, top layer
  first. There is no spall, no dome and no vent: the hole opens at once.
- After that, the front runs on through the rock for seconds, cracking it,
  with nothing happening above ground.
- Plants are cells. A plant stays standing while anything holds it up, and
  plant cells that never land never wilt, so plants over the blasted
  ground live on. Animals and walkers are never harmed; only cutters and
  haulers are.
- Conventional charges behave exactly as nuclear ones, without the
  leftover uranium.

## The design

One blast routine for every charge, set by its kind (nuclear or
conventional), its yield and its scaled depth. It runs on the sim clock in
the order above. The rock-breaking work stays on the budget as it is now
(steps 1 to 5 of `explosions-research.md`). What is seen is driven by
event times worked out at the start, not by how far the work has got.

1. **Detonation (t = 0).** Underground: the flash and heat glow (in our
   cross-section view the rock is seen, so it stays). Nuclear: the melt
   cavity, a vaporised mass vented later with the gas. Conventional: a
   small crushed zone and no melt.
2. **The shock through the rock** at the rock's own speed (about 1 to 6
   km/s by hardness; a decision below). The game's ring is drawn from the
   sim's front, so the two agree at any game speed.
3. **The shock reaches the surface (t_s).** Over the spall zone (out to
   about 1.5 to 3 times the depth) the ground jumps up at v(r), fastest
   over the charge (Sedan: 32 m/s), falling off with distance. Everything
   on it goes with it: plants are torn out and thrown as bodies, animals
   and drones are flung and hurt or killed. A dust ring runs out across
   the ground. Further out the ground shakes: loose rubble falls and
   plants are shaken, but stand.
4. **The dome (t_s to t_vent).** The ground over the charge rises together
   as a dome, slowly at first (the crater's ground lifted as one, at the
   spall speed).
5. **The vent (t_vent ≈ 0.85 W^(1/3) s nuclear, about half for
   conventional).** The dome bursts. The crater's ground gets the gas's
   push and flies out in the fan (today's fan speeds). Nuclear: a glow of
   hot gas and glowing rock, the vaporised mass let out. Conventional: a
   sharp second air pulse and fumes.
6. **Ejecta and fallback.** As now, with the crater's size and kind
   (crater, subsidence, mound, contained) taken from the scaled depth.
7. **The air blast**, on worlds with air only: overpressure by the
   Kinney-Graham fit, cut by the burial's transmission factor (or worked
   from the ground's rise), running out at the speed of sound. It knocks
   plants down (flattened, then dying), throws and kills animals, and
   harms drones and haulers by overpressure. Surface and shallow shots
   get the full air blast.
8. **The base surge**: a low dust cloud rolling out along the ground for a
   minute or so, drawn (and, for nuclear, carrying fallout if that is
   wanted; a decision below).
9. **Nuclear only, when the fireball shows** (surface and shallow shots, or
   a strong vent): the thermal pulse lights plants and burns animals out
   to its threshold, using the existing fire; prompt radiation kills
   animals in sight; EMP harms drones (decisions below).
10. **Later:** the chimney and subsidence for deep shots (built), the lip
    and walls slumping (settling, built).

Off screen, all of it is worked out at once as numbers, as now.

## Benjamin's decisions (27 Nov)

- **The shock runs at the rock's real speed** (1 to 6 km/s). At 10 kt it
  reaches the surface in about a tenth of a second; the jump, the dome and
  the burst (about 2 s) carry the show.
- **Radiation acts only when it happens** (prompt radiation, the flash, EMP).
  No lasting hot ground for now.
- **Harm to the swarm's machines:** the ground thrown under them wrecks them
  (from the real spall zone), and the pressure wave blasts them away with
  minor damage that grows with its strength. Heat, EMP and radiation do not
  harm the swarm.
- **Sappers stay buried** at their cratering depth. The routine handles any
  depth, the surface and air bursts, for test blasts and later units or
  upgrades.

## Built: part 1, the timeline and the shock at rock speed (27 Nov)

- **Each blast gets a timeline when it goes off** (`BlastTimeline` in
  `src/units/sapper.rs`), kept on its job: nuclear or conventional (any
  fissile in the charge), the yield, the depth, the shock's speed, when the
  shock reaches the surface (t_s), when the dome bursts (t_vent) and what
  hole it leaves (crater, subsidence, mound, contained; or none for a burst
  too high in the air for its fireball to touch the ground).
- **The shock's speed** comes from the ground between the charge and the
  surface: twelve places up the way (beside the sapper's shaft) are looked
  at, and the time is added up stretch by stretch, so a soft layer slows it
  as much as it is thick. Loose ground, sand and rubble 1 km/s; rock 0.9
  km/s per point of hardness (granite, hardness 6: 5.5 km/s), at most 6 km/s;
  water and magma 1.5 km/s. On the test body (barren world, its crust
  hardness 3 to 4): 2.8 km/s at 10 and 100 kt, 3.1 to 3.4 km/s deeper down at
  1000 to 10,000 kt, so the shock reaches the ground over a 10 kt charge 79 m
  down in 0.03 s (inside the first tick) and over a 10,000 kt one in 0.24 s.
- **t_vent** is 0.85 W^(1/3) s for a nuclear charge and 0.45 W^(1/3) for a
  conventional one, sooner in proportion when shallower than 30 m/kt^(1/3)
  (39 for conventional), and the same as t_s on the surface or in the air.
  10 kt: 1.83 s nuclear, about 0.9 s conventional. 1000 kt: 8.5 s. 10,000
  kt: 18.3 s.
- **The kind of hole** is by depth / W^0.294: a crater up to 65, subsidence
  to 90, a mound to 120, contained past that (a conventional charge's 1.3
  times as deep). Sappers' charges are all craters (40 at 10 kt, 53 at
  10,000 kt). For now every kind still throws its crater as before; the
  kind is there for the dome, the ejecta and the fallback steps.
- **The crater's ground is thrown when the dome bursts** (t_vent), by the
  clock, no longer when the work reaches the ground. The crater is planned
  in the blast's first ticks as before, and was ready in time at every
  yield tried: the dome bursting / the crater's ground going, 10 kt 1.83 /
  1.9 s, 100 kt 3.95 / 4.0 s, 1000 kt 8.50 / 8.5 s, 3000 kt 12.26 / 12.3 s,
  10,000 kt 18.31 / 18.4 s (within a tick every time).
- **The front and the work are kept apart.** The job's `front` is now the
  real shock, the rock speed times the time since it went off. The work
  (rays, column, cracks, in half-metre shells) follows it as fast as the
  budget allows, as before; it simply no longer has a speed limit of its
  own (it was 200 m/s). The work is not what is seen on time: the crater
  is. So the per-tick cost is unchanged (BLAST_BUDGET); the rock is dealt
  with as soon as the budget allows, while the job itself runs on until the
  vent and what follows it.
- **The game draws the buried blast from the sim's clock.** The boom now
  carries when it went off (game time), the shock's speed, t_s, t_vent, the
  depth and whether it was nuclear. The flash, the shake and the sparks come
  when the game clock reaches that moment; the shock ring runs out at the
  shock's speed by the game clock, so it reaches the ground over the charge
  when the sim says it does at any game speed (at full speed it crosses a
  10 kt blast's rock in a frame or two; slowed down it can be watched); the
  white light and the heat glow fade by game time too. The wall-clock ring
  is gone.
- **Test blasts at any depth:** `Sim::test_blast_at(over, kt, depth_m,
  conventional)` (0 on the surface, less than 0 in the air), and the game's
  `--test-blast <kt>[,<depth_m>[,conv]]` (`--test-blast 100` still works:
  a nuclear charge at a sapper's depth). A charge less than 3 m down is
  drawn as a blast in the open.
- Saves: nothing new is left pending; `finish_blasts` throws the crater at
  once as before.
- Timings (headless, `blast_timings`, worst whole step / most blast work in
  a tick, before and after):

  | Yield | Worst step | Most blast work in a tick | Blast work in all | Ticks the job ran |
  |---|---|---|---|---|
  | 10 kt | 57.7 / 56.1 ms | 16.3 / 15.7 ms | 88 / 85 ms | 15 / 25 |
  | 100 kt | 79.0 / 79.4 ms | 18.4 / 17.7 ms | 164 / 164 ms | 28 / 46 |
  | 1000 kt | 103.0 / 102.0 ms | 16.7 / 17.0 ms | 375 / 389 ms | 57 / 91 |
  | 3000 kt | 105.0 / 115.0 ms | 14.1 / 16.6 ms | 617 / 630 ms | 88 / 130 |
  | 10,000 kt | 108.3 / 110.3 ms | 14.0 / 12.4 ms | 961 / 969 ms | 143 / 197 |

  The blast's own work stays under 18 ms a tick. The worst steps are, before
  and after alike, the swarm's once-a-second work (54 to 89 ms; at 30 s it
  scans the ground the blast opened for the specialists), not the blast:
  that is over the 45 ms aim and was already, so it is the next thing to
  look at for big blasts. The job now lasts longer (it waits for the vent
  before its plume and held pieces), but its work in all is the same.
- Tests: `a_blast_times_its_events_by_its_kind_yield_and_depth` (the
  numbers), `a_blast_throws_its_crater_when_its_dome_bursts_and_keeps_the_books`
  (nuclear and conventional, books checked every few ticks),
  `surface_and_air_test_blasts_run_their_course_and_keep_the_books`.

## Built: part 2, the jump, the dome and the vent (27 Nov)

- **The ground jumps when the shock reaches it.** Right over the charge at
  4 × 10⁴ / λ^1.9 m/s (λ the depth in m/kt^(1/3); at most 100 m/s), which
  gives Sedan's 32 m/s at its depth and 42 m/s for a sapper's charge at any
  yield. It falls off away from over the charge as (depth / distance from
  the charge)^2.9: a third as fast one depth out. The zone reaches out to
  where it jumps at 3 m/s, at most 3 depths: 182 m either side at 10 kt,
  840 m at 1000 kt, 1.8 km at 10,000 kt. The ground goes away from the
  charge, with the upward part counted three times over, so straight up
  over the charge and at about 50 degrees at the zone's edge. The timeline
  carries both numbers (`spall_v0`, `spall_m`). A burst on or over the
  ground has no jump: its air blast is part 3.
- **The crater's ground rises as a dome** from when the shock reaches it.
  Each column of it goes straight up as one, at three quarters of the
  jump's speed there: 31 m/s over a sapper's charge, slower towards the
  rim, so it rises highest in the middle. That three quarters is Sedan's
  average: its dome rose 90 m in 3.7 s against its 32 m/s jump. A piece
  that meets anything while it rises stops there and waits for the burst.
  (At first the pieces were set down where they hit. Beside a big crater on
  the test body, the dome's edge ran into the steep slope and piled up
  there in the air, the dome under it piled up against that, and the crater
  came out roofed over, a third as deep.)
- **The dome bursts at t_vent**: everything still rising with it takes its
  speed in today's fan at once (a scheduled kick, `Bodies::vent`, on the
  clumps in the air and on held pieces flying as rock bodies). Ground not
  yet lifted by then goes straight out in the fan as before. I chose the
  kick on the flying pieces over one rising slab broken at the vent: the
  pieces are already the cheap way the ground flies (clumps), a kick costs
  one pass over them once, and each piece keeps its own fan speed.
  - Nuclear: a glow of hot gas and glowing rock at the vent. Conventional:
    a puff of brown fumes and no glow. The simulation sends a `Vent` (where,
    the crater's width, when, which kind), and the game draws it by its own
    clock: a light fading over 2 to 6 s and a column of blocks rising and
    cooling (white to red for nuclear, brown fumes for conventional).
- **The ground round the crater is kicked up and falls back.** In the zone
  outside the crater, the top 2 m of the ground (1 m where it jumps slower
  than 10 m/s), with any rock, soil and rubble in it and the plants on it,
  goes as the shock reaches each place, at the jump's speed. Liquids stay.
  It falls back near where it was within about 2.5 s: it is pulled back at
  least as hard as on Earth (9.8 m/s², more for the fastest pieces). On
  these small bodies the real pull is a few thousandths of that, so a jump
  of a few metres a second would hang there for minutes or leave the body.
  It lands as loose rubble that cutters take quickly. On the test body the
  ground 100 to 120 m out ends up within 4 m of its old height, loose. At
  most 384 pieces of it are in the air at 10 kt and 1,970 at 3000 kt
  (single cells up to 3,000 cells, then clumps of 2 × 2). At most 20,000
  cells a second are lifted.
- **Plants on the thrown ground die.**
  - Plants are followed up each column as tall as they stand, up to 120 m.
    (The crater took only 24 m before, which left tall crowns hanging.)
    They are thrown with the ground under them and wilt where they land.
  - Once everything the blast throws is lifted, the plants beside what went
    are followed through, cell to cell. Any group not joined to the ground
    falls the same way, and so does any plant that was left hanging on one
    of those.
  - Only cells side by side count as joined, as the world's own pieces cut
    free are joined. Loose rubble does not hold anything up.
  - Out of sight, nothing is thrown. The plants in the zone are killed
    where they stand instead: lifted and set down again, so they wilt.
- **Animals and machines on the thrown ground.** Where it jumps at 5 m/s or
  more (1.8 depths either side: 140 m at 10 kt), cutters on it, or in it
  down to the charge, are wrecked when the shock gets there. Animals there
  are flung and killed. This replaces the loosened column's own rule, which
  reached about 50 m either side at 10 kt.
  - Cutters within the blast radius and 4 m of the charge are still wrecked
    the moment it goes off. Haulers are hurt by distance as before (part 3
    replaces that).
  - An animal killed by a blast is counted as lost, like a fall. It leaves
    a burst of gore and alarms its herd, as the abductor's laser does.
  - Its biomass is not credited (the laser's is): the blast scatters it
    through the rubble, and no cutter scoops it up whole.
- **Saves:** `finish_blasts` bursts every dome at once and sets the
  kicked-up ground down where it is, so nothing new is saved. No save
  version change.
- **Not seen on time, yet:** the crater's plan and the zone's are worked
  out over the first ticks, within the budget, so the crater's ground
  starts rising a little after the shock arrives. At 10 kt it starts at
  0.4 s (shock at 0.03 s), at 100 kt 0.6 s, at 1000 kt 1.2 s, at 3000 kt
  1.7 s and at 10,000 kt 2.9 s. In every case that is well before the vent
  (1.8 s to 18 s). It starts from where it lies, and does not jump up to
  catch up. The plan's looks at fresh ground cost about two and a half
  times what they are counted as, so the crater keeps to half the budget.
  The zone's scan looks the ground up once a place (the crater uses
  three), and is counted at its real cost.
- Timings (headless, `blast_timings`; part 1's after / part 2's):

  | Yield | Worst step | Most blast work in a tick | Blast work in all | Ticks the job ran |
  |---|---|---|---|---|
  | 10 kt | 56.1 / 57.9 ms | 15.7 / 15.7 ms | 85 / 86 ms | 25 / 18 |
  | 100 kt | 79.4 / 76.7 ms | 17.7 / 14.6 ms | 164 / 165 ms | 46 / 39 |
  | 1000 kt | 102.0 / 97.9 ms | 17.0 / 14.9 ms | 389 / 376 ms | 91 / 84 |
  | 3000 kt | 115.0 / 97.5 ms | 16.6 / 14.7 ms | 630 / 630 ms | 130 / 122 |
  | 10,000 kt | 110.3 / 97.4 ms | 12.4 / 12.4 ms | 969 / 978 ms | 197 / 183 |

  - The worst steps are the swarm's once-a-second work, as before (56 to
    73 ms), not the blast.
  - The blast's tick with its flying and settling is at most 21 ms (it was
    23).
  - The crater still digs as deep as before: 17, 51, 97, 101 and 161 m at
    the middle.
  - At 1000 kt and up all of the crater's mass leaves this small body, as
    before.
  - At 10 and 100 kt slightly more than the crater's mass leaves (106% and
    102%). Kicked-up ground from the slope into a deep chasm 80 to 100 m
    from the test site falls down the chasm, and some of it is lost.
- Tests:
  - `the_ground_over_a_blast_jumps_by_its_depth` (lib): the numbers.
  - `a_blast_lifts_its_crater_as_a_dome_and_throws_it_when_the_dome_bursts`
    replaces part 1's crater-timing test. Nuclear and conventional: the
    dome rises no faster than three quarters of the jump; the ground round
    it is kicked up; nothing is still rising after the burst, and nothing
    was set down by it; the vent comes once, of the right kind, at the
    ground over the charge.
  - `the_ground_round_a_crater_is_kicked_up_and_falls_back`: where it lands,
    and a save taken mid-dome.
  - `plants_and_animals_on_thrown_ground_die`, on the dev planet with its
    life on: the animal over the blast is killed with gore, the plants
    thrown round the crater are dying, and no plant cell is left hanging
    within the zone.
  - The books are checked throughout in all of them.

## Built: part 3, the air blast and the base surge (27 Nov)

This runs only on bodies with air (oxygen or an atmosphere in their spec).
The bodies give no air pressure of their own yet, so every body with air is
taken to have the Earth's (101 kPa) and a speed of sound of 340 m/s. Bodies
without air are unchanged: the barren test body, and the sapper and
tunneler test bodies.

- **How strong.**
  - The Kinney-Graham fit for TNT in the open. A nuclear charge counts as
    half its yield in TNT; a conventional charge's yield is TNT already.
  - Times 1.8, for the ground reflecting it.
  - A buried charge lets out only part of it, by its scaled depth (depth /
    W^0.294; a conventional charge's counts as 1.3 times shallower): 0.3 at
    a quarter of the cratering optimum, 0.075 at the optimum (45), 0.015
    when contained (120), and less deeper still. A sapper's charge lets out
    about 9%.
- **Or the ground's push, whichever is stronger.**
  - The ground jumping pushes the air at 0.41 kPa per m/s. That is about 17
    kPa over a sapper's charge, falling off as 1 over the distance once past
    its depth.
  - For a sapper's charge the fit is stronger everywhere.
  - For a contained shot the push is all there is (10 kt at 400 m lets out
    0.3%).
  - The fit is measured from the charge itself, not from the ground over it.
    Measured from the ground, its huge close-in pressures had a contained
    shot flatten trees 50 m round.
- **Where and when.** A buried charge's front leaves the ground over it
  when its shock gets there. The jump's push and the gas that follows at the
  vent are not drawn as separate fronts. A charge on or over the ground
  sends its front from the charge at once. The front runs out at 340 m/s.
- **How far it reaches.**

  | Charge | Trees 90% down (35 kPa) | Plants stripped (12 kPa) | Machines thrown (2 kPa) |
  |---|---|---|---|
  | 10 kt on the surface | about 950 m | | |
  | Sapper's 10 kt | about 300 m (the ground's jump reaches 182 m) | 500 m | 1.2 km |
  | Sapper's 100 kt | | 1 km | 2.4 km |
  | Sapper's 1000 kt | | 2.1 km | 4.9 km |

- **Plants in sight.**
  - The ground is walked out from the blast, a step either side at a time,
    within the budget. The walk starts where the ground's jump stops taking
    plants.
  - A plant is its cells joined side by side, of one species, without its
    roots. Each plant met is judged once, by the pressure where it stands.
  - Ground cover and plants under 3 m are stripped from 12 kPa.
  - Taller plants are blown down by chance: 30% at 24 kPa, 90% at 37.5 kPa,
    on the straight line through those two points. Tall trees count the
    pressure as up to 1.3 times higher, short ones down to 1/1.3 (height
    over 15 m, to the power 0.3).
  - What goes is thrown downwind (away from the blast) when the front gets
    there, in clumps of up to 3 x 3 cells. Wood flies at 12% of the wind
    behind the front, leaves at 35%, at most 40 m/s, a little upward.
  - It falls back within about 1.5 s and wilts where it lands.
  - Plants left hanging on it fall after, through the part 2 check (which
    now waits for this walk too).
- **Plants out of sight.** Trees blown down are killed where they stand
  (lifted and set down again, so they wilt). Low plants are left.
- **Animals.** Every animal the front reaches at 10 kPa or more is dealt
  with when the front gets there.
  - From 200 kPa: killed.
  - From 35 kPa: killed by chance (things flying in the wind), 10% rising to
    100% at 200 kPa.
  - Otherwise: thrown along the ground away from the blast, as far as the
    wind carries it in 0.3 s (at most 40 m). It gets up and runs.
  - A kill is booked as in part 2: gore, its herd alarmed, counted as lost.
- **The swarm's machines.** Benjamin's decision: the pressure wave blasts
  them away, with minor damage that grows with its strength.
  - Every machine within the 2 kPa reach is dealt with when the front gets
    to it, soonest first.
  - It is thrown away from the blast at the speed of the wind behind the
    front (at most 150 m/s), for 0.3 s: 1 m at 2 kPa, 22 m at 35 kPa, at
    most 45 m.
  - Cutters: wrecked by chance from 100 kPa only, 0% at 100 kPa rising to
    25% at 400 kPa.
  - Haulers lose their whole working life at 400 kPa, in proportion below.
    On bodies with air this replaces their old damage by distance. Without
    air there is no pressure wave, so the old rule stays.
  - Walkers lose 2 health per kPa (a forester has 2,000, an abductor
    100,000).
  - **How it is drawn.** The simulation does not move them. It tells the
    screen which machine was thrown, when, how far and how fast (`Push`).
    The game draws it leaving at that speed, slowing to a stop, then flying
    back to where the simulation has it, by the game clock. Cutters fly back
    at their own speed, haulers and walkers at 15 m/s.
  - Cutters (there may be hundreds of thousands) are looked at a budget's
    worth at a time. Haulers, walkers and animals (a few hundred at most)
    are looked at all at once.
- **The base surge.** When the dome bursts (at once for a surface burst) and
  the blast leaves a crater, the simulation tells the game (`Surge`). The
  game draws a low cloud of dust rolling out along the ground either side,
  by the game clock.
  - It reaches 3 crater radii: about 120 m at 10 kt, 570 m at 1000 kt.
  - It starts at 30 m/s and slows. It is 95% of the way out after three time
    constants (12 s at 10 kt).
  - It is thickest at its rolling front, about an eighth as high as it is
    wide. Nuclear is greyer, conventional browner.
  - It thins away over a minute (longer for big blasts).
  - It leaves no lasting radiation.
- **Far sectors (out of sight, worked out as math).** The animals there are
  real, so the ground's jump and the air blast kill them by the same rules,
  all at once. Those it would only throw are scared off. The plants there
  are part of the sector's math, not cells, so they are left alone.
- **Saves.** `finish_blasts` deals with every machine and animal at once
  and walks all the ground for plants. Thrown plants wait in the same queue
  as the ground the jump throws, and go when the front would have got there,
  as in part 2. The screen's throws and surges are for drawing only. Nothing
  new is saved, so there is no save-version change.
- **Not built.**
  - No front is drawn in the air itself; only what it throws shows it.
  - There is no second pulse at the vent.
  - A burst high over the ground uses the same 1.8 reflection as a surface
    burst (the Mach stem).
- **Timings.**
  - `blast_timings` runs on the barren test body, which has no air, so none
    of part 3 runs there. Its numbers match part 2's within the noise. The
    most blast work in a tick was 15.3 / 14.3 / 17.0 / 14.9 / 13.0 ms at 10
    to 10,000 kt. The worst steps (58 to 97 ms) are still the swarm's
    once-a-second work.
  - A new ignored test, `blast_timings_air`, uses the dev planet with life
    on and 20,000 cutters at work. "Flock" rows: the blast goes off 300 m
    (times the cube root of kt / 10) along from the busiest flock. "Forest"
    rows: in the planet's thickest forest.
  - "Before" is the same run with the air blast switched off; "after" has it
    on. Each cell gives the worst whole step / most blast work in a tick /
    blast work in all / ticks the job ran.

  | Case | Before | After |
  |---|---|---|
  | 10 kt, flock | 38.2 / 15.2 / 57 ms / 18 | 39.7 / 14.8 / 59 ms / 36 |
  | 100 kt, flock | 37.6 / 23.5 / 128 ms / 39 | 40.5 / 22.2 / 126 ms / 71 |
  | 1000 kt, flock | 49.1 / 13.7 / 329 ms / 84 | 40.5 / 18.8 / 340 ms / 119 |
  | 10 kt, forest | 38.3 / 10.1 / 81 ms / 18 | 36.9 / 11.7 / 84 ms / 18 |
  | 1000 kt, forest | 37.6 / 21.1 / 400 ms / 84 | 43.8 / 25.6 / 405 ms / 144 |

  - The air blast adds a few ms of work in all.
  - The job runs longer because it waits for the front to reach the
    farthest machine: 1.2 km at 10 kt takes 3.6 s, 4.9 km at 1000 kt takes
    14 s.
  - Every "after" step is under the 45 ms aim. The one step over it in the
    table is a "before" row (49 ms), which is within run-to-run noise.
  - At 10 kt by the flock, the air blast threw 2,477 machines, 19 animals
    (5 of them killed) and 511 plant cells. In the forest it blew down 25
    trees and stripped 24 plants (906 cells).
  - The dev planet's thickest forest has only about 2,300 plant cells within
    300 m, so a much denser forest has not been timed.
- **Tests.**
  - `an_air_blast_is_as_strong_as_its_burial_lets_it_be` (lib): the numbers.
  - `an_air_blast_blows_down_plants_and_throws_animals`, on the dev planet
    with life on.
    - In sight: no plant between the jump's edge and the 30 kPa reach goes
      before the front gets there. Afterwards fewer than half are standing
      and the rest are dying, none is left hanging, and the animal 250 m out
      is thrown or killed. The surge starts at the vent.
    - Out of sight: the trees are killed where they stand and nothing is
      thrown.
  - `an_air_blast_blows_the_swarms_machines_away`, on the dev planet with
    3,000 cutters. The flock 300 m out is thrown away from the blast: 1,868
    cutters and its hauler. Each is told to the screen only after the front
    reaches it, and is thrown as far as the wind carries it in 0.3 s.
  - The books are checked throughout in all of them.

## Built: part 4, nuclear against conventional, and blasts in the open (27 Nov)

- **A buried conventional charge** (no fissile in it) now differs from a
  nuclear one in more than its dome bursting sooner (part 1):
  - Its heart doesn't melt. The rock there is crushed and left where it was,
    as rubble baked by the heat, out to where a nuclear charge's would melt
    (about 9 m at 10 kt). Nothing further out is baked (a nuclear charge
    bakes out to 0.6 blast radii). No melt, no glass, no glow.
  - On the screen its flash is small and short (0.15 s, under half as
    bright), and it leaves no heat glow.
  - Its gas comes out as a sharp second air pulse when its dome bursts. On a
    body with air, what its air blast does (throwing machines and animals,
    blowing down plants) now comes with that pulse, from the ground over the
    charge at the vent: at 10 kt about 0.9 s after it goes off instead of
    0.03 s. Where the ground's push is the stronger (a deep, contained
    shot), it still comes with the first front. The screen draws the pulse
    as a thin ring of dust and mist racing out at the speed of sound for 0.8
    s, with the brown fumes of part 2.
  - A nuclear charge is as before, with its vent glow.
- **A blast in the open** (a test blast 3 m down or less, on the ground, or
  over it: `--test-blast 10,0` or `10,-150,conv`):
  - **The fireball**, on a body with air only. A nuclear one reaches 0.088
    W^0.4 km on the ground (221 m at 10 kt, 1.4 km at 1000 kt), a little less
    clear of the ground (0.067 W^0.4 km), full size at (W/20)^0.4 s (0.76 s
    at 10 kt), growing as the time to the 0.4. A conventional one is only its
    hot gas: about five charge radii (56 m at 10 kt), full size in 0.06 s.
  - **Everything in a nuclear fireball is vaporised as it grows to it**: the
    plants in it (each cell lifted out and booked as vented, as the fires out
    of sight book what they burn) and the animals (counted as lost). The
    ground under it is glazed (its top cell's look turned to glass). The
    swarm's machines are not harmed (Benjamin's decision).
  - **The flash of heat** (nuclear only) peaks at 0.042 W^0.44 s (0.12 s at
    10 kt). It gives 0.86 W / D² cal/cm² at D km for a burst on the ground, up
    to 2.79 W / D² for one clear of the ground (a third of the yield as heat
    over a whole sphere), in between by how high it is for its fireball.
    Plants in sight of the fireball catch fire from 3 W^0.11 cal/cm² (3.9 at
    10 kt: about 1.5 km out on the ground), lit at the highest cell in sight
    at each place along the ground, and the fire spreads: on screen the
    sandbox's own fire (at most 3,000 cells lit by one flash; past that,
    scorched dead where they stand), out of sight a fire burned as math, as
    the forester's are. Animals in sight are burned to death from 8 cal/cm²
    (about 1 km at 10 kt).
  - **Prompt radiation** (nuclear only, at once) kills animals in sight
    within 1.2 (W/10)^0.14 km. Buried, none.
  - **In sight** is worked out by walking out along the ground from under the
    blast either side, keeping the highest the ground has risen so far as
    seen from the fireball's middle (half way up it, on the ground): a hill,
    or the curve of this small body, hides what is behind it. Every 0.7 m in
    sight, every 2.1 m out of it (the fire spreads between), within the
    budget; each place once the fireball has grown to it, or past it once
    the flash has come. The air blast's walk for plants (part 3) now starts
    past the fireball.
  - **With no air**: no fireball and no air blast. A nuclear charge's flash,
    most of it X-rays (6 W / D² cal/cm²), scorches the plants in sight dead
    where they stand (no oxygen to burn in), and kills animals in sight as
    above.
  - **The ground under it**: a shallow bowl by the table, 13 W^0.294 m each
    way and 6 W^0.294 m deep (26 m and 12 m at 10 kt), thrown by the existing
    crater code when the blast goes off, with the ground shock (the rays and
    cracks) round the charge as before. Over the ground it is smaller the
    higher the burst, and there is none once the fireball doesn't reach the
    ground: such a burst breaks no rock at all (no rays, column, cracks or
    rubble). Its base surge rolls out to three of its bowl's radii.
  - **On the screen** (by the game clock): a nuclear fireball is a dome of
    blocks growing to its full size, white-hot and lighting everything round,
    then rising, swelling and cooling through yellow, orange and red to
    smoke (7.5 s at 10 kt, at most 14 s), with the screen whited out and
    shaking. A conventional one is small, orange and gone in about a second,
    with a small flash. The embers and dust ring of a blast in the open are
    drawn as before, now by the game clock.
  - **Saves**: `finish_blasts` walks all of the fireball and flash at once;
    the fires out of sight are saved as the forester's are. No save-version
    change.
- **Testing from the game**: `test-big-blast.bat` now also asks how deep
  (Enter for a sapper's depth, 0 on the ground, -150 for 150 m up) and
  whether it is conventional (type conv), so all four kinds can be tried.
- **What it did on the dev planet** (`blast_timings_air`, in its thickest
  forest, or by the busiest flock):
  - 10 kt nuclear on the ground: 945 plant cells vaporised, 494 ground cells
    glazed, 364 plant cells set alight, 28 animals vaporised and 48 burned to
    death.
  - 10 kt conventional on the ground: no heat pulse; its air blast alone
    does its work.
  - 100 kt nuclear 150 m up: a fireball 508 m round (486 m along the ground),
    1,244 plant cells vaporised, 506 set alight, 59 animals vaporised and 56
    burned.
  - 1000 kt nuclear on the ground: a fireball 1.4 km round, 1,193 plant cells
    vaporised, 2,354 ground cells glazed, 88 set alight (the body's curve
    hides most of the 11.6 km its heat could light), 125 animals vaporised
    and 9 burned.
- **Timings** (headless; "before" is the same run without part 4). Each cell
  gives the worst whole step / most blast work in a tick / blast work in all
  / ticks the job ran.

  | Case (dev planet) | Before | After |
  |---|---|---|
  | 10 kt buried, flock | 40.5 / 14.8 / 60 ms / 36 | 38.7 / 14.6 / 60 ms / 36 |
  | 100 kt buried, flock | 38.1 / 22.6 / 127 ms / 71 | 37.9 / 22.8 / 129 ms / 71 |
  | 1000 kt buried, flock | 43.2 / 18.9 / 334 ms / 119 | 41.3 / 18.7 / 341 ms / 119 |
  | 10 kt buried, forest | 38.2 / 12.4 / 83 ms / 18 | 37.7 / 11.7 / 82 ms / 18 |
  | 1000 kt buried, forest | 43.7 / 26.5 / 405 ms / 144 | 43.7 / 26.0 / 405 ms / 144 |
  | 10 kt on the ground, forest | 46.1 / 9.8 / 77 ms / 192 | 40.3 / 9.1 / 83 ms / 193 |
  | 10 kt conventional on the ground, forest | 45.7 / 9.3 / 77 ms / 192 | 44.9 / 9.0 / 75 ms / 192 |
  | 100 kt 150 m up, forest | 49.5 / 12.6 / 103 ms / 194 | 52.6 / 12.6 / 121 ms / 196 |
  | 1000 kt on the ground, flock | 51.4 / 28.8 / 299 ms / 256 | 50.4 / 18.1 / 272 ms / 256 |

  - The fireball and flash cost up to about 18 ms of blast work in all,
    spread over the ticks. The most blast work in a tick doesn't go up.
  - The blasts in the open already went over the 45 ms aim before part 4
    (46 to 51 ms). Their worst steps aren't the blast's work (at most 18 ms
    in a tick), so part 4 neither caused nor fixed them.
  - `blast_timings` (the barren test body, buried nuclear charges) is
    unchanged within the noise. The most blast work in a tick was 16.2,
    14.3, 15.4, 18.7 and 12.7 ms at 10 to 10,000 kt (before: 14.9, 15.1,
    14.5, 14.4, 12.6). The worst steps, 57 to 97 ms, are the swarm's
    once-a-second work, as before.
- **Not built.**
  - A buried nuclear charge's vaporised mass (about 70 t a kt) is not let
    out at its vent: the brief kept buried nuclear charges as they were.
  - The flash and radiation reach only animals and plants. The swarm is not
    harmed, as decided, and there is no EMP.
  - Plants shade each other from the flash only through the ground: a tree
    doesn't hide the one behind it.
  - Animals killed by the flash just go (no gore); their herd is alarmed.
- **Tests.**
  - `a_blast_in_the_open_has_a_fireball_a_flash_and_a_shallow_bowl` (lib):
    the numbers above, a burst in the air, one too high for a hole,
    conventional, no air, buried, and the conventional charge's second
    pulse.
  - `surface_and_air_test_blasts_run_their_course_and_keep_the_books` now
    also checks, on the airless test body, that there is no fireball and no
    air blast, that a nuclear charge still flashes, the bowl's depth, and
    that a burst 300 m up breaks no rock.
  - `a_nuclear_blast_in_the_open_burns_what_is_in_sight_and_a_conventional_one_does_not`,
    on the dev planet with its life on. The nuclear one vaporises the plants
    and the animal in its fireball (next to nothing is left open to the sky
    in it), glazes the ground and sets plants alight. The conventional one
    sends out no heat pulse. The books are checked throughout.

## Built: review fixes (27 Nov)

A review of parts 1 to 4 found these. All are fixed except where it says
otherwise.

- **Only a crater-depth charge throws a crater.** A charge too deep for its
  kind and yield (subsidence, mound or contained) used to lift a dome and
  throw a full crater anyway: 10 kt at 250 m threw as much as a sapper's
  81 m shot, with no vent or surge to show why. Now only the crater kind
  plans and throws one. Deeper, the ground over it is kicked up like the
  ground round a crater and falls back where it was, and nothing vents.
  Such a charge also no longer loosens a column up to the surface (a 200 m
  column sank the ground 8 m): its cavity's roof falls in as a rubble
  chimney, as a tunneler's does. 10 kt at 151, 200 and 250 m now leaves the
  ground over it 0.0, 0.1 and 0.3 m lower. (The chimney doesn't reach the
  surface at these depths, so a subsidence shot doesn't sink yet.) Sapper
  charges are all the crater kind and are unchanged.
- **A test blast keeps the depth asked for.** The depth was measured again
  from the ground beside the charge, so on rough ground a blast asked for
  on the surface could go off buried (on the barren body 13 of 60, one
  812 m down), with no fireball. `test_blast_at` now passes its depth
  through.
- **The ground jumps on time.** The kicked-up ground is now planned before
  the crater, with first call on the budget. A plan that is still late goes
  from as far along its way as it would have got, but at most 0.2 s along
  (`SPALL_CATCH_S`). Past that, the whole of it goes that much later,
  nearest the charge first. The dome works the same way: it starts from
  when the shock reached it, at most 0.2 s along.
  - First kicked-up ground in the air, barren body: 0.3 / 0.4 / 0.6 / 0.8 /
    1.2 s at 10 / 100 / 1000 / 3000 / 10,000 kt. The review measured 0.4 s
    at 10 kt and 1.0 s at 1000 kt before. On the dev planet, 0.1 s at 10 kt.
  - The crater, planned second, now starts rising at 0.5 / 0.8 / 1.7 / 2.3
    / 4.0 s (it was 0.4 / 0.6 / 1.2 / 1.7 / 2.9 s). Its dome bursts at 1.8
    to 18.3 s.
- **Machines and animals are caught as the ground under them goes**, each
  place as the shock reaches it, not all at once when the shock first
  touches the surface. In sight, the air blast's hits also wait for the
  ground to go, then catch up at once. Before, cutters were wrecked and
  gore flew a few ticks before anything moved.
- **The whole jumping zone wrecks machines.** The wreck line was 5 m/s
  (about 140 m either side at 10 kt), while ground was kicked up out to
  3 m/s (182 m). It is now the zone's own edge, for animals too. Walkers on
  the jumping ground lose a forester's whole health (an abductor's huge
  pool shrugs it off).
- **A save throws what is still due.** The queue of ground still to be
  thrown (kicked-up ground, plants the air blast blows down, the crater's
  deeper layers) was never saved. So a game that went on after a save
  threw it and a loaded one didn't (3,248 against 3,753 plant cells left
  within 600 m). `finish_blasts` now throws all of it at once. In the new
  test both games keep 1,042.
- **Haulers have their own numbers.** Air-blast hits were stored by a
  hauler's place in the list, which shifts as haulers retire, so the wrong
  hauler could be hurt and drawn flung. Each hauler now has an id for its
  life (not saved; numbered afresh on loading), used by the hits, the
  pushes and the screen.
- **Drone numbers stay put during a blast.** The once-a-minute compaction
  of the drone list now waits while any blast is going off, so an air
  blast's queued hits stay on the right cutters.
- **The air blast's walk for plants keeps at most 1 s ahead of its front**,
  so it no longer takes the whole budget for many ticks. The shock's field
  (which the rock's breaking waits on) is now ready 1.1 s after a 1000 kt
  blast on the ground, 2.3 s after 10,000 kt buried and 1.7 s after 10,000
  kt on the ground (the review measured 3.8, 4.7 and 7.8 s).
- **Dome pieces are never dropped as escaping** before their dome bursts.
  Their speed isn't their own until then.
- **On the screen** (not yet seen in the game):
  - The air blast is drawn as a low wall of dust running out along the
    ground either side at the speed of sound, from when and where its front
    leaves. It thins out to where the front stops moving machines (1.2 km
    at 10 kt buried, about 9 km at 10 kt on the ground). It replaces the
    open blast's old dust ring and part 4's conventional pulse ring.
  - On a body with no air, a buried charge throws up dust off its jumping
    ground as the shock gets to each place, settling over 1.5 s.
  - The flash and the screen shake now fade by the game clock, so they keep
    step with the fireball at any game speed. A nuclear fireball's white-out
    lasts while it grows, up to 1.5 s.
- **Not changed.** The vent's sparks and fume particles still run on the
  real clock (they are short). Rock bodies and plume rock in the air at a
  save are still booked as lost on loading, as before.
- **Timings** (headless; "before" is the same runs just before these
  fixes). Worst whole step / most blast work in a tick / blast work in all /
  ticks the job ran:

  | Case | Before | After |
  |---|---|---|
  | barren, 10 kt | 57.2 / 16.7 / 89 ms / 18 | 56.0 / 15.5 / 78 ms / 18 |
  | barren, 100 kt | 78.1 / 14.4 / 160 ms / 39 | 76.1 / 14.9 / 156 ms / 39 |
  | barren, 1000 kt | 100.9 / 15.3 / 384 ms / 84 | 101.6 / 13.0 / 369 ms / 84 |
  | barren, 3000 kt | 98.2 / 14.7 / 656 ms / 122 | 96.4 / 10.0 / 617 ms / 122 |
  | barren, 10,000 kt | 98.1 / 14.2 / 1032 ms / 183 | 94.9 / 11.3 / 978 ms / 183 |
  | dev, 10 kt buried, flock | 53.2 / 15.0 / 60 ms / 36 | 40.0 / 15.1 / 61 ms / 36 |
  | dev, 100 kt buried, flock | 39.7 / 23.3 / 131 ms / 71 | 42.0 / 21.1 / 130 ms / 71 |
  | dev, 1000 kt buried, flock | 41.8 / 20.8 / 355 ms / 119 | 38.6 / 14.4 / 344 ms / 104 |
  | dev, 10 kt buried, forest | 38.7 / 12.2 / 86 ms / 18 | 38.0 / 10.3 / 83 ms / 18 |
  | dev, 1000 kt buried, forest | 46.2 / 31.8 / 432 ms / 144 | 39.8 / 15.0 / 412 ms / 145 |
  | dev, 10 kt on the ground | 41.1 / 9.0 / 84 ms / 193 | 43.5 / 10.6 / 84 ms / 193 |
  | dev, 10 kt conventional on the ground | 45.3 / 9.3 / 79 ms / 192 | 47.6 / 9.7 / 82 ms / 192 |
  | dev, 100 kt 150 m up | 54.9 / 12.5 / 122 ms / 196 | 41.6 / 12.7 / 124 ms / 196 |
  | dev, 1000 kt on the ground | 50.5 / 18.4 / 278 ms / 256 | 48.7 / 18.1 / 339 ms / 256 |

  - The barren body's worst steps (56 to 102 ms) are the swarm's
    once-a-second work, as before. The blast's own work is at most 15.5 ms
    a tick.
  - On the dev planet every buried case is now under the 45 ms aim. Two
    blasts on the ground are still over it (47.6 and 48.7 ms), from outside
    the blast's work, as before.
  - The 1000 kt forest blast's dearest tick of blast work halved (31.8 to
    15.0 ms), because the plant walk now keeps close to its front.
  - Crater sizes are unchanged. The 100 kt crater's middle ended 36 m lower
    against 51 m before (35 m planned).
- **Tests.**
  - `the_ground_jumps_as_the_shock_reaches_it` (dev planet, 3,000 cutters,
    10 kt under the busiest flock): the ground jumps within 0.5 s (it took
    0.1 s), over 50 cutters are wrecked by it (1,125), and none before the
    ground under them moved.
  - `a_charge_too_deep_for_a_crater_throws_none`: 10 kt at 151, 200 and
    250 m. No crater, no vent, the ground within 4 m (subsidence: up to 12 m
    lower allowed).
  - `a_test_blast_on_the_ground_goes_off_on_the_ground`: 12 places on the
    barren body and on the dev planet, each exactly 0 m down, in the open,
    with a fireball only with air.
  - `a_save_throws_what_a_blast_still_has_to_throw`: in the dev planet's
    thickest forest, saved three ticks in. Nothing is left waiting, and
    after 6 s the game that went on and the one loaded have the same plants
    within 600 m.
  - `plants_and_animals_on_thrown_ground_die` now waits for the ground under
    the animal to go before expecting it dead (within 6 ticks).
  - All of them check the books.

## Seen on screen and fixed (27 Nov, evening)

Filmed in the game at 10, 1000 and 10,000 kt buried, 10 kt on the ground,
100 kt 150 m up, and 10 kt conventional. The order of events was right; four
things looked wrong and were fixed:

- **The dome rose as a plug.** Every cell of the crater rose at its column's
  speed, so the whole crater lifted out as one flat-bottomed block over an
  empty bowl. Now each cell rises at its column's speed times (1 - its depth
  share): the top fastest, the bottom of the hole not at all. The ground
  swells and loosens into a mound joined to the ground round it, and bursts.
- **Trees stood with their roots blown off.** The jumping ground took plants a
  column at a time, straight up from the ground, so a crown spreading over
  bare ground stayed hanging until the "left hanging" check sank it into a heap
  seconds later. Now each plant found is taken whole (its cells joined side by
  side, not its roots, within the zone and outside the crater), and on a body
  with air it is thrown downwind too, by the air blast's wind where it stands
  (wood at 0.12, the rest at 0.35 of it, at most 40 m/s). Trees round a
  buried blast are swept away as the ground jumps.
- **The fireball was a wall of tiles.** 1,600 square blocks, tens of pixels
  each close up. It is now a smooth ball of triangles coloured at their
  corners, hottest in the middle, its surface churning, its edge fading out,
  cut off at the ground.
- **10,000 kt dropped to 48 fps.** Its dome holds some 80,000 cells in sight
  for about 14 s (before, the crater flew off screen in a few seconds). Moving
  each on to the frame's moment, keeping those in sight, colouring and drawing
  them one after another took 15 ms a frame. They are now done in one pass on
  all cores, and the cells are shared with the game rather than copied each
  frame. 10 / 1000 / 10,000 kt now hold 60 fps (worst second 59.9 / 60.0 /
  59.4).

Still to look at: the fire blocks flung from a blast in the open are drawn at
a fixed few metres, so close up they are big squares.
