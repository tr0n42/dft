# Ground units: agreed behaviour (25 November 2026)

Benjamin and Claude went class by class before implementing, so the units need
less iteration once built. This page refines the roster of 10 November and the
Swarm Production Design doc. Where they disagree, this page wins.

Rules shared by every class:

- **Off screen is pure math.** Out of sight, a unit is a position, a task and
  numbers. Each one keeps where it is and what it's doing, so it is visibly mid-task
  the moment the camera arrives. Plumes, steam and couriers are drawn only when
  visible and cheap at that zoom (not when they'd be a few pixels).
- **Mass is always kept.** Visual flair (ejecta, plumes) need not be physically
  accurate, but the totals must be exact.
- **Big machines are capped by performance first and materials second.** The
  director builds them as needed, not as kept counts. Every class's cost is
  measured against the cutters-and-haulers baseline (`bench-units.bat`).
- **Danger is real.** Blasts kill cutters and damage or kill haulers. The director
  never pulls flocks back.

## Sapper (follows the swarm, made to order)

- **Yield is the single lever.** Burial depth, the loosened volume, damage and
  plume size all come from the charge's yield, so upgrades scale everything.
  It's buried deep enough that the blast loosens a big chunk of ground above into
  debris cutters just scoop up. Proposed: depth ~ k * yield^(1/3), about 75-80 m
  for 5 kt.
- **When:** the director orders at its own discretion when cutters hit harder
  terrain (a stretch eating well below its soft-ground rate). It's always sent where
  it's most effective, with more when the need rises, but never frequent. Fewer,
  bigger booms beat many small ones.
- **Off screen:** a stretch turns into loose rubble by math, with no plume.
- Plumes scale with yield, so a big one shows from far zoom (skipped when it would be
  only a few pixels).
- It may use tunnelers' shafts, but must never catch a tunneler in its blast. It can
  dig sideways to steer clear.

## Tunneler (the opener, the vanguard)

- **It sees behind the fog** and reveals what's around it as it digs, at double the
  usual radius.
- **Targets:** a balance between material truly valuable to the space-bound swarm
  (spoils to leave with) and what the current swarm needs. It takes a more
  accessible, needed deposit first without forsaking the valuable one. Then it
  goes on to the next thing further down.
- **Special targets:** the magma layers around the core and extremely hard
  strata. Without tunnelers, cutters would die endlessly on the first magma
  layer. Through hard strata it simply drills, and cutters eat outward from its shaft.
- **Magma:** it drills to the chamber. If going on would engulf it, it spends a capping
  charge (the plug cools to glass) and marks the spot as a heat-sink job. Then it backs up
  its shaft and branches toward its next target.
- **Path:** up to 20 degrees off vertical, steered as a gradient so tunnels curve.
  Upgrades widen the angle.
- **Spoil:** it's shot upward in a big plume proportional to what it consumes.
  Its own cutter crew (built with it, plus a hauler sized to its dig rate)
  only clears that spoil, so it isn't buried.
- **Drill:** nuclear (built with uranium), needs no fuel, animated in the grimy
  pixelated style. Its heat melts low-melting-point material and transforms it where
  the chemistry applies.
- **End:** with nothing left to do, it self-destructs where it is (deep in the
  mantle by then) in a big nuclear blast, a final gift of matter. Only the yield
  varies. It waits until other tunnelers have moved clear.
- **Shafts** render whenever they'd be visible on screen, approximated far out
  and refined while zooming in.
- Very expensive to build.

## Heat-sink bot

- **Role 1, small pockets:** it settles above a small magma pool and cools it
  completely. Tentacle reach is upgradable, but from the start it covers a small
  pool that would otherwise kill a lot of cutters.
- **Role 2, deep:** under a tunneler's cap, it freezes a roughly vertical passage
  through magma (or another hot liquid), wide enough for a tunneler, so cutters
  can move through and tunnelers can carry on down. It makes a passage, not the
  whole layer.
- **Crew:** its own cutters (built with it) clear the frozen passage so it can keep
  reaching down. They never breach the frozen walls holding the magma back.
- **Heat** only sets how long a cell takes to freeze. Cooling never fails; it
  just slows.
- **No power beaming** for now (the economy is undecided).
- **Danger:** magma does what magma does. Pressure or magma invading from elsewhere
  can hit the bot's top like any other craft.
- **End:** with no next target it powers down and dies, to be reclaimed.
- **Count:** usually one per cap. For a wide pool under a large open cave, more
  go in if they fit, make their own caps and work them.
- Steam is drawn only when it's cheap at that zoom.

## Forester and abductor: the look

The Units page of the swarm foundry mockup (`mockups/swarm-foundry.html`,
`sceneTripod`, open with `open-mockup.bat`) is the basis for both machines'
visuals and animation. The game drifted from it on 12 November:

| | Mockup (the basis) | Game today |
| --- | --- | --- |
| Forester body | crab on **three** sprawling legs, facing forward: rostrum, mouthparts and bladed intake rotor in front, claws reaching forward, clippings bag slung **behind**, eye stalks near the front, laser from the front | **sideways** crab on **four** legs in diagonal pairs, a claw each side, bag **under the middle**, eye stalks in the middle firing the laser |
| Legs (both) | each leg two curves, hip to a knee riding high above the body to the foot, swaying a little on its own; two blocks thick on the forester | inverse-kinematics chains (2 bones on the forester, 4 on the abductor) bending at the joints |
| Gait (both) | a stride cycle, one leg at a time: long fast stride when travelling, a slow creep when working; sags when a leg lifts, jolts as a foot lands, surges while all three are planted | feet step when left too far behind, by gait group |
| Height | body 50 m (forester) / 62 m (abductor) over the ground | 50 m over the tallest plant within 30 m |

The plan is to port the mockup's leg drawing, gait and forester body as they
are. The game keeps only what the mockup can't do: feet found on the real
ground (slopes, ledges), turning about, and sim-driven targets for claws,
palps and laser.

## Forester (harvester body)

- **Mostly for fun.** Its working role is roaming the planet ahead of the
  cutters, clearing plants that might be hostile or hard to reach.
- **Flight:** harvester bodies can travel past the edge of the mother swarm,
  flying a bit like a UFO, then drop, unfold and start work. The director builds
  them as needed and sends them to high concentrations of plant life.
- **Fire** spreads to all ignitable material nearby (falling-sand sim). Off screen,
  a fire is a front moving across stretches by math. Burn gases are lost to the
  sky (gas-harvesting bots may come later).
- **No crew:** ash and puffs wait for whatever cutters arrive.
- **End:** it dies and is reclaimed if reaching the next plant life would take longer
  than building and deploying a new one.

## Abductor (harvester body)

- **Same as the forester**, but for fauna: flight, targeting dense fauna, no
  crew, the same end rule, the same off-screen rule.
- Creatures with a ranged attack get shot or attacked.
- **Its laser never starts fires.** It only bursts fauna into gore and biomass, for fun.
- **It has a large health pool:** almost invulnerable to anything short of a
  technologically advanced sentient race (not built yet).
- Animals are approximated off screen: a herd is a drifting point with a count,
  and the animals are placed when the camera arrives.
- Assumed unless corrected: catches are credited at their own mass (the old 2x
  placeholder goes), and cutters don't hunt animals.

## Couriers

- **Only a picture of the logistics math.** Material moves as flows with
  distance delays. Couriers cost nothing, their animations reflect what the math
  is doing, they can't die, and they fade back into the mother swarm when done.
- **Their only surface job** for now is taking up full forester bags and abductor sacs.
