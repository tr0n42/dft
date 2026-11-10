# The Dark Forest Theory — Plan

Nov 2, 2026 · @Benjamin

This is the single current plan, version 5, built from all four rounds of your answers. Earlier versions are discarded, and every settled decision is recorded in the decision log at the end.

**The name:** the Dark Forest theory is a proposed answer to the Fermi paradox. The universe is silent because any civilization that reveals itself gets destroyed. In this game, the swarm is the thing in the dark.

## Open questions

Perhaps grant higher tier materials a multiplicative bonus during processing just to make it fun.  Ultimately, simple conservation of energy is kind of boring.  We'll maintain a list of compounds and leverage their bonuses as meta-progression options (e.g. increase obsidian payout by 2x, etc).

Sure, but see my above comment.  Even though the swarm only cares about mass, we add gameplay flavor by adding bonuses to rare earth metals, crystals, compounds, reactions to help provide us with ways of spending hearts on meta progression to boost them further.

Yes

Yes.  Energy should be a significant portion of the initial creation requirement (costs a bunch of energy to reassemble)

None. Round 4 accepted every proposal, and your notes are folded in and recorded in the decision log. M0 to M3 are complete (see their results sections). M3 raised two balance decisions, listed under its results; next is M4, the sandbox.

## Vision and core loop

The swarm is unrelenting but unaligned. It is pure consumption, never a villain or a hero. The player never steers it. They shape it by spending, and its own AI does the rest.

**The player's decisions:**

- buy upgrades, drone classes and tiers
- choose when to launch
- pick the next destination from several offered on the intermission screen
- spend Hearts in the meta tech tree

**The world:** every body is a 2D cross-section disc with the core at its centre. The seed swarm arrives just above the atmosphere, or just above the surface on airless bodies. It holds all processing and fabrication, builds drones with energy, and spreads sideways until it envelops the planet.

**Progression ladder:**

1. **Asteroids.** A short run of asteroids, each a full prestige with no Heart.
2. **A geologically active planetoid.** The first body with a Heart.
3. **Rocky planets.** Each bigger and stranger than the last.
4. **Civilized planets.** Inhabitants from the stone age up to Earth-equivalent technology.
5. **After v1:** sci-fi civilizations, gas giants, stars and Dyson spheres, added as modular extensions.

**The loop:**

1. **Seconds:** drones dig, the sandbox reacts, and drones die. Their remains are revived or consumed.
2. **Minutes to hours:** spend mass and energy to break each growth plateau.
3. **Per body:** reach the launch threshold, ideally claim the Heart, launch, and choose the next destination.

**The one rule:** the player always wins eventually.

## Architecture

The simulation core is written in Rust and runs headless. It's built and tested in this workspace first, moves to GitHub once it proves itself, and ships later as a single desktop app on Steam. The technical decisions are mine to make, and this section records them.

&#91;embedded content: architecture · sim core and its clients\]

- **Deterministic, with a fixed 60 Hz tick.** The same seed and the same inputs always produce the same world, so any bug or balance run can be replayed exactly.
- **Multithreaded from day one.**
  - Chunks update in parallel in four checkerboard passes, so neighbouring chunks never update at the same time.
  - Drone and defender systems split their work across all cores.
- **Every tunable lives in data files.** Planet specs, materials, reactions, drone stats and costs are plain text files that reload while the game runs. They form the parameter API, and a future in-game universe editor would just be a front end over them.
- **Extensions plug in as modules.** Sci-fi civilizations, gas giants and stars will later arrive as new archetypes, materials, reactions and defender classes. None of them needs a rewrite of the engine.
- **The renderer only reads.** It takes snapshots from the sim and never changes its state. It will be built with Bevy, finalised at milestone 6.
- **Hobby scope.** No monetisation, no accounts, no servers.

## Planet generator

One spec format produces every body, from a 2 km asteroid upward. The goal is lots of variety that still looks believable: no planet is Earth, so formations can stretch real geology but should never look random.

### Inputs

| Parameter | Range or type | Drives |
| --- | --- | --- |
| Seed | u64 | All randomness |
| Radius | 1 km to 4× Earth in v1; larger after v1 | Size, time to the core |
| Mean density | 1,000 to 12,000 kg/m³ | Gravity |
| Layer stack | List of {name, depth fraction, base material, hardness, temperature} | Crust, mantle and core bands |
| Element abundances | Rolled per body from its archetype; real elements only in v1 | How much mass there is, and where |
| Ore rules | Richness, vein size, depth bias, host layer | Where the dense seams sit |
| Fluids | Any liquid in the material table | Oceans, aquifers, lakes |
| Underground gases | Pocket density, composition | Explosions, off-gassing |
| Atmosphere | Height and chemistry | A coloured, visual-only band |
| Activity | Magma, cryovolcanism, quakes | Hazard rate, mostly for active bodies |
| Feature densities | One number per catalog stamp | Caves, craters, faults and so on |
| Civilization | Tech level, coverage, defence profile | Settlements and defenders |
| Heart class | One of many; none for asteroids | The prize in the final core pixel |
| Palette | Colour ramps | Look only |

The Heart is the last pixel at the exact centre of the core. It is not a material; it is the meta-progression item.

### Body archetypes for v1

A seeded roll varies every parameter within each archetype.

| Archetype | Character |
| --- | --- |
| Rubble asteroid | Loose and low-gravity. The first prestiges |
| Metal asteroid | Iron and nickel, hard, very dense |
| Active planetoid | Small, young and restless, with magma, quakes and gas vents. The first Heart |
| Rocky world | Layered crust, mantle and core, with water |
| Ice moon | Ice crust over a subsurface ocean |
| Methane world | Hydrocarbon seas, cold, ice bedrock |
| Lava world | Thin crust over magma |
| Iron world | Thin rock over a huge, dense core |
| Carbon world | Graphite layers with diamond bands |
| Ocean world | A global ocean over high-pressure ice |

### Feature catalog (v1)

- **Geology:** strata, faults, caves, aquifers, magma chambers, ore veins, geodes, kimberlite pipes, salt domes, oil and gas pockets, impact craters.
- **Exotic but believable:** cryovolcanoes, methane karst, ice shelves, crystal caverns, glass plains.
- **Civilization stamps:** see the next section.
- **Aesthetics:** fossils, ruins, glowing fungi, crystal glow.

## Planet types (brainstorm)

This is a working list for comment, written before C2 so that the generator, sandbox and chemistry are built for every body we want rather than retrofitted. Each type below says what the cross-section looks like, what can kill drones, what is worth eating, and which systems it needs that don't exist yet. Comment on any row: cut it, change it, or add your own.

### The types

| Type | Cross-section, outside → in | Hazards | Worth mining | Needs that don't exist yet |
| --- | --- | --- | --- | --- |
| Rubble asteroid *(built)* | Regolith, then chondrite rubble with voids, ice pockets and metal boulders | Cave-ins; almost nothing else | Metal boulders, pyrite | Real rubble-pile look: separate boulders of different rock packed with gaps, not just noise |
| Metal asteroid | Thin regolith over solid iron-nickel, with troilite (iron sulfide) nodules, olivine-in-metal patches and platinum-group pockets | Hardness: every cell is slow, so Tunnelers and drill tiers matter | Nickel, cobalt, platinum group, all in bulk | Troilite, pallasite and platinum ore materials; a heavier hauling cost |
| Comet *(new suggestion)* | Dark dust crust over dirty water ice with frozen CO₂ and CO pockets | Heat makes ice turn straight to gas and jet out; jets push drones and loose rock | Cheap water and carbon; easy, fast prestige | **Sublimation** (solid straight to gas in vacuum); CO₂ and CO materials |
| Active planetoid *(built)* | Basalt crust, peridotite mantle, magma chambers, molten iron core, sulfur pockets, kimberlite pipes | Lava, cave-ins, falling rock; gas vents planned | Gold, platinum, diamonds in pipes, iron core | Pressurised gas pockets (M5); quakes are named in the archetype but not designed |
| Rocky world | Soil and sediment layers (sandstone, shale, limestone, coal seams), granite continents or basalt sea floor, mantle, core. Oceans and lakes on top, aquifers below | Flooding from aquifers and seas; gas traps; civilizations | Ore bodies in realistic places: copper near old magma, banded iron in sediment, gold placers in riverbeds, coal | **Strata** that follow the surface and fold; **surface oceans**; ore placement by geology; body-specific gravity; civilizations live here |
| Ice moon (Europa-like) | Ice shell, salty ocean tens of km deep, silicate rock, small metal core | Breaching the shell floods the tunnel with brine pushed up under pressure | Little in the ice; salts, rock and metal in the core | **Settled pools** so a whole ocean sleeps until breached; brine (C2 mixing); a pressure model so water rises through a hole |
| Methane world (Titan-like) | Tholin dunes and sludge, methane and ethane lakes on a water-ice bedrock, then a water-ammonia ocean, then rock. 94 K | Drone heat boils lakes into gas; deep ocean floods; extreme cold | Hydrocarbons as fuel (energy) once an oxidiser exists; ice as bulk mass | Surface lakes; ethane, tholin, ammonia; burning (C3); a cold limit for drones? |
| Sulfur world (Io-like) | SO₂ frost over sulfur flows over silicate crust riddled with lava lakes and magma | Lava everywhere; plumes of sulfur gas; frost turning to gas | Sulfur (cheap); metals carried up by magma | SO₂ material; sublimation; eruptions (M5 gas pressure, C5) |
| Lava world | Thin glass crust over a magma ocean; silicate vapour above | Almost everything is hot enough to kill drones | Glass; metals that sink through the magma and pool | **Settled pools** for the magma ocean (essential for speed); density separation (C2); glass (C4); heat-proof drone tiers |
| Iron world (Mercury-like) | Thin silicate crust over a core most of the radius wide, solid inside, molten outside | Hardness, heat at depth, a liquid-metal ocean | The best metal yield of any type | Alloys where metals meet (C2); settled pools for the molten shell |
| Carbon world | Graphite and silicon carbide crust, tar and methane pockets, diamond deeper, carbide mantle | Graphite burns if oxygen is brought in; gas pockets | Diamonds, a lot of them, deeper = more | **Depth pressure** so diamond forms where it should (C4); graphite, silicon carbide, tar materials |
| Ocean world | Global ocean hundreds of km deep over high-pressure ice over rock | All liquid at the top; the swarm has to work through or around water | Rock and metal under the ice; very little on the way down | Settled pools; **pressure ice** (water that freezes because of depth, not cold) |
| Ice giant interior | Hydrogen-helium gas, a hot water-ammonia-methane "ice" mantle, rock core | Crushing pressure; nothing solid to stand on until deep | Diamond rain falling through the mantle; the core | Gas held by gravity as real layers; pressure everywhere. **Suggest post-v1** |

### Exotic types

These stretch past real planetary science. Each one is built around one new mechanic, so it plays differently and doesn't just look different. Several tie into the Dark Forest theme: the galaxy holds other civilizations and other swarms, and most of them are dead or hiding.

| Type | Cross-section, outside → in | The twist | Worth mining | Needs that don't exist yet |
| --- | --- | --- | --- | --- |
| Derelict megastructure | A shard of something built: armour plating, decks, corridors, machinery bays, a dead reactor at the heart, wrapped in captured rock | Laid out by design, not geology. Dormant defences wake when drones cut into certain rooms | Alien alloys, salvage that unlocks tech, the reactor's fuel | **Prefab stamps** (rooms and corridors placed by rules); salvage items; triggered defenders |
| Shattered world | A planet split by an old impact: a ring of fragments around a broken half with its molten core open to space | Many bodies at once. Fragments drift and fall back; the exposed core is reachable from day one | Core metals with no crust to dig through | **Several bodies in one scene**; gravity from more than one centre |
| Crystal world | Giant geodes, crystal caverns, quartz-like spires through a silicate crust | Some crystals release energy when cut (piezoelectric); stressed crystal shatters in chain reactions that can bring caverns down | Energy as well as mass; gem crystals | **Chain reactions** that spread cell to cell; energy from mining a material |
| Strange-matter world | A normal rocky crust over a core of strange matter that converts anything it touches into more of itself | The core is slowly eating the planet. The swarm races it, and a breach speeds it up | Strange matter itself, extremely dense and valuable if it can be contained | **Contagion rules** (a material that converts its neighbours); containment by a material it can't convert |
| Living world | A crust of biomass with veins, chambers and a heart-like organ deep inside | It heals: dug cells regrow over time, and an immune response sends defenders at the swarm | Biomass as cheap fuel; rare organic compounds | **Regrowth** (cells that come back); defenders without a civilization |
| Antimatter-veined world | An ordinary rocky or icy body threaded with sealed pockets of antimatter held in exotic crystal | Breaking a seal destroys everything nearby, drones included. Handled right, it's a huge energy source | Energy, by far the most per cell of any body | Explosions (C5); a containment material; a way for Scouts to flag danger |
| Frozen rogue world | An Earth-like planet thrown from its star: frozen air (oxygen and nitrogen snow) over frozen seas over a dead civilization's ruins over rock | Heat plus oxygen snow plus hydrocarbons is a bomb. The ruins hold the story of who threw it out | Ruins salvage; frozen oxygen as oxidiser for burning | Burning (C3); ruins as prefab stamps; air ices |
| Rival-swarm world | Any base type, already half-eaten from one side by another gray-goo swarm | A second swarm competes for the same mass and fights back when the two meet. The Dark Forest made literal | Whatever the base type holds, plus the rival's wrecks | **An opponent swarm AI** on the same world; drone combat |
| Hollow world | A thick shell around an empty interior lit by a small artificial sun, with an inner surface facing it | Gravity points outward from the inside surface, so the inner world hangs upside down | The sun's machinery; the inner biosphere | Gravity that isn't toward one centre; an inner light source |
| Stripped giant core | The bare core of a gas giant boiled away by its star: metallic hydrogen over rock and iron, very hot, very dense | Metallic hydrogen is stable only under pressure. Dig too fast and it decompresses violently | Metallic hydrogen as an exotic fuel | Depth pressure; materials that change with pressure, not just heat |

**Scope (your call, Round 11):** v1 builds only the three cheapest: the **crystal world**, the **frozen rogue world** and the **stripped giant core**. They are mostly new materials plus chemistry steps already planned. The frozen rogue world's ruins wait for the civilization work; until then it's frozen air, frozen seas and rock. The other seven stay parked here for later. The generator and sandbox are built so they can be added without rework: body specs take new feature kinds, materials take new rules, and nothing assumes one body or one gravity centre more than it has to.

### Systems the list asks for

In rough order of how many types need them:

- [ ] **Depth pressure:** a pressure value at every depth, worked out from the mass above. It replaces the boiling and melting hold-point shortcut from C1, and gives diamond, pressure ice and deep-ocean behaviour. Needed by 6 types. Cheap: the generator already has a radial temperature profile, so this is the same kind of function.
- [ ] **Settled pools:** large bodies of liquid that sleep as one object until something disturbs them (planned for M5). Oceans, magma oceans, molten cores and big lakes all need it, and it also fixes the slow lava-heavy late game we found on the test planetoid. Needed by 6 types, so it should come forward.
- [ ] **Strata:** layers that follow the surface and fold, pinch out and fault, instead of perfect circles. Needed for rocky, carbon and ice worlds and to make sediment look right.
- [ ] **Surface liquids:** seas and lakes above the rock line, filled to a level in low ground.
- [ ] **Body gravity and temperature from the spec:** today gravity is one number for every body. It should come from mass and radius, and surface temperature from the type (40 K comet to 2,000 K lava world).
- [ ] **Sublimation:** ice, CO₂ and SO₂ turning straight to gas in vacuum. A small addition to C1's phase rules.
- [ ] **Mixtures:** brine, ammonia-water, tar. This is C2.
- [ ] **More material ids:** today ids fit in one byte (255). This list adds about 25 authored materials, and with their generated molten and vapour forms that's roughly 110–130. C2's alloys and mixtures will pass 255, so the switch to two-byte ids should happen before C2, ideally now while the generator is open.
- [ ] **Ore placement by geology:** veins near old magma, placers in sediment, banded iron in strata. Replaces today's "veins anywhere in this radius band".
- [ ] **Shapes and surface features:** elongated and two-lobed asteroids, mountains, volcanoes, rifts, dunes.
- [ ] **Scale test:** generate and play a 100 km and a 1,000 km body to check the generator's cost per cell, the world store, drone search and rigid bodies at that size.
- [ ] **Variable mass per cell** stays in C3; nothing on this list needs it sooner.
- [ ] **Prefab stamps** for built things: rooms, corridors, ruins, reactors (megastructure, frozen rogue world, later civilizations).
- [ ] **Cell rules that spread:** contagion, regrowth and chain shattering all fit one mechanism, a material that changes its neighbours over time.
- [ ] **Several bodies and several gravity centres** in one scene (shattered and hollow worlds).
- [ ] **An opponent swarm** (rival-swarm world). This is the largest item and can come last.
- [ ] **Drone temperature effects:** a work-speed bonus and extra wear from the cell temperature where a drone works (cold, water, ice).
- [ ] **Drones in liquids:** moving through water and other safe liquids, and working the cells around and below them.

### Questions for you

- ~~Which types belong in v1?~~ **Answered:** all 13, ice giant included, plus the exotic types.
- ~~Should cold hurt drones?~~ **Answered:** yes, but more slowly than heat, and with an upside. See below.
- ~~What does the swarm do with a deep ocean?~~ **Answered:** eats it like anything else, and drones can swim. See below.

### Decided: cold, water and life

**Cold.** Heat kills drones; cold wears them out. Working somewhere cold lets a drone shed its own heat faster, so it works faster, but its parts wear out quicker and it reaches the end of its service life sooner. Starting values, to be tuned:

- Below about 250 K, work speed rises steadily, reaching **+30%** at 50 K and below.
- Wear rises with it, reaching **2×** at 50 K and below.
- Cold never kills outright. Heat still does.

**Water and other liquids.** Drones move freely through water and other liquids that aren't hot enough to hurt them (liquid methane, brine). Working submerged, or against ice, gives a smaller cooling bonus than deep cold: about **+10%** work speed, with little extra wear. Drones can reach and work the sea floor from inside the water.

**Oceans.** An ocean is eaten like any other material, as fast as its mass allows. Water is light (920 kg a cell, the same as the ice it melts from, against basalt's 2,900) and soft, so seas go quickly. They still matter: water reacts with heat and other materials (steam, boiling, flooding into tunnels, freezing), and an ocean's floor is where the value is.

**Life (earmarked).** Life generation is planned alongside alien civilizations: procedurally generated complex organisms living on and in suitable worlds, built in the same milestone as the civilizations that might evolve from them. Nothing in the planet work now needs it, but the generator leaves room for organisms the same way it leaves room for settlements.

### What happens next

1. **Viewer:** a web page running the real generator, where you pick a type and seed, see the whole cross-section, zoom in to single cells, reroll, and compare two side by side.
2. **Generator work, one type at a time:** fill the gaps above in the order the types need them, and check each new type in the viewer before moving on.
3. **Then C2 to C4,** tested on those bodies instead of only the planetoid.

**Viewer is live:** [Planet Viewer](https://claude.ai/artifact/DGDJXTtTUNXxRk2LrrJsFx). It runs the game's own generator compiled to WebAssembly (600 KB). A test confirms it draws every pixel identically to the sim's preview renderer. It shows the five existing bodies; each new type will be added there as it is built. Source is in `sim/viewer`.

## Civilizations

v1 covers the stone age up to Earth-equivalent technology. Sci-fi civilizations come later, and they will slot in as modules: new tech levels, structure stamps and defender classes on the same systems, not a rewrite. Civilizations fight back, kill underequipped drones and cause disasters, but they are finite and always lose.

**Built from materials.** Settlements are generator stamps made of real materials: wood, stone, brick, concrete, steel and glass. The swarm consumes a city exactly as it consumes rock, and the city pays out its mass. Foundations, subways, mines and bunkers reach underground, so digging down meets them too.

**Scaled by two inputs.** Tech level, and coverage, which runs from a few villages up to a continuous urban surface.

| Tech level | Settlements | Defences | Scope |
| --- | --- | --- | --- |
| Stone age | Huts, villages | Spears, slings, fire | v1 |
| Medieval | Walled towns, keeps | Bows, catapults, burning oil | v1 |
| Industrial | Cities, mines, rail | Rifles, artillery, explosives | v1 |
| Earth-equivalent | Metropolises, bunkers | Missiles, aircraft, nuclear weapons | v1 |
| Sci-fi (Kardashev I to II) | Megacities, city-planets | Energy weapons, orbital defences | Later module |

**Defenders work like drones.** Each type is a class with parallel data arrays. Defenders are simulated one by one near the camera, and as aggregate packets elsewhere, where off-screen battles resolve as rates.

**Weapons are sandbox events.** A cannon shot, a dynamite charge or a nuclear blast is an explosion and heat event. It destroys drones and terrain alike and can set off floods and gas ignitions.

**Always lose.** Population and arsenal are finite and come from the planet's own mass. Defence capacity falls as structures are consumed, and escalation follows how close the swarm gets.

**Life (earmarked, Round 11).** Civilizations come with life: procedurally generated complex organisms on suitable worlds, with a civilization as one possible outcome. Designed and built in the same milestone.

## World storage and look

Cells are 1 m on every body, and a base drone is one cell. The world is the generator function plus a sparse quadtree of changes. Memory grows with the edge of what has been dug, not with planet size.

- **Index:** a hash map keyed by tree level and Morton code, using int64 cell coordinates. A 5 km asteroid needs 13 tree levels, an Earth-size disc 24, and each doubling of radius adds one.
- **Leaf states:** Procedural (unchanged), Empty, Uniform (one material), or Dense (a 64×64 block of material, temperature and flags per cell).
- **Collapse:** four identical children merge into their parent.
- **Aggregates:** every node caches its remaining kilograms per element, so the periodic table screen and the launch threshold are each a single read.
- **Saves:** a save stores the seed, the spec, the tree and only the changed blocks. Blocks are run-length encoded, then zstd-compressed, and entities are packed arrays. Steam Cloud saves come after v1.

**Look:**

- Each material has its own colour.
- Anything below the surface draws as a darker shade of its own colour, so depth reads at a glance, the way it does in Terraria and Noita.
- The atmosphere is a visual band of vacuum, tinted by the planet's chemistry.
- At full zoom, a cell is 8 px.
- Up to four drones can work one cell at once, one on each face, and they split its mass.

**Drones:** simple shapes in a single colour or tint, darkening with wear. This is a first pass, to be refined later.

## Physics sandbox

The sandbox is the heart of the game. Things just happen without the player: explosions, melting, freezing, off-gassing, floods and cave-ins. Hapless drones die in them. Everything is driven by two data tables, in the style of The Powder Toy and Noita, so adding chemistry never means adding code.

**Material table:** each row gives:

- state (solid, powder, liquid, gas)
- density, in kg per cubic metre; this is also its payout per cell
- hardness
- melting and boiling points
- flammability
- element composition

**Reaction table:** each row gives the inputs, the conditions (temperature, pressure, contact), the outputs, a probability per tick, and the energy released or absorbed.

### Starter reactions

Mass is always conserved. Reactions help the economy in three ways: they release harvestable energy, they gather scattered material into dense pools, and they create products such as obsidian, glass and diamond that carry processing multipliers.

| Inputs | Condition | Outputs | Economic effect |
| --- | --- | --- | --- |
| Lava + water | Contact | Obsidian + steam | Hazard; seals floods |
| Water | Above 100 °C | Steam | Hazard: pressure, burns |
| Steam | Cools | Water | Neutral |
| Ice | Above 0 °C | Water | Collapses ice shelves |
| Liquid methane | Warmed | Methane gas | Off-gassing hazard |
| Methane gas + spark | Contact | Explosion + CO₂ + steam | Energy burst; kills drones |
| Oil + fire | Contact | Burning oil + smoke | Energy over time; fire hazard |
| Acid + metal | Contact | Metal salt + hydrogen | Corrodes drones; flammable gas |
| Rock | Extreme heat | Magma | The mantle advances |
| Ore | Melts | Molten metal pools | Concentrates mass; fast to collect |
| Carbon | Heat and pressure | Diamond | Denser cells, same mass |
| Brine | Heat | Salt + steam | Concentrates mass |

### Fluids and gases

Liquids run in three tiers:

- **Active cells** near activity are simulated in full.
- **Settled basins** collapse into single records.
- **Reservoirs** placed by the generator wake only when breached.

Underground gases are simulated as cells in the same way. There's no atmosphere simulation in v1. Every cell carries a temperature: heat conducts between cells and speeds up drone wear.

### Rigid bodies and destruction

Corpses, large drones and falling building chunks are rigid bodies made of cells. They tumble, stack and crush. The Rapier physics library handles this layer, running in its deterministic mode.

Enough force destroys a body for good: melting, an explosion, crushing, or freezing and shattering. It breaks into loose cells of its materials, which can still be consumed as mass but can no longer be revived.

## Drone swarms

The seed swarm is the only factory, and it can never be destroyed. Drones are its limbs and choose their own targets. Every drone in a class behaves the same way, so each class is one system running over parallel data arrays. That lets the drone count grow until the performance budget sets the limit.

**Seed swarm:**

- It hovers above the atmosphere or the surface and spreads sideways until it envelops the planet.
- It turns energy and mass into drones.
- It always builds at a guaranteed minimum rate, so a run can never stall completely.

**Swarm AI (no player direction):**

- Scouts publish a shared map of targets, hazards and defenders.
- Each class follows flow fields toward its targets, weighted by its tiers and upgrades.
- Smarter AI is something the player buys.
- Drones must stay inside network range. Refiners, Reactors and the Necromancer extend that range, and a drone stranded outside it too long self-destructs.

### Classes

| Class | Size | Job |
| --- | --- | --- |
| Harvester | 1 m | Digs and carries solids |
| Siphon | 1 m | Consumes liquids and gases |
| Scout | 1 m | Reveals targets, hazards and defenders; steers the swarm AI |
| Breacher | 1 m | Slow digger for hard rock |
| Heat-Sink | 1 m | Cools a region so hot material can be collected; wears fast |
| Refiner | Large | Breaks compounds into atoms and books them as mass, so raw rock never travels; extends range |
| Reactor | Large | Regional energy and refuelling; extends range |
| Hauler | Large | Carries refined mass to the seed swarm. Tier sets carry speed on screen and mass per second off screen |
| Necromancer | Large | Revives intact corpses that Haulers deliver, at a large energy cost; extends range |

Tiers Mk I to Mk V raise efficiency, armour, heat tolerance and AI quality. There is no combat class: higher tiers survive defenders, and the swarm wins by consuming them.

### Drone lifecycle

1. **Built** by the seed swarm from mass and energy; energy is the larger share of the price.
2. **Works** off an energy tank, refuelling by processing mass. Refuelling costs a little mass but is far cheaper than building a new drone.
3. **Wears** with use, faster in heat.
4. **Dies** by apoptosis at end of life, by a hazard or defender, or by self-destructing out of range.
5. **Leaves remains:**
   - **An intact corpse,** a rigid body. Haulers can take it to a Necromancer for revival, or it can be refined as mass. A drone that died with an empty tank returns no energy.
   - **Debris,** if it was melted, exploded, crushed or shattered. Debris can only be consumed as mass.

**Scale:** near the camera and the dig front, drones are individuals that flock. Everywhere else they are packets that mine at a rate.

## Economy

There are exactly two currencies, Mass and Energy. Mass is literal kilograms, so a dense cell pays more than a light one. Elements are statistics shown on the periodic table screen.

&#91;embedded content: economy loop · mass and energy\]

Nothing leaves the loop except at Launch. Remains stay in the world until they are revived or refined.

- **Mass:** Refiners break compounds down into atoms and book them as kilograms. Density is the reward. A 1 m cell of gold yields about 19,300 kg, against about 7,900 kg for iron and about 2,700 kg for common rock. The periodic table screen shows the kilograms of every element collected and remaining.
- **Energy:** made by processing mass at the seed swarm, Refiners and Reactors, and topped up by exothermic sandbox events. It sets a hard cap on drones built per second and on the number of active drones.
- **Spending:** drones cost mass and energy, and upgrades, classes and tiers cost mass.
- **Losses:** refuelling burns a little mass. A drone that dies with an empty tank returns no energy. Revival needs a Necromancer and an intact corpse; debris can only be refined.
- **Offline progress:** none in v1. Later it may run until the current body is consumed.
- **Ledger:** every flow is booked from a source to a sink, and mass and energy must balance to zero every tick.

**Processing multipliers:** pure conservation is dull, so refining pays a bonus on special materials. Rare metals, crystals, compounds and reaction products carry a multiplier when processed. For example, a meta upgrade can double obsidian's payout. The credited Mass is the physical kilograms × that multiplier. The ledger books the bonus as its own source, so physical mass still balances exactly. The launch threshold counts physical kilograms only.

- **Starting multipliers:** rare metals, crystals and reaction products start at 1.5× to 3×, and common rock stays at 1×. These numbers are a tuning lever for economy pacing.
- **Codex:** each new material or compound is discovered the first time the swarm processes it. Discovery unlocks that material's multiplier upgrades.
- **Consumption totals:** the ledger tracks the total kilograms consumed of every material, whether element or compound, across each run and lifetime. The periodic table screen and the codex both read from it.
- **Upgrade costs:** each level costs 1.15× the last, tuned in balance runs.

## Prestige, asteroids and Hearts

Every body ends in a Launch. Asteroids are quick prestiges with no Heart; from the first planetoid onward, Hearts carry progress across every run.

### Launch

- **When:** in v1, launch unlocks once the swarm has consumed a set share of a body's mass. The proposal is 50%, tuned per archetype.
- **Later:** players can launch from anywhere. That lets them escape a generated planet they can't handle and pick another.
- **Trade-off:** launching before reaching the core forfeits that body's Heart.
- **Payout:** Seed Mass, legacy = k · (mass converted at launch)^0.5. Seed Mass buys permanent starting advantages:
  - more seed drones
  - higher starting tiers
  - faster first builds

### Asteroids

Each asteroid is a full prestige that pays Seed Mass and no Heart. There are three asteroids of growing size (2 km, 5 km, 10 km), then the active planetoid.

### Hearts

- **What it is:** the final pixel at the exact centre of a core. Reaching it takes the full kit: Breachers, Heat-Sinks, and Siphons for a liquid outer core.
- **Collection:** there are many Heart classes, collected "pokemon style." Lore comes later.
- **Bonus:** collecting a Heart grants its permanent bonus and records it in the collection forever.
- **Meta tech tree:** a Heart's value can be spent in the tree for deeper swarm customisation, specialisation, and higher processing multipliers on materials and compounds. Spending it never removes it from the collection.
- **Value:** rarer archetypes and bigger bodies hold more valuable Hearts.

### Intermission screen

Between bodies, this screen shows:

- the swarm
- run statistics: mass consumed, drones built, lost and revived, time, and whether the Heart was claimed
- the Heart collection
- the meta tech tree
- several candidate destinations to choose from

## Performance

Frame time depends on how much is simulated in full detail, never on how big the planet is. The swarm and civilizations stay at 1 m everywhere. Only the part near the camera gets full physics; everywhere else runs as statistics. We're trying this approach and will adjust from the benchmarks.

The world runs at two levels of detail:

- **Near the camera:** full cell physics, fluids, rigid bodies, individual drones and defenders.
- **Everywhere else:** packets mine at fixed rates, battles resolve as rates, and sandbox hazards become expected loss rates.

Those rates are measured from real full-detail runs in the headless harness. When the camera arrives somewhere, that region switches to full detail and every count carries over.

**Multithreading:** chunks update in parallel on the Rayon job library, using the four-pass checkerboard. Drone, defender and packet systems split across all cores. The target is near-linear scaling up to 8 cores.

### Frame budget at 60 fps (targets to confirm in M0)

| Work (sim threads) | ms per frame |
| --- | --- |
| Individual drones: AI, flocking, digging | 3.5 |
| Cell physics and reactions | 3.0 |
| Fluids and gases | 2.0 |
| Rigid bodies | 1.0 |
| Defenders | 1.0 |
| Packets, economy, ledger, aggregates | 1.0 |
| Headroom for explosions and cave-ins | 5.2 |

Rendering runs on the main thread with its own 8 ms budget.

**Minimum spec (proposed):** a 6-core CPU from about 2020 with integrated graphics, on Windows. Nuclear-weapon events from Earth-equivalent civilizations are the worst case and get their own stress test. If any design choice pushes frame time past budget, I'll warn you with the numbers before building further.

**Camera rule:** where the camera points must never change outcomes. Full-detail and statistical results are calibrated to match on average, and the batch harness checks that they do. The player gets free pan and zoom, plus an auto-follow action cam that jumps to explosions and sieges.

**Game speed:** 1×, 2× and 4×, unlocked in the meta tree. Faster speeds lean more on statistical mode to hold 60 fps.

## Instrumentation and live dashboards

Until there's a renderer, the live dashboard is how you watch the game. It shows the swarm growing, where the energy goes, and what's running slow.

**Recorded every tick:**

- time per system: calls, total and peak
- every mass and energy flow, by source, sink and element
- game counters: depth, mass consumed, drones built, lost and revived (by class and cause), reactions fired, Hearts

**Live dashboard panels:**

- frame budget by system against the 16.7 ms line
- drone population by class
- mass and energy income
- the share of the body consumed, against the launch threshold

The dashboard can also replay any recorded run.

**Batch runs:** many seeds run headless with scripted strategies. I publish a summary report from each batch. The headline numbers are time to launch, time to Heart, and the drone growth curve.

**Gates:** three checks must pass before each milestone closes:

- **Determinism:** the state hashes match.
- **Ledger:** mass and energy balance to zero.
- **Performance:** every stress scenario stays inside the budget. The scenarios are a flood, a gas explosion, 100,000 drones, a city siege and a nuclear blast.

I run the gates here for now. Once the code moves to GitHub, they run there automatically on every change.

## Roadmap

The asteroids double as the tutorial and the first development target, because they're small enough to iterate on in minutes. Civilizations come once the core game works.

1. **M0, benchmark spike.** Build a multithreaded falling-sand grid and a boids swarm, and measure both against the frame budget on the minimum-spec profile. Stand up the dashboard skeleton.
2. **M1, generator.** The spec format, the rubble asteroid and active planetoid archetypes, the first stamps, and `totals(node)` with a test that the parts sum to the whole.
3. **M2, world store and mining.** Quadtree, per-element aggregates, ledger, compressed saves, determinism hashes.
4. **M3, the asteroid loop (headless).** Seed swarm, Harvesters, Scouts, Refiners, energy, packets, launch and Seed Mass. Watchable through the live dashboard.
5. **M4, the sandbox.** Material and reaction tables, temperature, cell physics, rigid bodies, corpses and debris.
6. **M5, fluids and gases.** The three liquid tiers, underground gases, Siphons and Heat-Sinks.
7. **M6, renderer.** Bevy, 8 px per cell, depth shading, the atmosphere band, instanced drones, zoom out to the whole planet.
8. **M7, planets and prestige.** The active planetoid and all v1 archetypes, Hearts, intermission screen and destination choice, meta tech tree, Breachers, Haulers, Reactors and Necromancers.
9. **M8, civilizations.** Stone age to Earth-equivalent settlements, defenders, weapons as sandbox events.
10. **M9, polish and packaging.** UI, sound, Steam build.

Each milestone ends with all three gates passing.

**Current order (Round 11):** M0 to M4 and C1 are done. Next comes planet variety, so the remaining systems are built for every body rather than retrofitted:

1. Planet-type brainstorm (see Planet types above). Done.
2. Planet viewer: the real generator in an interactive web page. Done.
3. Groundwork every type needs: depth pressure, gravity and temperature per body, two-byte material ids.
4. Material catalogue: define every material the 16 v1 types can contain.
5. **Geology pass:** research how other generators place materials (Dwarf Fortress's world history, ore placement in other games, real ore-forming processes). Then replace today's noise-driven placement, which reads as scattered spaghetti, with a short formation history per body: accretion, layering, impacts, volcanism, fluids that deposit ore, sediment. The history runs at a coarse scale and the per-cell generator reads its results, so huge bodies still generate on demand. Existing bodies are retuned on it.
6. New types one at a time, each checked in the viewer.
7. Chemistry C2 to C4, tested on those bodies.
8. Then M5, possibly with settled pools pulled forward, since six planet types need them.

**Status review (5 Nov 2026).** The Round 11 list is done. That covers the groundwork, the catalogue and all 16 planet types. The geology pass was done as G1–G5 (strata, plutons and zoned ore, lenses, plumbing, relief) rather than as a formation history; the effect asked for (ore that follows geology, no spaghetti) is there, and the glamour pass adds the interior variety. Settled pools, surface seas, depth pressure, per-body gravity and heat, two-byte ids, the scale test and surface shapes are done too. M5 is half done: liquids, gases and settled pools work, but Siphons and Heat-Sinks don't exist yet.

Still open from the systems list:

- drones in liquids
- drone temperature effects
- cell rules that spread (growth, decay, contagion)
- mixtures (C2)
- prefab stamps
- several bodies in one scene
- the rival swarm
- two-lobed asteroids

**Proposed order (Round 12):**

1. **Benchmark on the PC:** `run-benchmark.bat` and the viewer's Benchmark button.
2. **Physics interactions (P1):**
   - drones in liquids, and drone temperature effects
   - cell rules that spread
   - burning and the first mixtures (C2)
3. **Life L1, plants and the living ground:** soil life, grass, forests, roots, peat, reefs and microbial mats, generated as layers and grown or decayed by the spreading rules. Any type can host it.
4. **Economy pass:** now that the planets, materials and plant life are set.
5. **M6, the game client and renderer:** Bevy, with the viewer's textures and backdrop.
6. **M7:** prestige, Hearts, the intermission screen and the new drone classes.
7. **Life L2 and M8:** animals and civilizations together, as planned.

**Decided (5 Nov 2026):** the game client (M6) moves to the front. Physics, life and the economy get built and judged inside the real game. The viewer is frozen as a development tool.

## M6 plan: the native game client

A Windows program built with Bevy 0.20, Rust's game engine, on the same simulation library the tests and the viewer use, so a seed plays out identically everywhere. It has no player UI: that will be designed from scratch later. Developer tools get a plain debug panel behind F1, and only there.

### How it's built

- **Two threads.** The simulation runs on its own thread at a fixed 10 ticks per game second times the speed. After each step it hands over what changed: the cells, drone positions and the events worth watching. The drawing thread never waits on it, so the picture stays at 60 frames a second whatever the simulation is doing.
- **The world on the graphics card.** The world is cut into 64 × 64 blocks (the same blocks the world store uses). Each visible block lives on the card as a small grid of material numbers, and a shader turns them into colour: the palette, the block textures, bevels and depth shading, all moved over from the viewer. Only blocks the swarm changed are sent again. Zoomed out, coarser versions of the blocks stand in, so the whole planet draws as cheaply as a close-up.
- **Backdrop and sky in the shader.** Behind eaten ground the ghost of the original rock, with parallax; space, stars and the atmosphere band behind the planet.
- **Drones** drawn as tiny sprites, moved smoothly between simulation updates, so motion is fluid at any speed.
- **Saves** written to disk with the existing save format, plus an autosave.
- **Self-check.** A `--screenshot` mode renders set views off-screen and writes PNG files, so I can check the picture without a screen. The camera director and fog logic are plain code with tests.

### The camera

- **Zoom that grows with the dig.** At the start you can only zoom out to see the swarm's first landing zone and the ground around it. The limit opens up as the explored area grows, until the whole planet fits. Zooming in to single blocks is always allowed.
- **Action cam** (on by default). A director looks at what the simulation reports and keeps a running score of what's worth watching:
  - flocks of drones at work, found by grouping drones every second
  - cave-ins and falling rock
  - lava or water breaking into tunnels, steam and boiling
  - rich ore or a geode opened
  - a new landing zone
  - drones lost to heat or crushing
  - launch

  Each shot holds for at least about 8 seconds. Places shown recently score lower, so it moves around the planet. Moves are smooth: a spring eases the camera's position and zoom. Long jumps pull out and swing over, then drop in. Each shot is framed to fit the flock, leaning toward where it's heading.
- **Manual control.** Any mouse drag or wheel turn takes the camera over: the right mouse button drags, the wheel zooms toward the cursor. The action cam comes back after about 20 seconds without input, or straight away with a middle click.

### Fog of war

Three states, drawn in a style rather than as black squares:

- **Uncharted:** dark, with a faint stippled or contour-line pattern hinting at the body's shape, like an old sonar chart. The planet's outline is always visible from orbit.
- **Charted:** places drones have been but aren't now, kept as a cooler, dimmed memory of what was seen.
- **Live:** full colour around drones and landing zones.

The edges are soft and slowly moving, with a faint glowing front where new ground comes into view. Drones reveal a circle around them (Scouts a wider one) and every landing zone reveals its patch. Fog is visual only at first; it could later shape what the swarm knows.

### Steps, each one a build you can run

1. **G1, window and planet:** the planet drawn from the block grids, with camera pan and zoom, the whole-planet view, and the screenshot mode. `run-game.bat` builds and starts it.
2. **G2, live simulation:** the simulation thread, changed blocks streamed in, smooth drones, speed keys (pause, 1× to max), the F1 debug panel (frame time, simulation cost, body, size, seed and glamour pickers).
3. **G3, looks:** the block textures, bevels, palettes, backdrop, space and atmosphere in the shaders.
4. **G4, fog of war.**
5. **G5, camera:** the action cam, the growing zoom limit and manual control.
6. **G6, saves and a speed pass:** saves and autosave, then a benchmark on your PC against the browser numbers.

After M6, the order goes on as planned: the physics batch, then plants and the living ground, then the economy, then prestige (M7), then animals with civilizations.

## M0 results

Both M0 targets are met with room to spare on your i7-14700F under Windows. Determinism holds: every scenario produced identical worlds at every thread count from 1 to 28, on Windows and Linux alike. The code is in your source folder under dark-forest-theory\\m0-benchmark, and run-benchmark.bat repeats the test.

&#91;embedded content: M0 Windows runs, 2 Nov 2026 · your i7-14700F, all 28 threads available · before = run 1, after = run 3\]

&#91;embedded content: M0 benchmark runs, 2 Nov 2026 · your i7-14700F (2 cores in the Linux VM) and the cloud workspace\]

Drone cost grows in a straight line with drone count. That came from one fix: sorting drones in memory by grid cell, which halved their cost.

| Measure | Your i7 on Windows | Target |
| --- | --- | --- |
| Sandbox, 2.9 million moving cells | 0.97 ms on 28 threads; 2.37 ms on 6 | 3 ms for 1 million |
| Drones, 100,000 | 2.0 ms (best, 12 threads) | 3.5 ms |
| Drones that fit in 3.5 ms | About 165,000 | 100,000 |

The stress scenario is harsh: about half of all cells move every tick, far more than a settled planet.

**Headroom:** on just 6 threads the sandbox handles about 3.7 million moving cells inside its 3 ms budget, so a 6-core minimum-spec machine should clear the 1 million target even with slower cores. Drones stop speeding up past about 8 threads, because one step of building their neighbour grid runs on a single thread. That doesn't matter at 100,000 drones, and it's the first thing to make parallel if we ever need more.

**The fix that unlocked scaling:** every moving cell used to mark its neighbouring regions as active, one cell at a time. All the threads ended up fighting over the same memory, which is why adding threads made the sandbox slower. Now each region reports once per tick. The final worlds are byte-identical to before the fix, so behaviour didn't change, only speed.

**What M0 changed:**

- Drones are always stored sorted by grid cell.
- Regions report activity once per tick, never once per cell.
- Floating-point drone maths stays deterministic so long as we avoid fused multiply-add and library trig functions. Fixed-point remains the fallback if cross-CPU checks ever disagree.
- Benchmarks run natively on Windows through run-benchmark.bat, and each run's results file is kept for comparison.

## M1 results

The generator builds both v1 starter bodies from a data file and a seed, and all 7 automated tests pass. The code is in your source folder under dark-forest-theory\\sim. Double-click run-planetgen.bat to rebuild the bodies, or run-tests.bat to rerun the tests.

&#91;image: Rubble asteroid, whole cross-section\]

*Rubble asteroid, 2.5 km radius: chondrite rubble with cave networks, ice pockets (pale blue), metal boulders, thin pyrite veins and craters.*

&#91;image: Active planetoid, whole cross-section\]

*Active planetoid, 30 km radius:*

- iron-nickel inner core (grey), with the Heart marked at the centre
- molten outer core (orange)
- peridotite mantle (green) with magma chambers
- basalt crust with ore veins and kimberlite pipes
- a thin sulfur haze

&#91;image: Rubble asteroid interior at 1 m per cell\]

*Asteroid interior at 1 m per cell (512 by 384 m): an ice pocket, metal boulders, voids, pyrite veins, and cave tunnels that taper shut.*

&#91;image: Planetoid crust and mantle at 1 m per cell\]

*Planetoid at 1 m per cell: mantle (left) meets crust along a rough boundary, a pyrite vein runs through, and a magma chamber (orange) cuts in.*

| Measure | Rubble asteroid | Active planetoid |
| --- | --- | --- |
| Radius | 2.5 km | 30 km |
| Quadtree depth | 8 levels | 11 levels |
| Solid cells (exact) | 15.1 million | 2.73 billion |
| Cross-section mass (exact) | 48.5 Mt | 9.97 Gt |
| Estimate error, total mass | −0.041% | −0.024% |
| Whole-body estimate, 1 million samples | 7 ms | 20 ms |
| Exact count of every cell | 0.35 s | 91 s |

Timings are from your i7-14700F on Windows. All 7 tests pass there, and the asteroid's exact report is byte-identical to the Linux run, down to every one of its 15 million cells.

The estimate is excellent for totals and weak for rare single-cell materials. Gold nuggets on the planetoid were estimated at 1.11 Mt against 1.95 Mt exact, 43% low, because a million samples hit only a handful of nuggets. M2 will count sprinkled materials from their known odds (chance × host cells) rather than by sampling, so rare payouts stay accurate when the swarm mines coarsely.

**What M1 settled:**

- Materials and bodies are pure data files. A new body is a copy of a file with new numbers, no code.
- Every material carries a real density, an element breakdown, and a processing multiplier, so the periodic table screen and the codex already have their numbers.
- The generator uses only plain arithmetic and square roots, never trig, so worlds match bit for bit on every machine.
- Cave networks fade at their edges rather than cutting off, after an early version left ruler-straight walls.

**Tuning to revisit:**

- Ore richness is a first guess. Platinum ore is 0.65% of the planetoid and gold about 0.01%; balance runs in M3 will set real values.
- Flat expanses of rock look bland up close. Per-cell colour variation in the renderer (M6) will fix that.
- The asteroid's surface gravity is about 1/5,000 of Earth's, which is real for a 5 km body. Whether drones feel that weak a pull is a gameplay call for M3.

## M2 results

The world store, mining, ledger and saves work on both bodies. All 15 tests pass, the books balance to within a hundredth of a kilogram, and saves reload identical to the bit. On your i7 under Windows, all 15 tests pass and both demo worlds finish with world hashes identical to the Linux run, so mining, the books and saves are deterministic across machines too. The whole demo runs in 3.0 s there, against 14.8 s on the cloud machine.

&#91;image: Rubble asteroid after the M2 demo\]

*The asteroid after the demo: 40 tunnels dug cell by cell (thin radial lines) and 12 surface regions strip-mined whole (dark squares).*

&#91;image: A 200 m cavern dug cell by cell\]

*A 200 m cavern dug cell by cell, cutting through natural caves and pyrite veins.*

| Measure | Asteroid demo | Planetoid demo |
| --- | --- | --- |
| What was mined | 40 tunnels, 12 regions whole, a 200 m cavern | A 30 km shaft to the Heart |
| Mass mined | 6.44 Mt, 13.3% of the body | 411 kt, Heart claimed |
| Cell operations | 571,000 | 90,000 |
| Store entries | 2,429 | 3,781 |
| Save on disk | 149 KB | 58 KB |
| Estimate correction | +0.15% | +0.05% |

**How it works:**

- Every region holds its remaining kilograms per material, and a parent always equals the sum of its children. Planet remaining, the launch threshold and the periodic table screen are each one read.
- Untouched regions carry estimated shares. The first time a 64 m block is generated exactly, the difference is booked as a correction, so physical mass stays exact.
- Fully mined blocks collapse, and four empty neighbours merge into one empty parent.
- Saves hold only changes and the books. They are run-length encoded, then deflate-compressed (pure Rust, so no C build tools; zstd was the plan). A fingerprint stops a save loading onto the wrong body.
- A test proves the world is identical at 1 thread and at all threads.

**Fixed along the way:**

- **Rare deposits** are now estimated by their odds rather than by sampling. The planetoid's gold estimate is within 2% of exact, down from 43% low.
- **Thread timing** changed the world. Parallel estimates added fractional numbers in whatever order threads finished, and floating-point addition depends on order. Estimates now add in fixed-point integers.
- **Large-region estimates** carried about 1% sampling error, inherited by everything beneath them. They now use up to 65,000 samples, and corrections fell under 0.1% in random tests.

**Watch items for M3:**

- **First touch is expensive.** Generating a fresh region costs roughly 1–10 ms; the planetoid shaft opened 940 regions in 1.8 s on your i7. Plan: open regions on worker threads ahead of the dig front.
- **Digging cost.** Digging costs about 1.9 µs per cell on your i7, counting the first-touch cost of new regions, so 100,000 cells per second would take about 3 ms per frame. Plan: pass the bookkeeping up the tree once per tick, not once per cell.
- **Square scars.** Mining whole regions leaves square holes, because regions are squares. Off-screen packets should eat regions from their exposed edges inward, so the camera never finds blocky scars.
- **Open pits draw dark.** Pits open to space still draw as underground voids. The renderer (M6) needs to know which voids connect to the sky.

## M3 results

The asteroid loop plays from landing to launch, headless, with a scripted player. It launches in about 30 minutes and simulates a full game in about 13 seconds. All 20 tests pass, including a run that's identical at any thread count, books that balance to the kilogram, and a swarm that never stops building.

&#91;embedded content: asteroidloop runs, 2 Nov 2026 · rubble asteroid seeds 1001 to 1005 · 2-core cloud machine\]

Every asteroid follows the same shape: a slow first few minutes, an explosive climb to about 100,000 drones by minute 10, then a long steady middle as each new drone costs more energy, until launch at 50% consumed.

&#91;image: The rubble asteroid at launch\]

*At launch: the swarm eats the asteroid from its landing side inward and spreads around the rim as it grows. The jagged front is made of 64 m blocks mined whole by packets.*

| Measure | Typical run |
| --- | --- |
| Launch | 28:27 to 33:10 across five asteroids |
| Peak drones | About 150,000 (2,000 individual, the rest in the packet) |
| Drones built / worn out | About 169,000 / 19,000 |
| Scouts / Refiners at launch | About 1,600 / 18 |
| Seed Mass paid at launch | About 4,900 |
| Upgrades bought | 300; each type reaches level 72 to 77 |

**What M3 built:**

- **The seed swarm.** Landing points around the rim open one by one as the body is consumed. A free Harvester arrives every 60 s no matter what, so a run can't stall.
- **Drones.** Harvesters cut one cell per trip and carry it home. Scouts steer targeting toward dense ore. Refiners appear when hauls get long, become delivery points, and add processing energy.
- **Mass and Energy.** Delivered mass is credited with its multiplier and processing makes energy. Building costs energy, and each new Harvester costs a little more than the last, which is the main brake on growth.
- **Packets, the camera rule's first form.** The first 2,000 Harvesters are simulated one by one. The rest work as a packet that mines whole 64 m blocks at the rate the individuals are measured achieving.
- **Launch** at 50% consumed, paying Seed Mass = √(Mass credited, in tonnes).
- **Every number in data/economy.toml,** so balancing never needs code.

**Found and fixed:**

- **A stalled start.** Landing points hovered over where the surface would be without craters. Asteroid 1003 landed over a deep crater, its first drones flew 100+ m per cell, and it limped for 12 minutes. Landing points now hover over the actual ground, and launch times tightened from 28–40 to 28–33 minutes.
- **Simulation speed.** Drones searched for work from their landing point, scanning ever more ground as pits deepened. They now look near their last dig, and a fast cell hash made a full game about 25× quicker to simulate.

**Decisions for you:**

- **Upgrades are too strong and too cheap.** Resolved in Tutorial pacing below. Each level is now a ×1.5 jump that costs 8× the last, so about 10 purchases in an hour.
- **Growth flattens instead of bursting.** Partly resolved below. Purchases now land in steps at each check-in, and Scouts and Refiners unlock at about minutes 10 and 40. Hand-tuning the feel of the bursts waits until the game can be seen (M6).

**Deferred:**

- **Pathfinding.** Drones fly in straight lines.
- **The live dashboard.** M3 writes a CSV per run instead.
- **Square scars.** The packet's blocky front is still the M2 watch item.

## Tutorial pacing (M3 follow-up)

The first tutorial now takes about an hour and the second about 30 minutes, with under 10,000 drones and about 10 purchases each. That fits an idle game the player checks on every 10 minutes. Simulated on the Linux machine, which now counts as minimum spec.

**What changed:**

- **An idle player.** The scripted player checks in every 10 minutes, buys at most 2 things, and launches at the first check-in after launch is allowed.
- **Fewer, bigger upgrades.** Each level multiplies an ability by 1.5 and costs 8× the last. The useless Cargo upgrade was replaced with Fabrication (cheaper drones).
- **Drone loads and caps.** A load is about one cell of rock, and dense ore takes two trips. Up to four drones work a cell, but no more than it has loads left.
- **Faster drones, sooner spread.** Drones fly at 20 m/s, and the swarm opens a new landing point every 0.5% consumed.
- **Prestige perks, bought with Seed Mass.** Mk II drones (3× flight and dig speed), a larger seed swarm (+100 drones), and efficient fabrication (half-price drones).

**Four ways to pace the second tutorial, tested.** Each was run after tutorial asteroid 1 (2 km) on the same seed, with the idle player.

| Second tutorial | Ready to launch | Peak drones | Verdict |
| --- | --- | --- | --- |
| A: 5 km, with prestige perks | Not within 2.5 hours | 13,000 | Too big |
| B: 3 km, no perks | 113 to 127 min | 11,100 | Too slow |
| C: 5 km, launch at 25% | Not within 2.5 hours | 10,600 | Too big |
| D: another 2 km, with perks | 30 to 33 min | 8,600 to 10,100 | **Adopted** |

Asteroid 1 itself is ready at 63 to 66 minutes, peaks at 8,300 to 9,700 drones, and earns about 2,100 Seed Mass. That buys Mk II drones and the larger seed swarm, and the second asteroid is then ready in half the time.

**Why A and C can't work under 10,000 drones:** 10,000 drones move at most about 2,000 tonnes a second. Half the 5 km asteroid is about 24 million tonnes, so it needs at least 3.3 hours however the perks are tuned.

**Open question:** the plan's asteroid ladder (2, 5 and 10 km) doesn't fit 30-minute runs under 10,000 drones. The 5 km and 10 km bodies need either more drones (packets make that cheap to simulate) or much larger per-drone gains from prestige.

## Orbiting swarm

The swarm used to eat each body from the top down, starting under the spot where it arrived. Now the seed swarm circles the body once every 10 minutes and drops a landing point as it passes each spot on the way around. New drones start from wherever the swarm is at that moment, so after one orbit there are drones working all the way around the body.

&#91;image: Tutorial asteroid 1 eaten top-down (left) and with the orbiting swarm (right)\]

*Left: the old top-down dig, at launch. Right: the orbiting swarm, at launch. What's left is a rough core in the middle instead of the bottom half of the asteroid.*

**How it works:**

- **Orbit:** the swarm drops a landing point the first time it passes each of the 64 spots around the body. Landing points sink with the ground every minute, so drones keep short trips.
- **Peel from the outside:** a drone treats each metre of depth below the original surface as 3 m of extra flight. So it takes the high ground first, wherever that ground is.
- **Work-sharing:** if a landing point has more than 1.25× its share of drones, a drone that just delivered moves to the least busy one. This keeps any one side from getting ahead.

**Measuring roundness:** the *peel score* is the share of what's left that lies outside the circle a perfect outside-in peel would leave (0% = perfectly round). Several simpler measures were fooled by specks of rock or by pits, so this one is the measure now used.

|  | Top-down (before) | Orbiting swarm (now) |
| --- | --- | --- |
| Peel score at launch | 95% | 12–17% |
| Tutorial asteroid 1 ready to launch | 63–66 min | 59–62 min (after retuning) |
| Tutorial asteroid 2 ready (with perks) | 30–33 min | 29–35 min |
| Peak drones, asteroid 1 | 8,300–9,700 | 7,300–8,500 |
| Purchases, asteroid 1 | 7–9 | 7 |

How fast the swarm orbits and how strongly drones prefer high ground made little difference. Work-sharing is what made the shape round.

**Retuning:** the orbit makes drones much more efficient, because every trip is short. The first asteroid dropped to 26 minutes, so it was slowed back down:

- Digging is 4× slower (4.8 s per point of hardness per load, up from 1.2 s).
- Upgrades cost 5× more, so a player still buys only about 7 things.
- Mk II drones now give 2× speed and dig rate instead of 3×. With 3×, the second asteroid came in at 20 minutes.

The test suite now also checks that tutorial asteroid 1's peel score stays under 30%.

**10,000 drones is not a cap.** No rule stops the swarm at 10k. Each drone alive makes the next one cost a little more energy, so the swarm levels off near 10k on a tutorial asteroid at about the time launch opens. On bigger bodies it can grow past that. This also answers part of the open question above: the 5 km and 10 km bodies can simply run with more drones.

## Dig difficulty and drone intelligence

Every cell now takes time to eat according to its hardness and density. There are no hard stops by layer. Drones start out choosing almost blindly and get better at choosing with each level of a new upgrade, **swarm intelligence**. Lava can't be dug; it has to cool first.

**How it works:**

- **Dig time:** a load of hardness-3 rock (chondrite) takes 14.4 s. Each point of hardness multiplies that by 1.6: basalt (4) takes 23 s, iron (7) 94 s and diamond (10) 386 s. Denser rock fills a load with fewer cells. The dig speed upgrade and Mk II drones divide every time.
- **Sense:** each level of swarm intelligence sharpens how well drones judge a cell. They first learn effort (how hard and heavy it is), then value (what it refines into). A blind drone thinks every cell is average, so it takes the nearest. A sharp drone picks the cell that pays the most per second, counting the round trip.

  | Level | Sees effort | Sees value |
  | --- | --- | --- |
  | 0 (start) | 0% | 0% |
  | 1 | 60% | 0% |
  | 2 | 100% | 40% |
  | 3 | 100% | 70% |
  | 4 | 100% | 90% |
  | 5 | 100% | 100% |
- **Patience:** a drone that can judge effort turns down cells that would pay under 35% of ordinary rock. Hard ore it can't cut well yet is left standing until dig upgrades bring it within reach.
- **Scent:** drones that can judge value also smell richer rock a few metres behind each cell, which draws them toward buried pockets.
- **Scouts:** while Scouts are out, the swarm chooses as if it were one level smarter and smells ore up to 20 m away.
- **Lava:** liquids can't be dug like rock. An exposed face cools into solid rock: magma into lava-rock in 2 minutes, molten iron into cast iron in 5. A drone may swallow lava only at the tip of a pocket, and it burns 300 s of its life. Blind drones do this; drones that can judge effort never do.

&#91;image: The test planetoid fresh (left) and after 75 minutes with a fully smart swarm (right)\]

*The new 3 km test planetoid, fresh and after 75 minutes. The crust is gone. Magma chambers stand as towers, because each exposed face has to cool before it can be eaten.*

**Test planetoid (3 km):**

- soft regolith and sulfur near the surface
- a basalt crust with gold-quartz veins
- a peridotite mantle holding pyrite and sperrylite veins, iron masses and 14 magma chambers
- kimberlite pipes with diamonds
- a molten iron outer core around a solid iron core

**Results** (75 minutes, with Mk II drones and the other perks):

|  | Blind | Sees effort | Sees effort and value |
| --- | --- | --- | --- |
| Body consumed | 34.8% | 35.2% | 35.0% |
| Lava loads swallowed | 525 | 0 | 0 |
| Diamonds (hardness 10) taken | 49% | 19% | 38% |
| Gold-quartz taken by 30 min | 32% |  | 37% |

The mechanics work. Smart drones stay out of lava. They leave diamond rock alone until they can see its value, and then they go back for it.

**But overall the choice barely matters yet.** About 10,000 drones outnumber the cells open on a 3 km body's dig face, so every open cell gets taken by somebody and the order hardly changes the outcome. Visible terrain so far comes from lava, not from hard rock. For intelligence to really shape the ground, the swarm needs something it can choose *between*:

- **Tunnelers:** a drone class that ignores the outside-in rule and follows scent, digging narrow shafts toward buried pockets ahead of the main face.
- **Bigger bodies:** on a 10 km body, 10,000 drones no longer cover the whole face, so judging cells starts to pay.

**Also changed:**

- Drones look at most 160 m from their landing point. One that finds nothing moves on to the next landing point and waits longer each time (5 s, then 10, 20, 40, 80). Without this, a crowded face made the sim about 10× slower.
- The tutorials still pace correctly. Asteroid 1 is ready in 63–67 min and asteroid 2 in 32–36 min, with about 8 purchases and peaks under 8,600 drones. Iron boulders on the asteroids are now hard enough to stand as terrain.
- Fixed a reporting bug: an all-empty block counted as zero cells instead of empty space, so the planet generator's parts-sum-to-whole check failed on small bodies.
- Two new tests: cooling lava neither makes nor destroys matter, and harder rock always takes longer to dig. All 23 tests pass.

**Known gaps:**

- Packets (drones beyond the 10,000 simulated one by one) still eat whole 64 m blocks, lava and hardest rock included. That leaves square notches.
- Lava doesn't flow until the physics sandbox (M4).

## Tunnelers

A Tunneler is a big boring machine that sinks shafts into the body, like the drill ships in The Matrix. It is built to open the way for the flock, not to earn. Its drill is strong but energy-hungry, and most of what it cuts is crushed to waste.

&#91;image: The test planetoid with Tunnelers (left) and a close-up of the chambers along one shaft (right)\]

*Left: the test planetoid after 75 minutes, with shafts and the chambers Harvesters have hollowed out along them. Right: a close-up at 1 m per pixel.*

**How it works:**

- **Unlock and build:** unlocked for 1.2 Mt of Mass. The swarm then keeps about one Tunneler per 1,500 Harvesters, up to 3. Each costs 50 t of Mass and 20,000 E.
- **Drill:** it bores a shaft 3 m wide and cuts rock 25 times faster than a Harvester. It burns 60 E per tonne cut, five times what processing a tonne gives back. Only 20% of the rock it cuts is recovered as Mass.
- **Pathing:** it bores mostly straight down. It leans at most 25° from vertical and turns at most half a degree per metre. A swarm that can judge value steers it toward the richest-smelling rock up to 200 m ahead; a blind swarm just goes straight down.
- **Stopping:** it stops at lava, at 600 m of shaft, or before reaching the inner 30% of the radius. It then takes 2 minutes to pull out and start a new shaft elsewhere around the body.
- **Opening the way:**
  - Rock within 4 m of the shaft is cracked. Harvesters cut it in half the time, and it carries no depth penalty.
  - Every 150 m down the shaft, the Tunneler leaves a staging point.
  - Work-sharing sends Harvesters there, and they hollow out round chambers from it instead of peeling from the outside.

**Results** (test planetoid, 75 minutes, perks, a fully smart swarm):

|  | No Tunnelers | Tunnelers from the start |
| --- | --- | --- |
| Income (Mass credited) | 8.69 Mt | 9.04 Mt (+4%) |
| Peridotite (mantle) taken | 13.0% | 17.7% |
| Sperrylite (deep platinum ore) taken | 0.0% | 7.7% |
| Iron masses taken | 0.9% | 4.8% |

Tunnelers add only about 4% to income. What they change is *where* the swarm works: it gets into the deep mantle and its ore, which it otherwise barely touches in 75 minutes. That's their role: access, not income.

**Fixes made along the way:**

- **Round reach:** drones used to search in square rings and stop partway through one. That carved square-edged pits whenever a hub sat inside the body. Drones now weigh every cell within a circle.
- **Chambers instead of boxes:** drones working from a shaft no longer use the outside-in rule. Applied deep inside the body, that rule gave flat-walled boxes; now they hollow out round chambers.
- **Packets stay on the surface:** packets work only from surface landing points and stay out of cracked shaft rock.
- **Pickiness without idling:** a drone that can judge effort first skips badly paying rock. If nothing better is in reach, it takes what there is rather than standing idle. Drones that can judge effort never swallow lava.

**Tutorials and tests:**

- **Pacing holds:** asteroid 1 is ready in 62–66 min and asteroid 2 in 32–36 min.
- **Tunnelers rarely matter on tutorials:** the idle player unlocks them only at the launch check-in on asteroid 1, if at all.
- **Tests:** a new one checks that Tunnelers bore down, cost more than they earn, and leave the books balanced. All 24 tests pass.

**Watch item:** sharing work with the shafts pulls Harvesters off the surface, so thin strips of crust are left standing between landing points. It looks like terrain, but it slows how much of the body gets consumed. Hand-tuning in M6 should decide how strongly shafts draw drones.

## M4a results: the sandbox

The first half of M4 is in: falling-sand cell physics, temperature and the reaction table, all driven by data and wired into the swarm. Rigid bodies, corpses and debris come next, in M4b.

&#91;image: Sandbox demo: a corridor lets water run down to a magma pocket, and loose rubble caves into a cavity\]

*The sandbox demo, two minutes. Top: loose rubble pours into a cavity dug under it. Right: water runs down a corridor toward a magma pocket, freezing where it meets open space; where the two touch, the lava quenches into rock. The pocket's skin crusts over against cold rock.*

**How it works:**

- **Only what's active is simulated.** A cell wakes when something beside it changes (a drone digs next to it, or a neighbour moves or reacts) or when a neighbour is much hotter or colder. It goes back to sleep after a tick in which it did nothing. Cells that are only cooling or warming step once a second instead of ten times.
- **Motion is toward the centre.** Powders fall, liquids fall and spread sideways, gases rise. Heavier liquids sink through lighter ones, and powders sink through liquids. Gas that rises off the body is lost to space and booked as vented.
- **Heat:**
  - Temperature is tracked only for cells that can change phase or flow. Ordinary rock acts as a heat bath at the body's own temperature: a smooth profile through its layers, hottest at the core.
  - Open space pulls hot cells toward 200 K. How fast heat moves depends on each material.
  - A liquid pocket in rock colder than its freezing point starts a margin above freezing, so magma chambers in a cool crust are still molten until disturbed.
- **Phase changes** happen at each material's melting, freezing, boiling or condensing point:
  - magma ↔ lava-rock
  - molten iron ↔ cast iron
  - ice ↔ water ↔ steam
- **Reactions:** two touching cells react with a set chance per tick, and the products share the inputs' heat. Starter reactions:
  - lava quenched by water
  - lava meets ice
  - molten iron quenched by water
- **Mass is never made or lost.** Cells move by swapping. Every phase change goes to a material of the same density, and every reaction's inputs weigh what its outputs do; the data tables are checked for this when they load. A steam cell therefore holds the vapour of one cell of water, and meltwater weighs what its ice did.
- **Determinism:** cells step nearest the centre first, and every random choice comes from a hash of the tick and the cell. Runs are identical every time.

**In the swarm:**

- The timer-based lava cooling stand-in is gone; lava now cools by real heat loss.
- When the sandbox moves or changes a cell, the dig face updates. Lava that flows into a dug space becomes part of the face; rock that freezes there becomes diggable; a cell that empties opens up.
- On the test planetoid, breached magma chambers now drain and crust over instead of standing as towers.
- The sandbox adds about 20% to run time on the test planetoid (75 minutes of play in 146 s on the Linux test machine). Tutorial pacing is unchanged: asteroid 1 is ready in 62–67 min, asteroid 2 in 32–36.

**New tools and tests:**

- `run-sandboxdemo.bat` runs the demo above and writes frames to `out\sandbox\`.
- Three new sandbox tests: a cave-in keeps every kilogram and balances the books; lava crusts over and water quenches it; the same start gives the same result. All 27 tests pass.

**Not yet:**

- **Explosions, fire and pressure:** the reaction table supports them, but the materials they need (methane, oil, acid, brine, carbon) arrive with the archetypes that carry them.
- **Rock melting into magma under extreme heat:** needs a melt of matching density for each rock.
- **Drones are not yet harmed by heat or lava;** that needs drone positions, which come with M4b.
- **Large-scale flooding and gas build-up** belong to M5's basin and reservoir tiers.

## M4b results: loose bodies and hazards

Rock that digging cuts free now falls, tumbles and lands. Drones can be wrecked by lava and falling rock, and wrecked drones leave scrap.

&#91;image: Sandbox demo with a block of rock cut free by a moat, falling toward the centre\]

*The updated sandbox demo. Left: a 10 m block cut free by a moat drops toward the centre and settles. Right: the rubble cave-in and the water-meets-lava corridor from M4a.*

**How it works:**

- **Breaking free:** after digging, the solid rock next to each newly opened cell is checked. A connected piece of 8 to 120 cells touching nothing else has been cut free and is lifted out of the grid as a rigid body of 1 m voxels. Smaller specks cling where they are; anything bigger stays put as an overhang.
- **Falling:** bodies run in the Rapier physics library, in its cross-platform deterministic mode. They fall toward the centre under a game gravity of 3 m/s² (a real asteroid barely pulls at all), and tumble and stack against ground colliders built from the grid near them.
- **Landing:** a body that has been still for a second sets down into the grid, each cell in the nearest open cell. It is part of the dig face again.
- **Bookkeeping:** while a body is in the air its mass is booked as aloft, so the books balance at every tick.
- **Hazards:**
  - A drone arriving at a cell beside lava, or beside rock hotter than 1000 K, is wrecked.
  - A drone working beside rock falling faster than 6 m/s (a fall of about 6 m) is crushed.
  - Glowing heat and open lava are plain to see, so every drone, blind or not, steers clear of them when choosing a cell. Drones are only caught when lava flows in or rock falls during the trip.
- **Wrecks:** a wrecked drone leaves one cell of drone scrap where it fell. Scrap is a loose powder, so it piles up in the sandbox and can be mined again; it is booked as matter deposited from outside the body. Necromancers can later raise these wrecks.
- **Drone positions:** each drone on a trip knows where it set off from and the cell it is working. That is enough to place hazards and wrecks. A full flight path comes with the renderer (M6).

**Effect on play:**

|  | Before M4b | With M4b |
| --- | --- | --- |
| Tutorial asteroid 1 ready | 62–67 min | 66.5 min (seed 1) |
| Tutorial asteroid 2 ready | 32–36 min | 33.5 min (seed 1) |
| Asteroid 1 peak drones | about 8,200 | about 6,800 (falling rock wrecks some) |
| Test planetoid, 75 min: drones wrecked | none | 17,100 (12,500 by lava, 4,600 by falling rock) |

Tutorial pacing holds. On the test planetoid, breached magma chambers are now the big killer: lava pours over the dig face while thousands of drones work it. That is the "hapless drones die" the design asked for, though the toll is high and needs tuning once the game is visible.

**Cost:** the cave-riddled crust of the test planetoid breaks off about 30,000 chunks in 75 minutes. The run takes 425 s, against 146 s before M4b. A tutorial asteroid run takes about four times as long as before. Most of the extra time is the swarm reacting to the moving ground. Profiling and speeding this up is a task for M6, when frame budgets are measured on the real renderer.

**Tests:** three new ones:

- a block cut free falls and lands, with every kilogram on the books at every tick;
- the same fall happens every run;
- a wrecked drone leaves scrap on the books.

All 30 tests pass on the Linux test machine; the full suite now takes about 7 minutes. Rapier's deterministic mode is meant to give the same result on every machine; running the tests on your Windows PC will confirm it.

**Not yet:**

- Impacts don't shatter rock into rubble yet. That needs a powder form of each rock, which comes with the procedural chemistry (C1).
- Large drones (Tunnelers) aren't affected by hazards.
- Saves set every body down first.

## Speed pass

After M4b the sim was 3–4× slower than before it, which slowed every check. Profiling on the Linux test machine (a sampling profiler on short benchmark runs) found the time, and four changes recovered it.

| Benchmark (Linux test machine) | Before | After |
| --- | --- | --- |
| Tutorial asteroid 1, 40 min of play | 37 s | 11 s |
| Test planetoid, 30 min of play, perks and Tunnelers | 79 s | 25 s |
| Swarm tests (two full tutorial runs) | 406 s | 99 s |

**What changed:**

- **World lookups:** the world's node table used the standard library's attack-resistant hash, at several times the cost of a plain one. Lookups that land in a leaf already opened by digging now answer in one step instead of walking down from the root.
- **Choosing cells:**
  - Drones weigh about 48 nearby cells instead of 96.
  - The search no longer scans an extra ring to keep its reach round; it keeps only cells inside the circle the scanned rings fully cover.
  - The check for glowing heat and lava runs only on the cell that would otherwise win, instead of on every candidate.
- **A bug fix:** cells that left the dig face and came back (rubble that fell away and returned) were listed twice in the search, so drones weighed them twice. Fixing this changed results slightly.
- **Falling rock:** the check for rock cut free stops as soon as it reaches ground nothing has touched yet. The ground the rock collides with is rebuilt at most every 5 s after nearby digging.

**Retune:** with these changes tutorial asteroid 1 drifted to 66–72 min. Rock now cuts 10% faster (13.0 s per load of hardness-3 rock, down from 14.4). Asteroid 1 is ready in 61–65 min and asteroid 2 in 31–34 min across three seeds.

**Cross-platform check:** after `powi` (Rust's built-in for raising a number to a whole-number power, whose rounding can differ between platforms) was replaced with our own fixed-order version, all six determinism fingerprints match exactly between your Windows PC and the Linux test machine. Every run is now bit-for-bit identical on both.

## C1 results: elements and generic phase changes

The first step of procedural chemistry is in.

**Element table** (`data/elements.toml`, 42 elements). Each element has:

- atomic mass and radius;
- crystal structure;
- electronegativity and valence;
- melting and boiling points;
- its usual oxide and that oxide's formation energy;
- a value per kg, and its class (metal, metalloid, nonmetal or noble gas).

Every material's composition is checked against the table when it loads. Later steps read these properties: alloys use radius, structure and electronegativity, and burning uses oxide energy.

**Phase changes are generic.** A material only states the temperatures at which it melts and boils. Unless it names a specific result, the game generates a molten form and a vapour for it at load time, and the molten form freezes back into what it came from:

- molten basalt, molten gold, sulfur vapour and methane vapour all exist without being written;
- 13 molten forms and 16 vapours are generated in all, for 54 materials;
- liquid methane boiling at 112 K uses exactly the same rule as water at 373 K.

**Latent heat:** at a melting or boiling point the temperature holds while heat goes into (or comes out of) the change, so melting, freezing and boiling take time. Lava now crusts over more slowly, and water touching lava takes a while to boil.

**Pressure holds deep rock solid.** Deep in the test planetoid the rock is hotter than its own melting point, so without this rule digging would have woken the mantle into a sea of melt. Where a body is hotter than a material's melting or boiling point, the weight above holds that point at 100 K above the local heat; only heat beyond that melts or boils it.

**Solids join the heat simulation only when it matters:** when a neighbouring liquid is hot enough to melt them (magma against sulfur). Otherwise rock stays a fixed heat bath and costs nothing.

**The three hand-written reactions are deleted.** Water meeting lava boils, and the lava crusts over, purely from heat and phase change. The reaction table remains for overrides only.

**Lava danger:** a drone is in danger only beside a liquid at 1000 K or more (lava, molten metal), no longer beside hot rock, and only one in five caught is wrecked; the rest get away.

**Also:** methane ice and liquid methane are added, ready for Titan-like bodies.

**Tests:** four new chemistry tests:

- every material's elements are known;
- every meltable solid gets a molten form of the same mass that freezes back;
- sulfur held at a heat source melts and then boils by its own numbers, with latent heat delaying each step;
- liquid methane boils into a generated vapour that condenses back.

All 34 tests pass; tutorial pacing is unchanged (65.4 and 31.9 min).

**Not in C1 (moved to C3):** variable mass per cell. Phase changes and C2's mixing keep a cell's mass, so they don't need it. Burning and redox, which make products of different weights, do; it will come with them.

## Groundwork results

Three changes every new planet type needs. All 35 tests pass with **every fingerprint unchanged**, so none of them altered how existing bodies play.

- **Two-byte material ids:** room for 65,535 materials instead of 255. Saves move to version 5.
- **Per-body gravity and sky temperature:** a body spec can set `gravity_m_s2` and `space_k`. Without them it uses the physics table's values, as before.
- **Depth pressure:** each body works out real gravity and pressure from the centre to the surface, treating its layers as a sphere. A new test checks this against the exact answer for a uniform sphere, and it matches to within 0.1%.

**What the numbers say:** real pressure inside small bodies is tiny. The test planetoid's centre sits at 4 kPa and the 30 km active planetoid's at 2 MPa, while diamond needs about 5 GPa. Pressure-driven materials (diamond, pressure ice, metallic hydrogen) will only appear in large worlds of a few thousand km. That suits the carbon, ocean and stripped-core types, which are meant to be big. Real surface gravity on these bodies is 0.001–0.03 m/s², so the game keeps its own gravity value (3 m/s²) for feel.

The viewer now has a Pressure colour mode, shows pressure under the cursor, and lists each body's real gravity and centre pressure.

## Material catalogue

89 new materials for the 16 v1 types, written by `tools/catalogue.py` into `data/materials-catalogue.toml`. Each composition is worked out from real mineral formulas (granite as quartz, feldspars and mica; ruby as aluminium oxide with a trace of chromium), so element fractions always sum to exactly 1 and value follows the chemistry. With generated molten and vapour forms the game now has **276 materials**, past the old 255 limit.

| Family | Count | Materials |
| --- | --- | --- |
| Igneous | 7 | granite, andesite, gabbro, obsidian, pumice, tuff, volcanic ash |
| Deep rock | 2 | perovskite rock (lower mantle), eclogite |
| Sediment | 14 | sand, gravel, clay, soil, sandstone, shale, limestone, conglomerate, coal, rock salt, gypsum, banded iron, crude oil, tar |
| Metamorphic | 5 | marble, quartzite, slate, schist, gneiss |
| Ores | 22 | magnetite, hematite, chalcopyrite, native copper, galena, sphalerite, cassiterite, cinnabar, chromite, ilmenite, bauxite, uraninite, native silver, molybdenite, wolframite, stibnite, arsenopyrite, cobaltite, spodumene, troilite, pallasite, platinum nugget |
| Gems | 9 | quartz crystal, amethyst, corundum, ruby, beryl, emerald, garnet, opal, olivine |
| Carbon | 2 | graphite, silicon carbide |
| Ices and cold liquids | 19 | nitrogen, oxygen, frozen air, ammonia, ethane (each with its liquid), brine and sea ice, dirty ice and muddy water, pressure ice, hot ice, dry ice, SO₂ frost, CO ice |
| Gases | 5 | carbon dioxide, sulfur dioxide, carbon monoxide, hydrogen, helium |
| Surfaces | 2 | comet dust, tholin |
| Exotic | 2 | metallic hydrogen, resonant crystal |

**Sublimation for free:** dry ice, SO₂ frost and CO ice are written as solids that turn straight into gas, so comets and Io-like bodies outgas without new code. The ices that matter for lakes and oceans keep a liquid phase.

**Effect on play: none.** All 35 tests pass. Four fingerprints changed only because the auto-generated molten and vapour forms moved to higher id numbers. Drone counts and consumption at every 10-minute mark of both tutorials match the previous run exactly (65.4 and 31.9 minutes).

## Geology pass: research and design

### Why today's worlds look like spaghetti

The generator draws veins and caves as endless noise contours anywhere inside a depth band. Nothing ties a vein to a cause (a magma body, a fault, a rock layer), blobs land at random, layers are near-perfect circles, and every body uses the same few shapes at the same scales. The result is evenly spread squiggles with no story.

### What other generators do

| Source | Technique | What we take |
| --- | --- | --- |
| [Dwarf Fortress](https://dwarffortresswiki.org/index.php/DF2014:Rock) | Stone comes in layer classes (soil, sedimentary, igneous intrusive and extrusive, metamorphic). Every mineral lists the host rocks it may appear in and its form: vein, large cluster, small cluster or single gem. Some inclusions leak across layers as deliberate exceptions | A data table of **mineral rules**: host rock, setting and form for every ore and gem |
| [Sculpting Mountains](https://www.cs.purdue.edu/homes/bbenes/papers/Cordonnier18TVCG.pdf) (Cordonnier et al. 2018) | The crust is an ordered stack of layers, each with a thickness and stiffness. Folds and faults are displacements of the stack; to find the rock at a point, map it back to its depth before deformation. Fold wavelength grows with layer thickness and stiffness contrast | **Undeformed depth:** every cell is mapped back through the body's history to the depth it formed at, which picks its rock. Works cell by cell, on demand |
| [Noita](https://noita.wiki.gg/wiki/World_generation) | Edge-matched tiles keyed to material palettes, with hand-made set pieces dropped on top | Later, for ruins and megastructures |
| Minecraft 1.18 ore veins | One noise field picks the ore, a ridged field shapes it into ribbons with gaps, and filler stone surrounds it | Mostly what we do now. Useful only inside a vein body, not for placing veins |
| Real ore geology | Ore forms where a process concentrates it: hydrothermal veins around cooling magma, zoned by distance (copper and molybdenum close, then lead, zinc and silver, then gold, antimony and mercury far out); chromite and platinum layers at the base of cooling magma; skarns where magma meets limestone; banded iron and coal in sediment; gold placers in old riverbeds; diamonds only in pipes from great depth | The **processes** that place each ore, so every deposit has a reason |

### The design: a short history for every body

When a body is created, it gets a **formation history**: an ordered list of geological events worked out once from the seed at a coarse scale. Each event is a bounded object with real geometry (a pluton, a dike, a fault line, a crater), never an endless noise field. To find a cell's material, the generator walks the history backwards, as in the mountains paper. Undo the faults and folds to find the depth the cell formed at, read its layer, then check the bounded events that touch it (spatially bucketed, as blobs are now). Huge bodies still generate cell by cell on demand.

Events, in the order they happen:

1. **Differentiation:** core, mantle and crust, as now.
2. **Strata:** the crust built up as bands that follow the surface rather than the centre, each one rock from the type's sequence (lava flows and ash on a volcanic world; sandstone, shale, limestone and coal on a rocky world; ice and dust on a comet).
3. **Deformation:** folds (wavelength from layer thickness) and faults (lines that shift one side), so bands bend, break and pinch out.
4. **Intrusions:** plutons (large deep granite or gabbro bodies), dikes (thin straight sheets rising from them), sills (sheets along the strata), pipes (kimberlite from great depth).
5. **Ore from the intrusions:** short, finite vein swarms along fractures above each pluton, zoned by distance like real deposits; skarns where a pluton touches limestone; chromite and platinum layers at a pluton's base.
6. **Metamorphism:** rock baked near intrusions (limestone to marble, shale to slate, sandstone to quartzite), and deep burial turning shale to schist to gneiss.
7. **Sedimentary ore:** banded iron in old sea-floor bands, coal seams in shale sequences, salt and gypsum lenses, gold placers at the base of sand and gravel channels.
8. **Voids:** caves only where they form: dissolved in limestone, lava tubes in basalt, open cracks along faults. On rubble asteroids, gaps between packed boulders of different rock.
9. **Impacts:** craters that cut everything earlier, leaving shattered rock and impact melt.
10. **Surface:** soil, sand, regolith, frost.

The **mineral rules** table (Dwarf Fortress style) lives in data: each ore and gem lists the events and host rocks that can make it, its form and its rarity. A planet type is a recipe: which events happen, how often, and with what rocks.

### Build steps

- **G1, strata and deformation:** crust bands that follow the surface, folds and faults. Bodies stop looking like onions.
- **G2, intrusions and their ore:** plutons, dikes, pipes, zoned vein swarms, skarns, contact metamorphism.
- **G3, the rest:** sedimentary ores, cave types, rubble-pile boulders, impacts with breccia.
- **G4, retune:** move the five existing bodies to the new system and check that tutorial pacing (60 and 30 minutes) still holds. Old spec features stay available until then, so nothing breaks midway.

Each step is checked in the viewer, old and new side by side.

**Loading-screen budget (your call):** bodies are generated behind a loading screen, so a few seconds of precomputing at load is fine. Geology aims for believable rather than exact: creative licence within geological rules.

**Poles and magnetic fields (Round 11):** the cross-section is a slice through both poles, at the top and bottom of the disc. Poles affect generation: surface rock is colder toward them (`polar_cooling_k`, fading out by `polar_depth_m`), which G3 uses for ice caps and frost. A magnetic field follows from a molten metal core and becomes a derived flag. Generation uses it only to decide whether a body keeps its atmosphere and surface volatiles; auroras, sea-floor magnetic striping and any gameplay effects are a layer added later.

## G1 results

Strata, folds and faults are in (`src/geology.rs`), shown on a new **Strata demo** body in the viewer. Existing bodies don't use them yet, so their fingerprints are unchanged; all 36 tests pass.

- **Strata** follow the surface (depth below ground, not distance from the centre). The stack is a weighted, seeded sequence of rocks that never repeats a rock twice in a row: 34 bands, 350 m deep on the demo.
- **Load-time table:** band bottoms are worked out for about one direction every 4 m around the body (3,141 on the demo, at most 65,536), and each cell blends its two neighbouring directions. The demo builds in **5 ms**; an Earth-size body with a deep stack would take under a second.
- **Pinch-out:** each band thickens and thins along the surface, and above about 0.6 pinch it shrinks to nothing, leaving lenses.
- **Folds** lift and sink the stack, and older, deeper bands fold more than young ones. Where a fold rises, the surface cuts across it and exposes deeper rock, as erosion does.
- **Faults** are straight, slightly leaning breaks from the surface down. One side shifts by the throw, fading toward the fault's deep tip, and an optional band of crushed rock (gouge) marks the break.
- **Roughness:** about a metre of jitter on every boundary, so close up nothing looks ruled.
- **Poles:** the demo is 60 K colder at the poles near the surface.

New test: almost all near-surface cells (2,595 of 2,600) belong to the strata and none appear deep in the mantle. Faults move rock, and the body loads well under the 2-second budget.

## G2 results

Plutons and the ore around them are in. The rules for which ore forms where live in a data file, `data/geology.toml`, so they can be tuned without code. The demo body (now **Geology demo** in the viewer) has three granite plutons and one gabbro body over a gneiss basement. Existing bodies are unchanged; all 37 tests pass.

- **Plutons:** lumpy, flat-topped bodies of granite (felsic) or gabbro (mafic), placed at a set depth below the surface so they push up into the strata.
- **Dikes:** nearly straight sheets (andesite on the demo) from each pluton toward the surface, some reaching it and some stopping short.
- **Vein swarms:** finite veins fanning out of each pluton's upper side. They are mostly quartz, with ore in patches. The ore changes along each vein by distance from the magma, following the zoning in the rules file.
- **Baked rock:** within the aureole, rock turns into its metamorphic form (limestone to marble, shale to slate, sandstone and conglomerate to quartzite, coal to graphite, tuff to schist).
- **Skarns:** right against limestone, patches of magnetite, garnet and chalcopyrite.
- **Layered bases:** mafic bodies get chromite stripes at the bottom, with platinum nuggets in them.
- **Load time:** 15 ms for the demo. Plutons and veins are looked up through occupied buckets only, so huge bodies cost nothing extra.

**Zoning measured on the demo** (mean distance of each ore from the nearest granite):

| Ore | Cells | Distance |
| --- | --- | --- |
| molybdenite | 162 | 57 m |
| galena | 206 | 138 m |
| gold-quartz | 60 | 179 m |
| stibnite | 5 | 210 m |
| cinnabar | 0 | none in this sample |

The far-zone minerals are rare, as they are in real deposits; cinnabar may need more weight when bodies are retuned in G4.

## G3 results

The rest of the geology toolkit is in. It's shown on the **Geology demo** and a new **Rubble demo** in the viewer. All 38 tests pass, and both tutorials play identically (65.4 and 31.9 minutes).

- **Rubble piles:** boulders at three nested sizes. Big ones (up to \~100 m) fill their space whole, medium ones fill around them, and small rubble and dust fill the rest, with dust in the gaps and some empty voids. Each boulder is one rock picked by weight, so metal and pallasite boulders appear as whole chunks. A first try at a single size looked like uniform tiles and was dropped.
- **Caves only where they form:** each cave kind lists its host rock. Karst passages dissolve through limestone and marble, and lava tubes run inside basalt flows. Both run along the layers, and both have quiet regions with no caves at all.
- **Lenses inside layers:** a deposit at the base, middle or top of a host band, thickest at its centre, at a set average spacing along the band. On the demo: river gravels at the base of conglomerate with gold nuggets in them, salt lenses inside shale, and gypsum at the top of limestone. Banded iron is now a strata band type.
- **Impacts:** craters get a lens of shattered rock (a new **impact breccia** material) under the bowl, and a thin sheet of melted glass on the floor.
- **Surface:** polar ice caps that thicken toward each pole, and a choice of surface blanket per body (soil on the demo).
- **Load time:** both demos together load in 30 ms.

Adding impact breccia moved the auto-generated molten and vapour materials up one id, so the sandbox, bodies, world and swarm fingerprints changed. Play is unchanged.

**Next: G4.** Move the five existing bodies onto the new geology, starting with the tutorial asteroids as rubble piles, then re-check tutorial pacing.

## G4 results

All five existing bodies now use the new geology, and no body uses the old noise veins or noise caves any more. All 38 tests pass. A new tool, `bodystats`, reports a body's mass, empty space, dig work per kg and value multiplier, so each conversion was matched to the old body before the slow tutorial runs.

**Tutorial and rubble asteroids** are rubble piles: chondrite and basalt boulders, with troilite, metal, pallasite and ice kept to medium and small sizes so composition stays steady from seed to seed. The matrix between boulders is cemented crushed rock (impact breccia), and the empty space comes from missing boulders. Craters carry breccia and melt.

| Tutorial asteroid 1 | Old | New |
| --- | --- | --- |
| Empty space | 22.2% | 19.6% |
| Dig work per kg | 15.55 | 14.94 |
| Value multiplier | ×1.026 | ×1.027 |
| Tutorial 1 ready | 65.4 min | **67.2 min** |
| Tutorial 2 ready | 31.9 min | **31.2 min** |
| Both tutorials, sim time | 177 s (open-crack version) | **80 s** |

**What went wrong first:** with open cracks between all the boulders, small boulders were free-floating from the start. 3,176 broke loose in the first 10 minutes and 48,600 by 60 minutes (the old body had 8,700), so tutorial 1 slowed to 72.6 minutes and the sim to half speed. Filling the cracks with cemented breccia fixed both. It is also what real rubble-pile meteorites look like.

**Planetoids:** the 1.5 km test planetoid and the 30 km active planetoid have a folded stack of lava flows, ash and glass over the basalt crust, faults, and lava tubes. Small granite stocks in the crust carry zoned copper, lead-zinc and gold veins, and gabbro bodies in the mantle hold chromite and platinum reefs (sperrylite was added to the base-of-pluton rules, as in real platinum reefs). Magma chambers, gas and sulfur pockets, and kimberlite pipes stay as they were. The active planetoid loads in 78 ms.

**A trade-off for you:** honest geology concentrates ore. The old planetoid was about 8% ore, laced everywhere with veins; the new one is about 1.5%, in deposits around intrusions. Value per kg dropped from ×1.12 to ×1.06 on the test planetoid, and from ×1.09 to ×1.07 on the active planetoid. Ore richness is a set of numbers in `data/geology.toml` and each body's intrusion counts, so it can go back up if play wants it. Concentrated deposits also give swarm intelligence something worth choosing between.

## Run mode

The [Planet Viewer](https://claude.ai/artifact/DGDJXTtTUNXxRk2LrrJsFx) can now play the full game simulation (swarm, sandbox and falling rock) on any body, with the idle player making the purchases.

- **The same code:** the whole sim is compiled to WebAssembly (2.9 MB) and needs nothing from outside. The parallel code falls back to one thread in the browser on its own. Run headless, tutorial 1 launched at 67.2 minutes with fingerprint `3f0530bbbf623871`, **bit-for-bit the native Linux result**, so the browser is a third platform that agrees.
- **Speed:** about 90× real time in Node.js and 25–85× in a browser page while drawing. The whole of tutorial 1 plays out in under 2 minutes at Max.
- **A background worker:** the sim runs in a worker with its own copy of the module, in 30 ms slices, so the page never freezes. About four times a second it sends a frame of the current view (reduced to 400k pixels when the window is large), drone positions and the live numbers. Panning and zooming move the last frame at once while the next one is made.
- **What you see:** drones as dots (cyan flying out or cutting, amber carrying a load home, grey idle), landing points and Refiners as orange diamonds, ground the swarm has eaten as open sky, and live temperatures in Heat mode. The readout shows game time, actual sim speed, drones, % consumed, purchases, crushed and burned drones, awake sandbox cells and loose rock. At launch it shows the time, Seed Mass and run fingerprint.
- **Detail by zoom, not physics by zoom:** the simulation never changes with the camera (that would break fingerprints). Only drawing does: the frame resolution adapts to window size. Cached 64× 64-block images, redrawn only when a block changes, are the next step and become the M6 renderer's approach.

Drone positions are worked out from each flight leg's start time and speed, through a read-only query that doesn't touch the simulation's state; all 38 tests keep their fingerprints.

## Variety review (after G4)

Five seeds each of the test planetoid, the active planetoid and the geology demo were rendered side by side, with a close-up of each surface (new `contactsheet` tool).

**What works:** underground, close up, the geology now reads as real: layered and folded rock, faults that step the layers, granite stocks with fans of ore veins, baked contact zones, karst caves, lenses.

**What doesn't:**

- **Flat surfaces.** Every close-up shows a smooth horizon. The outline is a separate noise wobble plus crater bowls, and nothing in the geology shapes it: no volcanoes, no fault cliffs, no ridges, no valleys.
- **Seeds are siblings.** Every seed of a body has the same counts and sizes of everything, so seeds differ only in where things sit. Overviews of one type are hard to tell apart.
- **Onion and polka dots at planet scale.** The mantle fills about 60% of the disc and is flat green, with magma chambers and gabbro bodies sprinkled evenly as round dots. Layers still read as concentric rings when zoomed out.
- **Magma isn't plumbed.** Chambers are isolated blobs; nothing connects them to dikes or to the surface.
- **The active planetoid's crust** is mostly basalt bands riddled with lava tubes, which look like Swiss cheese at that scale.

## G5: surface and history (proposed)

Planets are the game's maps and its main visual draw, so this comes before new planet types.

1. **Topography from geology:** the surface height comes from the body's events. Volcanoes build cones out of their own lava-flow layers over a vent; calderas collapse; faults leave cliffs; folds raise ridges; rifts drop valleys; impacts throw up rims and ejecta blankets; hard layers stand out as terraces where softer ones wear back. The surface becomes a load-time table around the body, which is also faster than today's noise lookup.
2. **A character for every seed:** each body type defines ranges and a few headline events, and each seed draws its own: one planetoid gets a giant shield volcano, another a great rift, another a huge impact basin or a mountain belt. Counts and sizes are drawn from wide ranges, not fixed.
3. **Hemispheres:** an old, cratered, uplifted half against a young, smooth lava-plain half (as on Mars and the Moon), set per seed.
4. **Plumbing:** magma chambers fed from below and feeding dikes up to surface vents, with sills along layers and old frozen chambers turned to gabbro.
5. **A living mantle:** rising plumes, faint convection streaks, the occasional deep slab, and pockets of partial melt, so the biggest area of the disc has structure.
6. **Eras:** an older folded stack eroded flat, then younger flat layers laid over it (an unconformity), so depth tells a sequence of events.

Each step is checked on contact sheets of several seeds, with a target that a type's seeds should be told apart at a glance.

## Deep interior (before G5)

On large bodies the crust is a thin rind and about 80% of the disc is mantle and core, so the deep interior needs as much character as the crust. It's built from real deep-Earth structure seismology can see, scaled to each body's size for visual interest (`src/deep.rs`, a `[deep]` section in each body spec). It's on the test planetoid, the active planetoid and the geology demo. All 39 tests pass, and the tutorials are unchanged (67.2 and 31.2 minutes).

- **Depth zones:** a lower mantle of perovskite, a **ringwoodite transition zone** (blue, and it really holds water), and an upper mantle of peridotite, each boundary with its own relief.
- **Marble-cake streaks:** old crust stirred in by convection, as loops of harzburgite, pyroxenite and eclogite around each convection cell. Their shape is the closed streamlines of a convection cell, bent by a slow warp. A first pass was bold perfect ovals, like fingerprints; the final version is thinner and irregular.
- **Plumes:** columns rising from the core with mushroom heads under the crust, partly molten. They run about 230 K hotter than the rock beside them, which shows in Heat mode.
- **Slabs:** cold sheets of eclogite curving down from the surface, some stalling flat on the transition zone, with rare super-deep diamonds below it. They run about 120 K colder.
- **Deep piles:** dense crimson domes resting on the core, with thin melt pockets at their edges (as under Africa and the Pacific).
- **Inner core:** iron crystals radiating from the centre in growth shells (the count around halves as the radius halves, so they never pinch to a point), with an innermost core of small even grains. **Platinum-iron alloy** gathers on the grain boundaries, densest toward the Heart, so the deepest dig pays most (most of Earth's gold and platinum really is in its core).
- **Outer core:** convection columns of a pale sulfur-rich melt lined up with the spin axis (vertical in our pole-to-pole slice), and a light layer along the top.

**New materials** (catalogue now 99): harzburgite, pyroxenite, plume rock, ringwoodite rock, deep pile rock, iron crystal, platinum-iron alloy, sulfur-rich core melt.

**Cost:** the 30 km active planetoid still loads in 79 ms. Deep materials carry value, so value per kg is back up to about ×1.13 on the planetoids, concentrated deep.

**Mistakes caught on the contact sheets:** the first inner-core crystals came out as a spiral, because grain spacing was measured along the arc at each radius. Then, with a fixed count around, all the crystals pinched to one point at the centre. Growth shells and an innermost core fixed both.

## Earth-size scale test

A 6,371 km rocky world (`data/bodies/earth-test.toml`) using every generator feature at realistic sizes: a 5 km strata stack, 400 faults, 600 granite plutons with nearly 20,000 dikes and veins, 300 magma chambers, 40 craters, and the full deep interior. A new tool, `scaletest`, measures it.

**Generator: fixed, and fast.** The first run couldn't load at all: the old blob lookup grid tried to allocate 15 GB. With that fixed it loaded but was slow and heavy, so the generator was tuned. Every change gives bit-identical worlds (same checksums), and all 39 tests keep their fingerprints.

| Earth-size body | First run | After fixes |
| --- | --- | --- |
| Load | 2.1 s | **0.13 s** |
| Memory | +436 MB | **+35 MB** |
| Per cell: crust / mantle / core | 3.8 / 3.7–4.1 / 3.1 µs | **0.8 / 0.5 / 0.36 µs** |
| Whole-planet render, 1000 px | 1.3 s | **0.2 s** native, **1.3 s** in the browser viewer |
| World store set-up | 0.9 s | 0.9 s |

Fixes: lookups use occupied squares only, sized to the body; layer-boundary wobble, crater, strata and fault checks are skipped where they can't apply; faults are binned into wedges around the body (a cell checks 2–3 of 400); slab segments have bounding boxes. Small bodies got faster too: the generator tests run in 3 s instead of 9.

**Swarm: doesn't scale yet.** At start the swarm maps the whole surface as its dig face, tracing every surface cell around the body. On the 30 km planetoid that takes 10.5 s and 160 MB; scaled to Earth it would be roughly 40 minutes and 30+ GB. Tracing is now faster where possible (skipping empty steps above the ground gives identical results), but the approach itself has to change. The viewer's Run mode is disabled above 50 km until then.

**Rock physics precision:** the falling-rock engine (Rapier) uses 32-bit floats, which resolve about 2 mm at 30 km from the centre but only about **0.5 m at Earth's radius**. That's too coarse for 1 m cells.

### Proposed fixes

1. **Landing zones.** The swarm lands at one or more points and maps only the surface around them, growing its dig face as it spreads. On small bodies a single zone grows to cover everything, so play doesn't change. On big bodies, landing zones become a purchase and a strategic choice, which fits the 10k-drone scale: 10,000 drones can't touch more than a sliver of an Earth-size surface anyway. This is a swarm change that should come before any body over about 50 km is played.
2. **Physics near the action.** Either switch Rapier to its 64-bit build, or run falling rock in a local frame centred on the area being dug. The local frame is cheaper and keeps 32-bit speed.

The Earth-size body is in the viewer for looking around.

**Order (your call, Round 11):** G5 first, then the swarm pass (landing zones and local-frame physics). G5 only touches the generator, which now scales to Earth size, and landing zones are better built on the final terrain.

## G5 step 1: surface shape

The surface is now a load-time table around the body (`src/relief.rs`), built from **relief families** in each body spec (`[[relief]]`). Each family has a chance, a count range and size ranges, so **each seed draws its own set**. All 40 tests pass, the tutorials are unchanged (67.2 and 31.2 minutes), and the active planetoid loads in 112 ms.

| Feature | Shape | Built from |
| --- | --- | --- |
| Shield volcano | broad, gentle, with a flat-floored summit caldera | draped lava flows (basalt, tuff, glass) |
| Steep cone | concave, sharp | draped andesite and tuff |
| Rift | flat floor between steep walls, raised shoulders | sediment fill on the floor |
| Impact basin | broad bowl with a raised ring | lava flooding the floor |
| Lowland | one half of the body sunk smoothly | a lava-plain cover |
| Mountain belt | broad fold with peaks and valleys carved out of it | the folded layers themselves |
| Mesa | flat top, eroded cliffs | the layers, cut by the cliffs |
| Fault cliffs | steps where faults break the surface (`scarp` share of each throw) | the shifted blocks |
| Crater rims | raised rims with ejecta blankets | shattered rock |

**One rule makes the layers come out right:** every feature records both where the ground is and where the layers' reference surface is. Uplift moves both; piles (volcanoes, blankets, fills) sit on top without moving the layers; erosion and excavation lower the ground without moving the layers, so valleys, basins and cliffs cut through them. Fault cliffs keep the strata matched across the break: the part of the throw still standing as a cliff moved with the surface, and only the worn-away part shifts the layers.

**Fixed along the way:** mountains and mesas first raised the layers right up into every peak and edge, squeezing them into fans. Real peaks and mesa cliffs are carved, so both are now a broad uplift with the shape cut out of it.

**Measured variety** (five seeds of the test planetoid): 4 to 10 features each, drawn differently every time (three volcanoes and a rift; six volcanoes, a lowland, a rift and a basin; and so on), with relief from −95/+91 m up to −214/+186 m on a 1.5 km body. A new test checks that every seed has real relief and that no two seeds share a surface.

**Next G5 steps:** magma plumbing (chambers feeding volcanoes through dikes, plumes feeding hotspots), eras (an old folded stack worn flat under younger flat layers), and a pass on the tutorial asteroids' surfaces.

### G5 step 2 — magma plumbing (done)

Every volcano now has a source underneath it, and the surface is tied to the deep interior.

- **Chamber:** a flattened, lumpy chamber 0.7 widths below the ground the volcano stands on.
- **Conduit:** a wandering conduit up to the vent.
- **Dikes:** 2–4 radial dikes fanning to the flanks. Some reach the surface as fissures.
- **Sills:** 1–2 sills spreading sideways between the layers.
- **Active volcanoes** (`active_chance`) hold real magma, with a heat halo that keeps it above 1,450 K. There's a lava lake at the vent, and some dikes are still molten.
- **Extinct volcanoes:** the chamber freezes to gabbro (shields) or granite (steep cones), or to `chamber` if set. The conduit becomes a plug of the cone's rock (a volcanic neck), and dikes and sills are dolerite. Dolerite is a new catalogue material, number 100.
- **New relief kinds tied to the deep anchors** (`deep::anchors`, same seeded draws as the deep build):
  - `hotspot`: a shield over each plume (by chance), with a feed pipe from the plume head up to its chamber.
  - `trench`: a structural bend down where each slab sinks.
  - `arc`: a chain of steep cones set `offset_m` back from the trench, on the side the slab dips.
- **Fix:** a volcano built on an earlier pile now stacks on it (two-level pile: `top` and `under`). Before, a cone on a shield replaced the shield's lava and left a flat-sided block of strata inside the mountain. Volcanoes now apply after all other relief.
- **Speed:** plumbing lookups use a 256² grid of (volcano, part) lists over the plumbing's bounds, so the output is exactly the same. Crust cells on the 30 km planetoid cost about 10% more.
- **Checks:** new `volcanoes` cut-away tool (material + heat); new test `g5_volcanoes_have_plumbing`. Tutorials unchanged at 67.2 / 31.2 min. The Viewer is updated (v11).

Next: G5 step 3, eras (an old folded stack worn flat under younger layers), then a surface pass on the tutorial asteroids, then the swarm optimization pass.

### G5 step 3 — eras (done)

A body's strata can now sit on an older era (`[strata.old]`): its own band stack, folded hard, worn flat, and buried.

- **The worn surface (unconformity):** lies `cover_m` below the young layers. Its depth varies over `cover_wavelength_m` (old hills and valleys), and it bends with the young layers' folds by as much as a young band at that depth would.
- **The old stack:** `erode_m` of its top was worn away, so the cut slices across its folds. It ends at a smooth base `depth_m` below its original top, and its deepest band fills any fold gaps, so no basement spikes reach upward.
- **Gravel:** an optional conglomerate bed (`conglomerate_m`) lies on the worn surface.
- **Bodies:**
  - Geology demo: folded quartzite, slate, marble and schist under the young sediments. A rock-salt lens in limestone was added, since the thinner young stack left little shale.
  - Active planetoid: a greenstone belt (folded schist, slate, old basalt, banded iron) under young lava.
- **Tooling and tests:** `contactsheet` takes `SPAN=<m>` for wide crust close-ups. New test `g5_an_older_era_lies_under_an_unconformity`; the strata test counts old bands (`Strata::all_bands`). Tutorials unchanged (67.2 / 31.2 min). Viewer updated (v12).

Next: a surface pass on the tutorial asteroids (pacing re-check), then the swarm optimization pass (landing zones; local-frame physics).

### G5 step 4 — tutorial asteroid surfaces (done; G5 complete)

Both tutorial asteroids now have a surface that varies by seed:

- **Crater rims:** the crater rims have blankets of crushed rock (`rim = 0.05`).
- **Old impact scar:** an old giant impact scar on 60% of seeds (a basin with a breccia rim).
- **Seas:** 1–2 “seas” (shallow low spots filled with fine regolith, like Itokawa's) on 60% of seeds.
- **Boulders:** 120–200 boulders (180–300 on asteroid 2) resting on the surface, from 3 to 14 m across in radius, partly sunk in. They come from a new `boulders` relief kind: one rock per boulder, picked by weight (mostly chondrite, some basalt, rare iron-nickel and pallasite). Boulders stay bare rock with no regolith coat, and crater blankets go under them.
- **Pacing:** tutorial 1 is 68.2 min (was 67.2) and tutorial 2 is 31.6 min (was 31.2). Both are within tolerance, so no re-tune. Viewer updated (v13).

Next: the swarm optimization pass. Landing zones replace whole-surface frontier seeding, and physics moves to a local frame (or f64) for large bodies.

### Swarm optimization pass (done)

- **Landing zones:** bodies with a circumference up to `full_survey_m` (25 km) are still surveyed whole at the start, so tutorials and small bodies behave exactly as before. Bigger bodies start with nothing. Each landing point surveys `zone_m` (500 m) either side of it when the orbiting seed swarm drops it, and the dig face grows from there as digging opens ground. Zone surveys stride through deep open ground in 4 m steps, and the new `Planet::ground_hint` skips crater bowls analytically.
  - Swarm set-up on the 30 km planetoid went from 14.3 s to 0.035 s.
  - On the Earth-size body set-up takes 0.3 s, and the first minute of play digs and delivers (new test `a_big_body_opens_one_landing_zone_at_a_time`).
- **Local physics frames:** rigid-body physics is split into 16,384 m frames, each its own Rapier world centred on its region. A body moves in the frame it broke free in, so 32-bit floats stay fine-grained anywhere. Bodies under about 8 km across fit in the central frame (origin 0), so their results are bit-identical to before. New test `a_block_falls_true_on_an_earth_sized_body`: the block keeps its shape and lands in its pit. Without frames it never comes to rest.
- **Viewer:** Run mode now works on every body, the 30 km planetoid and Earth size included (v14).
- **Profiling:** `Sim::prof` keeps time per phase (drone events, sandbox, rock bodies, per-second work), and `scaletest` has a `TRACE` mode.
- **Remaining cost:** on the active planetoid, broken-open magma chambers, falling regolith and loose rock keep 30–45k sandbox cells awake. That runs around 1× real time in the browser. It's chemistry load rather than scale, and it's the next performance target (sandbox sleep rules near magma; ground-collider rebuild throttling).

### Performance pass 1 (done)

Profiled with `perf` (a `profiling` cargo profile: release plus symbols). The 30 km volcanic planetoid is the benchmark.

- **Rock settling in magma:** colliders now include liquids, so rock rests on pools. Before, a block that sank into a magma lake searched rings of cells hundreds deep for room. Rock-physics cost dropped about 30×.
- **Downhill-only motion:** powders and liquids move only strictly toward the centre, and gas only away. On a round body the 8 directions don't follow the ground exactly, so cells used to trade back and forth forever. Busy magma cells went from 6,800 to about 700.
- **Free fall:** grains and drops fall up to 8 cells a tick through open space.
- **Exact speed-ups (fingerprints unchanged):**
  - Already-generated leaves skip the walk from the root (`leaf_for_edit`, `in_empty_region`) and the copy in `materialize`.
  - `swap_cells` updates only the nodes below the two leaves' shared ancestor.
  - Emptiness checks stop at the first material.
  - Empty cells are not woken.
  - Exterior cells are cached.
  - Open and exterior space are stored as 64×64 bit tiles (`CellBits`), and the sandbox's per-tick sets are bit-tile queues.
  - Sorts are unstable on unique keys.
- **Region splits:** sample counts depend on depth below the root (the tutorials' table is unchanged), so splitting 2 km regions on big bodies is cheap.
- **Result on the 30 km planetoid:**
  - Busiest stretch: about 10 ms per tick, from 160+.
  - Ten minutes of play: 65 s natively.
  - In the browser it runs about 6× real time even after breaching a magma chamber.
  - Tutorial 1 plays 10 minutes in 2.6 s; the Earth-size body in 7.9 s.
- **Tutorials:** 68.1 / 31.6 min. The lava-and-water sandbox test now checks that water is brought to the boil, since its lava front crusts over before the water arrives.
- **What's left:** most remaining time is regolith sliding down slopes into the swarm's pits (85% of moves are powder diagonals, about 5,000 a tick). That is real content. Its cost is per move: world-store hash lookups and dig-face bookkeeping. Next candidates: direct indexing of generated leaves in the world store (no hash per lookup) and batched dig-face updates for sandbox moves.

### New planet types: metal asteroid, comet, frozen rogue world (done)

Each type was built, checked across seeds, profiled for 10 minutes of play, tested and added to the viewer (v16).

**Metal asteroid** (`metal-asteroid`, 3 km, like 16 Psyche):

- An iron-nickel core with the Widmanstätten structure. A new generator module (`metal.rs`, `[widmanstatten]`) makes jittered-grid parent crystals, each with three families of plates: kamacite plates edged with taenite, and plessite where plates crowd.
- Troilite and graphite nodules, schreibersite needles, platinum nuggets and gold in the metal.
- A pallasite layer with olivine crystals.
- A rubble shell of mantle rock with metal grit between.
- Iron and rock boulders, a big impact scar, and cast-iron impact melt.
- New materials: kamacite, taenite, plessite, schreibersite, metal grit.
- Cost: 14 ms load, 10 min of play in 4.5 s.

**Comet** (`comet`, 2.5 km, like 67P):

- A heap of icy pebbles inside (rubble of dirty ice, ice, dry ice, ammonia ice and tholin, with dust between).
- Onion layers of ices, loose dust, dry ice and tholin.
- CO ice and trapped CO gas deep inside.
- A dust crust, collapse pits, cliffs and ice boulders.
- Olivine and presolar silicon carbide as the valuables.
- **Sandbox changes:** gas now rises up to 8 cells a tick through open space, and bubbles up through powder and liquid. New test: a breached gas pocket vents about 90% within a minute, mass balanced.
- Cost: 15 ms load, under 1 s per 10 min.
- Inert for now: without sunlight nothing warms its ices. A later **solar heating** layer (warm day side) would make comets outgas.

**Frozen rogue world** (`rogue-world`, 40 km, landing zones):

- Frozen-air strata on top: nitrogen, methane, CO and oxygen ice, tholin, with air-snow regolith.
- A water-ice shell that convects (streaks of dirty ice, sea ice and salt), with refrozen brine veins.
- A liquid brine ocean kept at 262 K, then high-pressure ice, a convecting rock mantle and an iron core with crystal grains.
- A nitrogen-glacier plain, water-ice mountains, impact basins, many craters and ice blocks.
- **Cryovolcanoes:** relief volcanoes now take `melt` and `dike`. Brine chambers are held just above their freezing point (the glow uses each volcano's own melt temperature); extinct ones are sea ice with ammonia-ice dikes.
- Cost: 0.25 s load, about 6 s per 10 min.

**Tutorials:** 68.1 / 31.6 min. All tests pass.

### Sunlight and day/night (done)

A body can have a `[sun]` section: `day_k` (what bare ground in full sun warms to), `day_s` (length of a day) and `start_deg`.

- The Sun turns around the body, using deterministic `noise::cos_sin`.
- Sunlit ground warms toward `day_k · √√(sun height)`; the fourth root is radiative equilibrium. The night side cools to space.
- **Shade:** a ray toward the Sun decides whether an open cell is lit. It counts as lit once clear of the ground beneath it, and answers are cached per 2 s Sun step, so pits and shafts are shaded properly.
- **Heating:** a sunlit open face closes 10% of the gap per tick (physics `sunlight`). Exposed solids under the Sun count as tracking heat.
- **Waking:** every 5 s the swarm wakes dig-face cells under the Sun whose material can thaw or boil below `day_k`.
- **Cost:** none at all for bodies without a Sun. Every existing fingerprint is unchanged, and the tutorials are still 68.1 / 31.6 min.
- **Comet:** now has a 10-minute day at 260 K. Water ice holds; dry ice, ammonia, methane and CO ice exposed by digging on the day side thaw and outgas. Over 20 minutes of play about 14,000 cells change phase, at 9 s natively (about 130× real time); the shade cache made it 4.6× cheaper.
- **New test:** a pit dug to dry ice on the noon side thaws; at midnight nothing does.

Other bodies get a `[sun]` as their types call for it: inner, warm worlds next.

### Sulfur, iron, carbon and methane worlds (done)

All four are in the viewer (v18) and run at the 20× cap there. Times are 10 minutes of play, native.

- **Sulfur world** (25 km, like Io): 15-minute day at 215 K. About 20 s. The heaviest body so far, and the target for settled pools.
  - Basalt, sulfur, SO₂ frost, tuff and lava-rock strata over hundreds of magma pockets and molten-sulfur pockets, with pyrite and gold veins.
  - A hot mantle with plumes and a molten iron core.
  - Hotspot volcanoes, mostly active; paterae with lava lakes; thrust mountains.
- **Iron world** (30 km, like Mercury): 30-minute day at 700 K. About 3 s.
  - Core from 0 to 0.83 R (crystalline inner core, molten outer core with columns), a troilite layer, a thin mantle with chromite.
  - A dark crust of basalt, breccia and graphite.
  - Shrinkage scarps (faults with cliffs), lava plains, basins, many craters, and sulfur hollows.
- **Carbon world** (20 km): 20-minute day at 420 K. About 6 s.
  - Silicon carbide and graphite crust with coal, tar sands, oil and CO gas pockets.
  - A graphite mantle whose diamond share rises with depth (0.5%, then 2.4%, then 6.7%), a solid diamond layer, a carbide mantle and an iron core.
  - Kimberlite pipes, tar lakes, graphite hills and carbide boulders.
- **Methane world** (30 km, like Titan): no Sun modelled; the sky is held at 92 K by the haze. About 3.5 s.
  - Water-ice bedrock with tholin and ammonia layers and buried ethane pools; a convecting ice shell, a brine ocean, then rock.
  - Liquid methane seas and ethane lakes in basins, tholin dunes, ice mountains and ammonia cryovolcanoes.
  - A hemisphere-wide sea left the first landing zones on liquid, and the swarm couldn't start. Seas are basins now. Drones working on liquids is still to come.

**Engine work along the way** (no existing fingerprint changed except the sandbox demo):

- **Slow heat cadence:** cells cooling slowly, far from a change of state, step every 10 s instead of 1 s.
- **Plume heat:** skips its noise where no plume head can reach.
- **Sunlight:** applied implicitly, so it never overshoots; it only warms materials it could melt or boil; and shade rays stride along the ground line (crater bowls count as open). Iron world went from 31 s to 2.6 s.
- **Ground colliders:** for untouched leaves they are built once and kept. Comet went from 37 s to 18 s per 20 minutes.
- **Comet:** day now 250 K, so water ice no longer melts.
- **Tests:** new generator tests for all four bodies; the sunlight test now checks the warming of a sunlit pit floor (peak 145 K at noon, cold at night).

**Next:** settled pools, then the ice moon, ocean world, lava world and rocky world.

## Settled pools, ice moon, ocean world, lava world

Liquids now sit still until something disturbs them, and buried oceans push back when you breach them.

- **Lids:** lava lakes and liquid seas carry a thin crust (`crust_m`) of their own cooled material. Drones crack it open instead of finding open liquid.
- **Pressure:** a liquid layer under a heavier shell has a balance level. Drill through and it rises up the hole toward that level, drawn from far along the roof so the ocean stays calm. Test: brine rose 530 m of 692 m (77%) in 10 minutes of game time, computed in 0.4 s.
- **Hot pockets:** buried magma and sulfur blobs stay molten until opened.
- **Dry landing:** the swarm doesn't drop drones on zones that are mostly liquid.

**New bodies**

- **Ice moon** (10 km): brine ocean under an ice shell, with cryovolcano plumes and vent minerals on the ocean floor.
- **Ocean world** (40 km): a deep water layer under an ice crust, over hot ice and pressure ice, with chalcopyrite and magnetite at the floor.
- **Lava world** (20 km): a magma ocean under an obsidian skin, with lava lakes and mostly active volcanoes.

Cost per 10 minutes of play: ice moon 0.9 s, ocean world 0.9 s, lava world 17.5 s, sulfur world 25.7 s, active planetoid 71.2 s. The viewer (v19) has all three. In the viewer they run at 18–20× speed.

## Next stages

1. **Rocky world.** The last realistic type: continents, seas, aquifers and ore placement.
2. **Lock in the realistic models.** For every type, freeze a reference fingerprint, a contact sheet of cross-sections and a spec sheet (layers, materials, shares, cost). These become the baseline the next stage can always return to.
3. **Interesting interiors.** Stylised mantles and cores that favour play over strict science. They're layered on top of the locked models, so each realistic version stays recoverable.
4. **Economy.** Starts once every planet exists and the material distribution is known.

## Rocky worlds

Rocky worlds are now a family of four. The living world is only a canvas: no plants, animals or structures yet. Life will come later as its own layers and materials, and could appear on any planet type.

- **Barren world** (20 km, like Mercury or the Moon): a big iron core under a pale feldspar crust (new material: anorthosite). It's battered by 70 craters, its basins are flooded with dark basalt, and cliffs mark where the shrinking body squeezed its crust. Titanium ore sits in the basalt and iron sulfide comes from the impactors. Slow, baking days. 3.2 s per 10 min.
- **Dry world** (30 km, like Mars): red dust (a new material) over basalt and the sediments of a wetter past. Dry riverbeds hold gold, dried lakes hold salt and gypsum, and frozen aquifers sit in the sands. It has polar ice caps, a great canyon, smooth northern lowlands, and giant volcanoes that are mostly dead. 14.6 s per 10 min.
- **Hothouse world** (30 km, like Venus): 735 K everywhere, day and night. Young lava plains surround crumpled highlands of older folded rock. It has shield volcanoes, flat lava domes, coronae, lava tubes, galena "metal snow" in the highlands, and few craters. 10.2 s per 10 min.
- **Living world** (35 km, like Earth): one great ocean and one great continent. The continent is granite under thick sediments (sandstone, shale, limestone, coal, clay) over an ancient basement of gneiss and banded iron. The ocean floor is basalt. It has trenches and volcanic arcs, volcanic islands, karst caves, aquifers (new material: aquifer sandstone), oil, salt, gold placers, kimberlite pipes and polar ice. Soil only lies where the slope is gentle. 8.1 s per 10 min.

**New engine pieces**

- **Level seas:** a lowland can now hold a sea at a set level. Every hollow below that level fills flat, islands stand out of it, and its floor is its own rock. Water that drains out of the open sea is replaced at once, so no hole wanders across its surface. The sea and the rock beneath it start at the surface temperature.
- **Sea floors:** basins and lowlands can lay a floor rock under their fill.
- **Bare slopes:** loose ground can be limited to gentle slopes.
- **Crust map:** a new tool that unrolls a body's crust into a strip picture, for checking surfaces.
- **Drone drops over seas:** while the swarm orbits over a sea, new drones go to the land landing point with the fewest drones.

**Open points for the economy stage:** the living world's swarm grows about half as fast as on a dry body, because only the land can be worked until drones can work in liquids. The hothouse world is slow too, because its 1 m of loose ground gives way quickly to hard basalt.

## Locked in, glamour pass, block textures, dig backdrop

**Lock-in.** Every body's generator output is saved as a fingerprint (`data/reference/fingerprints.toml`). A test fails if any of them changes, and `BLESS=1` re-saves after a deliberate change. `reference/` holds a spec sheet per body: the disc, the surface unrolled, a dive to the centre, a close-up at play scale, other seeds, the layers, the material mix by depth, and the cost of 10 minutes of play. The living world's polar caps were moved closer to the poles before locking (about 15% of the surface instead of a quarter).

**Glamour pass** (`src/glamour.rs`, rules in `data/glamour.toml`). It's a layer over the realistic model, set per body with `glamour` from 0 to 1. At 0 nothing changes (checked by test). It works only below a skin under the surface and only on solid rock. It adds, by density per area so every size of body gets the same richness:

- regions favouring one rock on big bodies
- provinces of other mantle rock (up to 2.5 km)
- round patches, thin wavy bands and lumps of other rock
- ore and gem seams in districts, richer with depth
- crystal-lined geodes, hollow or filled
- crystal caverns from about 0.35
- growth rings in solid cores

Ten new materials: lherzolite, dunite, serpentinite, majorite, ferropericlase, agate, jade, lapis, prismatic crystal, void glass. Deep cells take about twice as long to generate. A 10-minute run costs about 10% more at 0.5.

**Block textures and backdrop** (`src/texture.rs`, viewer only). From 4 pixels per metre, each material kind draws its own texture:

- fitted stones for rock
- beds for sediments
- lit facets for gems
- a sheen for metals
- ice, grains and liquids each their own

Faces open to space catch light on top and fall into shade below. Eaten ground shows the original rock dimmed and greyed, blended softly, scored with drill grooves, and set back with parallax. The viewer (v21) has a glamour slider (default 0.4) and a switch for the new looks.

**Stress tests** (`src/bin/stress.rs`, `run-stress.bat`):

- **Drones:** 100,000 drones run at under 1 s per game minute.
- **Body size:** bodies up to Earth size run at 0.2–1.2 s per game minute, using under 300 MB.
- **Worst case:** the active planetoid with 5,000 drones peaks at 12.4 s per game minute, still 5× real time. The cost comes from how much loose rock and magma the swarm sets moving.
- **Fixed:** rock under the living world's sea was hot enough to boil it where fault steps break the floor. The top 400 m under a sea now stays at sea temperature. An hour of the living world now takes 45 s, down from 226 s.

**Next:** the economy, once you've looked at the glamour defaults. Open points: the living world's swarm levels off at about 3,300 drones in the first hour (land only), and the hothouse world digs slowly.

## Colour schemes

Above glamour 0, each body draws its materials in a colour harmony around a base hue picked from its seed (`src/palette.rs`). There are five harmonies: analogous, complementary, split-complementary, triadic and square.

- **Rock:** toward the base hue.
- **Mantle:** toward the second hue.
- **Ice, seas and sky:** toward the cool counterpart.
- **Gems:** onto the accent hues, and from 0.7 in colours no real gem has.
- **Melts:** keep their glow until 0.85.

The colours are worked out in OKLab, a colour space where equal steps look equal, and lightness is kept, so layers stay readable (0.07% of naturally distinct pairs merge). Only the look changes, never the material. Side by side, seeds differ by about the smallest visible step at 0.3, 3–4 times that at 0.6, and 6–7 times at 1. A body can pin its look with `[palette] scheme` and `hue`. `palettesheet` shows any body in many palettes. Viewer v22 shows the scheme in the view's label.

## Body types and size classes

The viewer (v23) lists only the 16 base body types, in four groups: small bodies, rocky, icy and exotic. Any type can be drawn in seven size classes: asteroid 0.5–3 km, large asteroid 3–10 km, tiny planetoid 10–30 km, planetoid 30–100 km, dwarf planet 100–500 km, small planet 500–2,000 km and planet 2,000–8,000 km. The seed picks the exact radius within the class.

Resizing (`spec::resize_text`) keeps each kind looking like itself:

- **Smaller:** a miniature, with lengths shrunk in proportion.
- **Bigger:** lengths grow with the square root of the size and feature counts grow up to 3×, so big worlds get more features rather than blown-up ones. Surface roughness and layer wobble grow the same way.

`sizesheet` shows types across sizes.

## Procedural chemistry (plan)

The M4a reaction table is hand-written: every pair has its own row. That doesn't scale to the planets we want: Titan-like methane lakes, Io-like sulfur volcanism, carbon worlds with diamond bedrock, ice giants. So the plan is to replace it with a system that works out what happens from the physical and chemical properties of whatever touches. Authored rows stay only as overrides.

### What other games do

| Game | How materials work | How reactions work | What we take |
| --- | --- | --- | --- |
| [Noita](https://noita.wiki.gg/wiki/Documentation:_Reaction) | Each material has a cell type (solid, liquid, sand, gas, fire), density, burnability and tags such as `[meltable]` and `[corrodible]` | A table of pair reactions with a probability. Inputs can be **tags**, so one row covers many materials (`[meltable]` + lava → `[meltable]_molten`) | Rules over classes of material, not single names |
| The Powder Toy | Each element has heat conduction, weight, and low/high temperature (and pressure) transitions | Mostly custom code per element. Molten lava remembers what it melted from, so it refreezes into the same thing | A melt carries its source composition |
| [Oxygen Not Included](https://oxygennotincluded.wiki.gg/wiki/Elements) | Each cell holds element + **mass** + temperature. Elements have specific heat, conductivity and transition temperatures, some with byproducts (sour gas → methane + sulfur) | Few true reactions; most behaviour comes from heat and phase changes. Heat flow between cells scales with conductivity and phase | Variable mass per cell; heat capacity; byproducts on phase change |
| [Dwarf Fortress](https://dwarffortresswiki.org/index.php/DF2014:Material_definition_token) | Rich material tokens: melting, boiling and ignition points, heat-damage points, specific heat, solid and liquid density | Alloys are fixed recipes (bronze = tin + copper) with their own listed properties | A full temperature property set per material |

None of them derive behaviour from chemistry: each is a table or code per material. Our bodies are generated from element compositions already, so we can go further.

### The approach: properties in, behaviour out

Every material is a **composition** (element mass fractions, which we already have), a **phase**, and a **structure** (glass, crystal, powder). Behaviour comes from rules that read properties, in this order:

1. **Heat:** specific heat and conductivity per material; heat flow between cells (as now).
2. **Phase change, for every material:** melting, boiling and sublimation points come from the material's data (or are estimated from its elements). Liquid methane warmed past 112 K boils exactly the way water does at 373 K, with no rule written for methane. A melt remembers its source, so molten basalt refreezes as basalt.
3. **Mixing:** two touching liquids of the same family (metal melts, silicate melts, polar liquids such as water and ammonia, nonpolar ones such as methane and ethane) blend into one liquid with the averaged composition. Liquids of different families don't mix; they separate by density. Molten iron sinks through magma, which is how a planet grows a core, and oil floats on water.
4. **Freezing a mixture, decided by the Hume-Rothery rules:** compare the main elements' atomic radii, crystal structures and electronegativity.
   - Close match: the mixture freezes as one alloy, a solid solution, with properties in between.
   - Poor match: a eutectic. It melts below both parents and separates into two solids as it freezes.
   - Very different electronegativity: an intermetallic compound forms instead.
5. **Redox and burning:**
   - A fuel (carbon, hydrogen, reactive metals, sulfur) touching an oxidiser (oxygen, chlorine, sulfur) above its ignition temperature turns into the standard oxides, with exact element bookkeeping.
   - The heat released comes from formation energies.
   - A metal takes oxygen from a less stable oxide (the Ellingham ordering), so thermite (aluminium + iron oxide) emerges without a row.
6. **Acids and dissolving:**
   - Metals above hydrogen in the activity series dissolve in acid, giving a salt plus hydrogen. Gold and platinum don't.
   - Ionic solids dissolve in polar liquids up to a saturation limit. Evaporating the liquid gives the crystals back, so brine + heat → salt + steam falls out of the rules.
7. **Crystallisation and structure:** how fast a melt cools as it freezes decides what it becomes.
   - Quenched: glass. Lava + water gives obsidian with no special rule.
   - Slow: crystalline rock, with a chance of large crystals and gems, worth more when processed.
   - Dissolved minerals that precipitate slowly grow crystals (geodes).
8. **Pressure and decomposition:**
   - Depth gives pressure from the body's own weight. Carbon deep enough and hot enough becomes diamond, and hydrogen in a giant's interior becomes metallic.
   - Compounds break down above their decomposition temperature: limestone gives quicklime + CO₂; hot methane gives carbon + hydrogen, the "diamond rain" of ice giants.

**Authored overrides** stay, in Noita's style: rows can name classes (`[metal_melt]` + `[water]`) or single materials, and they win over the procedural result. They exist for gameplay tuning and the exotic (the Heart, anything deliberately unphysical).

### Molten gold meets molten silver

- **As liquids:** both are metal melts, so they mix.
- **The radius test:** gold and silver atoms are almost the same size (about 144 pm), both crystallise face-centred cubic, and both have valence 1. The rules call for a **complete solid solution**.
- **On cooling:** the mix freezes as one alloy, **electrum**, with a melting point between silver's (962 °C) and gold's (1064 °C). It doesn't separate.
- **Copper and silver, by contrast:** their atoms differ in size by about 12%. That mix freezes as a eutectic: it stays liquid down to about 780 °C, then splits into copper-rich and silver-rich grains, drawn as a speckled solid.
- **Molten iron meeting magma:** they never mix at all; the iron sinks.

### What this needs underneath

- **An element table** of about 40 elements. Each gets: atomic mass, metallic radius, crystal structure, electronegativity, valence, melting and boiling points, standard oxide and its formation energy, activity-series position, and a value per kg for processing. Materials whose properties aren't written down are estimated from their elements.
- **Variable mass per cell:** a cell stores how many kg it holds, as Oxygen Not Included does, but only for cells that differ from their material's normal fill, so untouched ground costs nothing. Reactions can then make products of any weight while every element still balances exactly. The world's counts become kilograms instead of cells (a part-full cell counts as a fraction of a cell), so the ledger keeps working.
- **Materials made at run time:** an alloy or a new mix gets its own id when first made, with properties derived from its composition. Compositions are rounded to 5% steps so the number of materials stays bounded. Ids are assigned in creation order, which is the same on every run, and are stored in saves. Ids widen from 8 to 16 bits.
- **An interaction cache:** what happens when material A meets material B, by temperature band, is worked out once and reused, so the rules cost nothing per tick after the first contact.
- **Value from composition:** a material's processing multiplier comes from its elements' value per kg and its structure (crystals and gems pay more). New alloys and compounds price themselves.

### Exotic bodies this has to handle

| Body type | Materials | What should happen without special rules |
| --- | --- | --- |
| Titan-like | Water-ice bedrock, liquid methane and ethane, tholin sludge | Methane lakes boil off when drones bring heat; ice is the "rock"; hydrocarbons burn only if an oxidiser is brought in |
| Io-like | Sulfur and SO₂ frost over silicate magma | Sulfur melts and flows at low heat; SO₂ sublimates; magma meeting sulfur boils it |
| Europa-like | Ice shell over a salty ocean | Breaching the shell floods with brine; brine freezes leaving salt crystals |
| Carbon world | Graphite, diamond, silicon carbide | Deep carbon is diamond; graphite burns with oxygen; drones mine gems |
| Iron world | Thin silicate crust over a huge iron core | Metal and silicate melts separate; alloys form where metals meet |
| Ice giant interior | Water, ammonia and methane "ices" under pressure | Methane breaks down to diamond rain; ammonia and water mix |
| Lava world | Molten surface, silicate vapour | Everything exposed is molten; quench products (glass) are the harvest |

### Build order (after M4b)

1. **C1:** element table, derived properties, generic phase changes with latent heat, variable mass per cell. The M4a starter reactions then fall out of the rules and their rows are deleted.
2. **C2:** mixing, alloys, eutectic separation, density separation of liquids that don't mix.
3. **C3:** burning, redox, acids, dissolving and precipitation.
4. **C4:** crystal versus glass, gems, depth pressure, decomposition.
5. **C5:** explosions, built on M4b's debris and drone positions plus M5's gas pressure.

Each step adds test bodies for the exotic types above, with checks that every element balances and every run repeats exactly.

**Round 11:** C1 is done. C2 to C4 now wait until the planet types, the viewer and the generator work are in, so each step can be tested on the bodies it is meant for.

## Decision log

Every settled decision from both rounds, so the earlier versions aren't needed.

| Topic | Decision |
| --- | --- |
| View | 2D cross-section disc, core at the centre |
| Player role | No steering. The player buys upgrades, classes and tiers, launches, picks destinations, and spends Hearts |
| Swarm character | Unrelenting, unaligned, pure consumption; no avatar |
| Win rule | The player always wins eventually; the seed swarm can't be destroyed |
| Technical choices | Mine to make: Rust core, Bevy renderer, built here first, then GitHub, then one Steam app |
| Parameters | Every tunable is in data files; a universe editor may come after v1 |
| Sim tick | 60 Hz, multithreaded |
| Body range | Asteroids upward; gas giants, stars and Dyson spheres after v1 |
| Elements | Real elements only in v1; exotics later |
| Variety | All planet types, stretching physics but human-believable |
| Heart | The final core pixel; many collectible classes; lore later |
| Civilizations | Stone age to Earth-equivalent in v1; sci-fi later as a module |
| Atmosphere | A visual band of tinted vacuum; no atmosphere simulation in v1 |
| Depth look | Underground materials draw darker, as in Terraria and Noita |
| Cell and drone size | 1 m cells; base drones fill one cell; up to 4 drones share a cell |
| Zoom | 8 px per cell at full zoom |
| Saves | Compressed; Steam Cloud after v1 |
| Sandbox | Noita-like and unguided; all fluids; underground gases simulated |
| Currencies | Mass (kilograms × processing multiplier) and Energy only |
| Refining | Compounds become atoms, booked as mass; no transmutation |
| Energy | Hard cap on build rate and active drone count |
| Offline progress | None in v1 |
| Scope | Hobby project |
| Seed swarm | Sits above the atmosphere and is the only factory; envelops the planet over time |
| Drone classes | Harvester, Siphon, Scout, Breacher, Heat-Sink, Refiner, Reactor, Hauler, Necromancer; no Pump, no Replicator |
| Drone death | Wear, hazards, or out-of-range self-destruct. Intact corpses can be revived; debris only refined |
| Haulers | Carry refined mass; tier sets carry speed on screen and mass per second off screen |
| Network range | Refiners, Reactors and the Necromancer extend it |
| Tiers | Mk I to Mk V; no combat class |
| Launch | At a mass-consumed threshold in v1; launch from anywhere later |
| Asteroids | Three (2, 5 and 10 km), each a prestige with no Heart |
| First Heart body | A geologically active planetoid |
| Hearts | Kept in the collection forever; their value is spent in the meta tech tree |
| Intermission | Shows the swarm, stats, collection, tech tree and destination choices |
| Large planets | 1 m everywhere, full detail near the camera only; to be proven by benchmark |
| Dashboards | Live dashboard plus batch reports |
| Specs | Multithreading required; warn before needing higher specs |

Round 3 added:

| Topic | Decision |
| --- | --- |
| Processing multipliers | Rare metals, crystals, compounds and reaction products pay a multiplier when refined. Hearts raise them in the meta tree (for example, obsidian payout ×2) |
| Density | Physical density stays the base reward |
| Cell sharing | Confirmed: up to four drones per cell, one per face, splitting its mass |
| Revival | Haulers deliver intact corpses; Necromancer revival costs a large amount of energy |
| Build cost | Energy is the larger share of a drone's price |
| Asteroid count | Three before the active planetoid |

Round 4 added:

| Topic | Decision |
| --- | --- |
| Title | The Dark Forest Theory |
| Starting multipliers | Rare materials start at 1.5× to 3×; a lever for economy pacing |
| Codex | Materials and compounds are discovered on first processing, which unlocks their upgrades |
| Consumption tracking | Total kilograms of every material consumed, elements and compounds alike |
| Upgrade costs | 1.15× per level |
| Camera | Never changes outcomes; free camera plus action cam |
| Game speed | 1×, 2×, 4×, unlocked in the meta tree |
| Hearts in v1 | 12 classes: 8 archetypes plus 4 rare variants |
| Drone look | Simple shapes, one colour or tint each, to be refined |

Round 5 added:

| Topic | Decision |
| --- | --- |
| Simulation testbed | The Linux machine; it counts as minimum spec |
| Player model | Idle: checks in now and then, buys a couple of things, decides on launch |
| Upgrades | Few and impactful: ×1.5 per level, each level 8× the cost of the last |
| Tutorial drone count | Under 10,000 |
| Tutorial times | First about an hour, second about 30 minutes |
| Second tutorial | Another 2 km asteroid, made faster by prestige perks (option D) |
| Growth feel | Core tuning now; hand-tuning of bursts once the game is visible (M6) |

Round 6 added:

| Topic | Decision |
| --- | --- |
| 10,000 drones | Not a cap: a natural level from rising build costs that happens to coincide with launch; bigger bodies can go past it |
| How bodies are eaten | The seed swarm orbits, dropping landing points as it goes; drones peel from the outside in and share work, so bodies are eaten evenly from all sides |

Round 7 added:

| Topic | Decision |
| --- | --- |
| Dig difficulty | By each cell's hardness and density, not hard stops by layer; dig upgrades bring harder rock within reach |
| Drone choice | Starts nearly blind; the swarm intelligence upgrade teaches effort first, then value; Scouts have a better nose |
| Lava | Can't be dug; exposed lava cools to rock; swallowing it is rare and costs heavy drone wear |
| Test body | A 3 km layered planetoid for testing dig behaviour |

Round 8 added:

| Topic | Decision |
| --- | --- |
| Next step | Tunnelers first, then M4; other drone classes wait until after M4 |
| Tunnelers | Big, wide, energy-hungry drills that open access rather than earn; mostly straight down and unwieldy; their shafts make nearby rock quicker to dig and draw the flock deep |

Round 9 added:

| Topic | Decision |
| --- | --- |
| M4 scope | Split: M4a cell physics, heat and reactions (done); M4b rigid bodies, corpses and debris |
| Mass in the sandbox | Every phase change and reaction keeps mass exactly; a cell keeps its mass whatever it turns into |

Round 10 added:

| Topic | Decision |
| --- | --- |
| Reactions | Procedural chemistry driven by element properties (plan above), with authored overrides only for tuning and the exotic; built after M4b in five steps (C1–C5) |
| Loose bodies | Rock cut free (8–120 cells) falls as Rapier rigid bodies and sets down when still; specks cling; bigger pieces hold as overhangs |
| Drone hazards | Lava and hot rock wreck drones; falling rock crushes them; every drone avoids visible heat; wrecks leave mineable scrap |

Round 11 added:

| Topic | Decision |
| --- | --- |
| Build order | Planet types first (brainstorm, viewer, generator work one type at a time), then C2 to C4 on those bodies |
| Planet viewer | The real generator compiled for the browser, in an interactive page: type and seed, whole cross-section, zoom to cells, reroll, side-by-side compare |
| Planet types | All 13 brainstormed types (ice giant included), plus three exotic types: crystal world, frozen rogue world, stripped giant core. The other exotic types are parked for after v1, with room left for them |
| Cold | Cold speeds drones up (better cooling) but wears them out faster; it never kills. Heat still kills |
| Oceans and liquids | Eaten as fast as mass allows; drones swim through safe liquids with a smaller cooling bonus, and work the floor |
| Life | Procedural complex organisms, earmarked to be built with alien civilizations |
