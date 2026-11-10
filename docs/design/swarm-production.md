# Swarm Production Design

Nov 9, 2026 · @Benjamin

## Overview

All production happens inside the mother swarm: ore comes up, is refined into elements, fabricated into parts, and assembled into new drones, with couriers carrying one block at a time between storage ships and machines. Production is small next to mining but it is what turns a growing stockpile into a growing swarm.

Decided:

- **No surface production.** The swarm shrinks with the planet, so anything built on the ground would be left behind.
- **Parts are a real layer.** Drones are built from named parts drawn from robot fiction (neural cores, behaviour chips, servos), not straight from raw elements.
- **Couriers move blocks, not items.** One trip carries one block of parts from a storage ship to a factory.
- **The director decides, then waits.** It keeps a hidden build queue it can reorder, but nothing appears until a factory finishes it.
- **Energy gates every machine.** Ground units beam power up; in a shortage the director can switch factories to making power instead.
- **Next classes are heat-sink bots and tunnelers**, unlocked later by the meta tree. Magma layers are the soft wall between the swarm and big-planet cores.

## The chain

&#91;embedded content: production chain · 6 stages, couriers between\]

Ore loops from the ground through storage and three machines and comes back down as new drones; the director only chooses what the factory builds next.

## Parts catalogue

Ten parts cover every unit planned so far; each is a fixed mix of elements with a substitute when the main one runs short (the same fallback idea as today's recipes). Masses are first guesses for tuning.

| Part | Role in the lore | Made of (main, substitute) | Mass per part |
| --- | --- | --- | --- |
| Neural core | The drone's mind; every unit needs exactly one | Si, Cu, a trace of Au (Ag if no Au) | 2 kg |
| Behaviour chip | Programs one job (cut, haul, carry, cool, bore) | Si, Cu (Al if no Cu) | 0.2 kg |
| Servo | Moves a joint, leg or cutting arm | Fe, Cu windings (Al) | 1 kg |
| Power cell | Stores beamed energy | Li or Na, Ni (Fe) | 3 kg |
| Frame strut | Load-bearing skeleton | Al (Fe) | 2 kg |
| Armour plate | Hull skin; heavier units carry more | Fe, Ti (Fe only) | 5 kg |
| Cutting head | Tooth of a cutter; wears out | W or Ti, Fe (Fe only, softer) | 1.5 kg |
| Hold module | One cargo cell of a hauler or storage ship | Al, Fe | 20 kg |
| Heat sink | Soaks and sheds heat; core of the heat-sink bot | Cu (Al, worse) | 10 kg |
| Lift vane | Keeps flyers airborne | Al, C (Al only) | 1 kg |

Parts are stored like ore: by kind, in storage ship holds, and counted in the stockpile window on a third tab.

## Unit recipes

Every unit is one neural core, one behaviour chip for its job, and a body; the body is what makes big units slow to build. Build times are at one factory with full power.

| Unit | Parts | Build time | Status |
| --- | --- | --- | --- |
| Cutter | core, chip, 4 servos, 1 power cell, 2 struts, 1 plate, 3 cutting heads, 2 lift vanes | 20 s | in game |
| Courier | core, chip, 2 servos, 1 power cell, 2 struts, 4 lift vanes, 1 hold module | 30 s | in game (free today) |
| Hauler | core, chip, 6 servos, 2 power cells, 12 struts, 6 plates, 8 hold modules, 8 lift vanes | 3 min | in game |
| Storage ship | core, chip, 4 power cells, 40 struts, 20 plates, hold modules to taste (one per 150 t cell) | 10 min + 10 s per hold | in game (free today) |
| Heat-sink bot | core, chip, 2 servos, 2 power cells, 4 struts, 4 plates, 6 heat sinks, 2 lift vanes | 1 min | next |
| Tunneler | core, chip, 8 servos, 3 power cells, 6 struts, 10 plates, 6 cutting heads | 2 min | next |

Today's starting swarm (1,000 drones) is a gift: it costs nothing. Everything after it is paid for in parts.

## Rare parts and premium units

Rare elements buy better versions of the same jobs, never a new resource loop; every premium unit has a common unit that does its job worse. All proposed, open to change.

| Rare part | Made of | What it upgrades |
| --- | --- | --- |
| Lattice core | Ge, Ga, In | A neural core that runs several behaviour chips at once |
| Superconducting coil | Nb, Ti, kept cold by a heat sink | Servos and power beams; far less energy lost |
| Refractory shell | Ta, Hf, W, Re | Armour that survives molten rock |
| Catalyst bed | Pt, Pd, Rh | Refiners: faster, and pull trace elements out of slag |
| Isotope cell | U, Th, with Pb shielding | A power cell that needs no beamed energy, for years |
| Beryllium frame | Be, Al | A strut at half the mass; faster flyers |

| Premium unit | Key rare parts | Does better than |
| --- | --- | --- |
| Magma diver | Refractory shell, heat sinks, isotope cell | Cutters in lava: digs molten rock without melting; far fewer needed than heat-sink bots, but each is costly |
| Seer | Lattice core, beryllium frame | Fog of war: charts deep ground ahead of the cutters, so the finder and director see more |
| Beam relay | Superconducting coils, isotope cell | Energy: gathers ground units' power and sends it up with little loss |
| Deep hauler | Isotope cell, beryllium frame | Haulers: needs no power and flies faster; for long shafts toward the core |
| Core lance | Refractory shell, lattice core, superconducting coils | Tunnelers: the late-game unit that bores through the magma layer into the core |

## Machines in the swarm

Three machine kinds, all flying ships in the swarm's middle layer, built by the swarm block by block like storage ships and drawn in the same voxel style with the shared light colour.

| Machine | Takes in | Puts out | Power mode | Look |
| --- | --- | --- | --- | --- |
| Refiner | Raw material blocks from storage | Pure element blocks, plus slag | — | Squat furnace ship, glowing seams |
| Fabricator | Element blocks | Part blocks (one part kind at a time, switches slowly) | Yes | Long ship with a conveyor of small cells |
| Factory | Part blocks | Finished drones, which drop out of its belly | Yes | Open cradle; the drone is assembled visibly inside |

- **Blocks.** A block is one storage cell's worth: 150 t of ore or elements, or a fixed count of one part kind. A courier carries one block per trip.
- **Input buffers.** Each machine holds two input blocks and one output block, so a courier arriving late stalls it and the player can see why.
- **Built like storage ships.** The swarm grows a new machine when the director asks, paid in plates and struts; the first refiner and fabricator come free with the starting swarm.
- **Power mode.** Fabricators and factories can stop work and turn their power cells into generators (see Energy).

## The director's queue

The director keeps one hidden list of orders and re-sorts it every few seconds; the player never sees the list, only its results arriving.

1. **Orders** name a thing to build: a unit, a machine, a storage ship, or a batch of parts. Each has a priority the director sets.
2. **Re-planning.** Every few seconds the director re-scores the list: storage full → storage ships first; lava ahead → heat-sink bots; haulers idle → fewer haulers. It may cancel or push back any order not yet started.
3. **Pulling parts.** The top order that has all its parts in storage claims them; couriers start fetching blocks to the free factory. An order missing parts asks fabricators for them, which may ask refiners for elements.
4. **Waiting.** A started order cannot be hurried; its time is set by the factory and its power. That wait is the momentum: the swarm feels its earlier choices.
5. **Delivery.** The finished unit joins the swarm and the order leaves the list.

For testing, a cheat key will show the queue in a small window.

## Energy

A machine only works while the swarm's power covers its draw; when it doesn't, machines stop in reverse priority order until it does.

- **Supply.** Ground units (cutters, tunnelers, heat-sink bots) beam power up to the swarm, each a small fixed amount. Heat-sink bots beam more while cooling lava: the heat they soak becomes power.
- **Draw.** Refiners draw the most, then factories, then fabricators. Storage ships and couriers draw nothing.
- **Shortage.** The director picks factories or fabricators to switch into power mode. Switching takes about 30 s each way, so flipping back and forth is costly.
- **Showing it.** A thin power bar beside the matter score; a stalled machine's lights dim and stop pulsing.

Open: the actual numbers, and whether power is stored (power cells) or used as it arrives. My plan is used-as-it-arrives first, storage later.

## Next classes

On big planets a magma layer sits over the core. Cutters can cross it, but they lose so many units doing it that core mining is a soft cap; heat-sink bots in numbers remove that cap.

**Heat-sink bots.** They hover over lava and pull heat out of it, so it freezes back into rock that cutters can take. One bot cools slowly; hundreds cool a shaft. They burn out if left over lava too long, so the director rotates them. Unlocked by the meta tree.

**Tunnelers.** Heavy ground units that bore straight shafts through hard rock instead of digging faces. Slower per tonne than cutters but tougher, so they lead the way down and cutters widen behind them. Unlocked by the meta tree.

**Cutters in magma.** Unchanged: they can dig molten rock, but each load risks the cutter melting. Expected losses are high enough that the director avoids it unless nothing else is left.

The lava bug on the roadmap (lava not flowing into hollows) must be fixed before this is tuned.

## Build order

Six phases after a phase 0, each playable and testable on its own; phase 0 starts next.

**Phase 0, before phase 1: the full element table.** Grow the table from 42 elements to every element except the lanthanides and actinides, keeping uranium and thorium (about 75), and add minerals that carry them. Planet gen places the products of past radioactivity as if billions of years already passed: helium trapped in gas pockets under cap rock, radon near uranium ore, and warm ground around uranium and thorium. Live decay during play stays in the later radioactivity step.

1. **Parts in the sim.** Parts catalogue in a data file, a parts stock, unit recipes in parts, and a Parts tab in the stockpile window. Drones still appear instantly but now cost parts.
2. **Refiner and fabricator.** Two machine ships; couriers carry ore blocks in and element and part blocks out. Parts now have to be made.
3. **Factory and the queue.** The director's hidden queue; new units are assembled visibly and take time.
4. **Energy.** Ground units beam power; machines stall in shortage; power mode on factories.
5. **Heat-sink bots.** Lava cooling, after the lava flow bug is fixed.
6. **Tunnelers.** Shaft boring.

Open questions:

- [ ] Do machines ever wear out or get destroyed, or only storage ships?
- [ ] Should cutting heads wear out and need replacing, as a steady parts drain?
- [ ] Is the meta tree in scope before phase 5, or do heat-sink bots and tunnelers start unlocked for testing?

## Between runs

Everything left in the swarm at launch or core collection goes into a grand total in space, by element; meta upgrades cost cores and large amounts of specific elements.

- Choosing the next planet sends a small share of that total down with the swarm as seed stock, which fast-unlocks part of the per-run tech tree.
- Meta upgrades raise how much seed stock comes along.
- Premium units unlock only through the per-run tech tree.
- Planet selection can promote or discourage bodies by what they hold: see the Element Abundance Master Table.
