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
