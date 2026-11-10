# Life L1 — Procedural Flora Design

Nov 6, 2026 · @Benjamin

## Summary

Recommendation: every plant on a planet is grown from a small **genome** by one **growing-tip engine**, and each planet invents its own species to fill the niches its chemistry allows. The same rules make moss, vines, trees, cave fungi and vent forests. Variety comes from combining a few dozen genes, not from hand-drawing hundreds of plants.

- **Two layers, one look.** Ground cover (moss, grass, lichen, microbial mats) is a cheap per-surface field. Anything taller than about a metre (shrubs, vines, trees, stalks, fungi) is real cells in the world: wood, leaf, sap. The existing physics then cuts, burns, freezes and topples them for free.
- **Planet invents its flora.** At generation, the planet lists its niches (sunlit soil, cave dark, vent heat, ice, acid pools and so on). It then breeds 8 to 30 species to fill them, each a genome. Glamour decides how strange they may be, using the same 0 / 0.3 / 1 scale as the interior.
- **Life is gated by chemistry, not by planet type.** A niche is alive when it has a liquid solvent, an energy source and the temperature range for both. Living world and ocean world are lush; an ice moon can still hide a vent forest under its crust.
- **Your four behaviours plus defences.** Plants grow and spread, burn, die and decay, and are worth eating. Defences (thorns, toxic sap, exploding pods, snap traps, armour, regrowth) cost the swarm drones and make the chaos fun.
- **They react to doom.** A local alarm (drones nearby) and a planet-wide doom level (share of mass eaten) drive visible responses. Leaves turn, plants bloom all at once, spore clouds rise, and in the end, seed pods launch into space.
- **Cost stays flat.** Only plants near the camera are drawn as cells. Off-screen plants are a seed, a birth time and a few numbers, grown in closed form when the camera arrives.

## Research: generation methods

No single method gives moss-to-giant-tree range on its own. A **growing-tip engine** (cell-by-cell growth agents, as falling-sand games do) carries the variety. Two classic methods plug into it as gene options.

| Method | How it works | Good at | Weak at | Our use |
| --- | --- | --- | --- | --- |
| Growing tips (falling-sand style) | A tip cell steps through the grid, leaving stem behind; rules pick its turn, branch and stop | Reacting to terrain: hugs walls, dodges rock, hangs from ceilings, regrows after cuts | Big tree crowns can look scraggly | **The core engine** for every plant |
| L-systems ([Prusinkiewicz & Lindenmayer](http://algorithmicbotany.org/papers/abop/abop.pdf)) | A string of rewrite rules unrolled into turtle drawings | Regular, repeating forms: ferns, palms, succulents, spiral stalks | Ignores the world around it unless extended | A **pattern gene**: the tip follows a short rule string |
| Space colonization ([Runions et al. 2007](https://diglib.eg.org/items/b5d756ee-0ab3-436e-a5cb-617d50df78fb/full)) | Scatter attraction points in a crown shape; branches grow toward the nearest ones | Natural, space-filling tree crowns of any silhouette | Costs more; needs a crown shape | A **crown gene** for large trees only |
| Hand-drawn tiles ([Terraria vegetation](https://terraria.wiki.gg/wiki/Vegetation)) | Fixed sprites per biome, placed by rules | Art control | Low variety, no physics | Not used; organs (leaf, flower, fruit) are tiny procedural stamps instead |
| Coverage fields (biome grass, lichen) | A value per surface cell spreads to neighbours | Huge areas for almost no cost | No height or shape | **The ground-cover layer** |

Why growing tips win for this game: plants are built from real cells, so the swarm's cuts, fire, lava, freezing and loose-body falls already apply. Tips also respond to the world. A vine finds a cave wall, a root follows water, a cliff tree leans out over the drop. That reactivity reads as life far more than prettier but rigid models.

The extremophile research ([overview](https://Www.wikipedia.org/wiki/Extremophile)) gave the niche list in the habitat section. Life runs on chemical energy at vents, inside rock pores (endoliths), in acid, in brine and under ice. Plant defences ([Britannica](https://www.britannica.com/story/botanical-barbarity-9-plant-defense-mechanisms)) gave the defence list.

## The flora genome

A species is about 24 genes in six groups. Each gene has a few distinct settings, so the combinations run into the billions. A planet only ever breeds a few dozen of them, so no two planets share a flora. Genes with a glamour floor only appear at or above it, like the interior pass.

| Group | Genes | Example settings | Glamour floor |
| --- | --- | --- | --- |
| Form | body plan, height, internode length, stem thickness and taper, branching rate and angle, crown gene | mat · tuft · rosette · vine · shrub · tree · stalk · cap · tube · crystal lattice | crystal lattice 0.7 |
| Steering | sun pull, gravity sense (up, down, hanging), wall hugging, water seeking, heat seeking, spiral twist | phototropic tree · hanging cave curtain · wall-climbing vine · heat-seeking vent tube | heat seeking 0.3 |
| Organs | leaf shape and size, flowers, fruit or pods, roots, bulbs, glow spots | needle · blade · fan · bladder · lantern · spine crown | glow 0.4 |
| Life cycle | growth speed, lifespan, spread mode, seed range, regrowth from roots | moss creep · bamboo surge · kudzu runner · seed cannon · spore cloud | seed cannon 0.5 |
| Metabolism | energy source, solvent, temperature band, tolerances (acid, salt, radiation) | sunlight + water · sulphide chemistry · heat gradient · radiation · rock-eating | radiation 0.6 |
| Defence | none, or one or two of the defences in the behaviour section | thorns · toxic sap · explosive pods · snap trap · armour · resin | snap trap 0.6 |

Colour follows the planet's palette harmony from the existing palette code. At glamour 0, plants keep natural greens, browns and fungal whites. As glamour rises, they take the body's harmony colours and gain glow and odd materials.

Species are bred from their niche, not picked at random. The niche fixes metabolism and tolerances first. Body plan and steering are then drawn from what suits that niche: hanging and glowing in caves, wall hugging on cliffs, heat seeking at vents. The rest is free. Every species therefore makes sense where it lives, yet looks fresh. Each species also gets a generated name from the same seed, shown only in the dev panel.

## Body plans: moss to giant trees

Ten body plans cover the whole range, all from the same tip rules. Cells are 1 m, so anything under a metre lives in the ground-cover layer and is drawn as a tinted fringe on the surface.

| Body plan | Size | Built from | Typical niche |
| --- | --- | --- | --- |
| Mat (moss, lichen, microbial film) | under 1 m | ground-cover field only | rock faces, ice, cave walls, vent rims |
| Tuft (grass, reeds, sedges) | under 1 m | ground-cover field, taller fringe | open soil, shallows |
| Rosette (ferns, succulents, bromeliads) | 1–4 m | one tip, L-system pattern gene | soil, ledges |
| Vine and runner | 2–60 m long | tips with strong wall hugging | cliffs, cave walls, tree trunks |
| Shrub | 1–6 m | many short branching tips | everywhere with soil |
| Tree | 8–80 m (giants to 150 m at high glamour) | trunk tip, then crown gene | sunlit soil |
| Stalk (bamboo, cane, spire) | 5–40 m | one fast tip, little branching | wet soil, burned ground |
| Cap (mushroom, puffball, bracket) | 1–25 m | short stalk and a cap stamp | caves, rotting wood, dark |
| Tube (vent worms, kelp, sea fans) | 2–30 m | upright tips in liquid | sea floor, vents |
| Lattice (crystal and glass growths) | 2–60 m | straight tips at fixed angles | glamour 0.7 and up: anywhere strange |

Hanging versions of vine, cap and tube grow down from cave ceilings. Roots are tips that grow down into soil from every rooted plant. They anchor it and hold it in place until the ground under it is eaten.

## Where life can live

A spot can hold life when three things meet: a **liquid solvent** (water, brine, ammonia or methane), an **energy source**, and a temperature where both work. The check runs per niche at generation, so any body can have pockets of life. Planet type only sets how many niches pass.

| Energy source | Needs | Niches it opens |
| --- | --- | --- |
| Sunlight | sky above, solvent nearby, some air | surfaces, shallows, cave mouths |
| Chemical (sulphide, methane, iron) | hot fluid meeting cold rock or sea | sea-floor vents, hot springs, sulphur pools |
| Heat gradient (glamour 0.3+) | a steep temperature step | lava edges, magma-chamber rims |
| Radiation (glamour 0.6+) | radioactive ore | ore seams, deep cave pockets |
| Rock-eating (endoliths) | slow seepage through rock | inside porous rock near water, in the deep crust |
| Decay | dead plants or peat | under forests, cave floors below living areas |

**Niches** are found from the generated world: sunlit soil, cliff faces, shallows, sea floor, vent fields, cave dark, cave mouths, ice and permafrost, acid or brine pools, lava edges, rock pores. Each niche that passes the check gets two to five species bred for it.

**Expected results** with today's bodies (a sketch; the real answer comes from each body's chemistry):

| Body | Expected life |
| --- | --- |
| Living world, earth test | Lush everywhere: forests, grass, cave fungi, kelp |
| Ocean world | Kelp and vent forests below; island and ice-edge mats |
| Hothouse world | Heat-tolerant shrubs and stalks if water survives; else acid-cloud mats in caves |
| Ice moon | Hidden vent forests under the ice crust; nothing on the surface |
| Methane world | Strange slow life in methane lakes (glamour 0.5+) |
| Sulphur world | Chemical-energy mats and tubes at sulphur pools |
| Dry, carbon, lava, iron, barren, asteroids, comet | Mostly lifeless; rare pockets where a niche passes |

A body file can also force life on or off, or scale how much (a `life` setting from 0 to 2), so you can tune any planet by hand.

## Behaviours

All four behaviours you picked ride on systems that already exist. Plant cells are materials, so cutting, fire, heat and falling need no new physics.

**Grow and spread**

- Each plant grows at its gene's speed, from moss creeping centimetres a day to bamboo-style surges of metres a minute (game time).
- Spread modes: runners along the ground, seeds dropped or flung (ballistic, using the existing fling), spores drifting on the air into caves, fragments that root where they land.
- Fast growers recolonise eaten ground and fresh craters, so the swarm leaves scars that green over.
- Fire-adapted species drop seeds after a burn, so a burned forest regrows as a different forest.

**Burn**

- New rows in burning.toml: green wood, dry wood, leaf, dry grass and litter, resin, oil-fruit.
- Living plants hold water and are slow to light; dead and dry ones go up fast. A drought or a nearby lava breach can set off a wildfire.
- On worlds without oxygen, plants do not burn but still cook and die near heat.

**Die and decay**

- Plants die from age, from losing their ground or light, from heat, cold, or being cut off from water.
- Dead plants turn to deadwood and litter, then to humus and soil over time. Deep, old layers become peat, which links to the existing coal.
- Decay feeds cave fungi and the decay niche, so a dying forest grows mushrooms.

**Worth eating**

- Plant materials carry real mass and are rich in energy, so cutters refuel well on them.
- A bonus value for biomass fits the tech tree later (for example, a branch that only biomass unlocks).

**Defences** (one or two per species; most species have none)

| Defence | What it does to the swarm | Glamour floor |
| --- | --- | --- |
| Armour (bark, silica) | Harder to cut: takes longer and wears drones | 0 |
| Thorns | Wear damage when a drone cuts or brushes them | 0 |
| Toxic or acid sap | Damage when swallowed; acid eats drone hulls over time | 0.2 |
| Resprouting | Cut it and it grows back from roots, fast | 0 |
| Resin | Lights when cut near heat; makes fire traps | 0.2 |
| Explosive pods | Burst when cut: blast, flung seeds, nearby drones knocked out | 0.4 |
| Spore cloud | Clouds that slow and clog drones for a while | 0.4 |
| Snap trap | Grabs a drone and crushes it; large ones grab several | 0.6 |
| Electric or light flash | Stuns drones in range for a few seconds | 0.8 |

Drones stay reckless, as agreed. Defences just make them die in more interesting ways, and they feed the Losses and Explosion events the camera already watches.

**Animation** (added by Benjamin)

- Every plant that is not rigid gets an idle motion from two genes: stiffness and sway. Grass ripples, vines swing, tall trees lean slowly, cave curtains drift, kelp waves in currents.
- Plants with an active defence get an attack motion: a vine whip, a trap snapping shut, a pod swelling then bursting, a gas vent puffing, tendrils reaching for a passing drone.
- Rigid species (crystal lattices, armoured trunks, stone-like mats) stay still.
- Physics stays on the 1 m grid. Motion is drawn on top: near the camera, plant cells are drawn as a separate layer and each cell is offset by its height up the plant and its distance from the root, so the grid never moves. Off-screen, nothing animates.

## Reacting to doom

Plants cannot run, so they show doom through colour, bloom and launch. Two signals drive it, and fauna will reuse both later.

- **Local alarm:** drones within a species' sense range, or ground shaking from a breach or blast near it.
- **Doom level:** the share of the planet's mass eaten, plus big events such as losing the atmosphere or a sea draining.

&#91;embedded content: doom stages · by share of planet mass eaten\]

Stage boundaries are a first guess to tune by eye. Exodus is the showpiece: pods arc off the surface into space as a new camera event, so a dying world visibly tries to save itself.

## Performance plan

Target: life costs under 1 ms per sim step on the living world with the camera on a forest, and close to nothing off-screen. It follows the off-screen approximation rule you already approved.

- **Plants as records.** Each plant is a small record: species, root cell, seed, birth time, health. A 35 km living world holds roughly 50,000 to 150,000 of them, a few megabytes.
- **Grown on demand.** A plant's shape is a pure function of its seed, species and age. When tiles near the camera need it, it is grown to its current age in one go and stamped into the world. Off-screen plants just age as numbers.
- **Live tips only in sight.** Growing tips step cell by cell only near the camera or near busy drones, and on a budget per step like the rock checks.
- **Ground cover is a field.** One small value per surface column (species and density), updated a slice at a time. Drawn as a tinted fringe by the tile code.
- **Damage is remembered.** Once drones or fire touch a plant, its cells are real and stay real. Its record notes that it is now hand-edited, so it is never regrown over the damage.
- **Saves.** Records and the ground-cover field go into the save; touched plants are already in the world cells.

The main risk is stamping many plants at once when the camera cuts to a new forest. The fix is the same as tiles: stamp the nearest first, a budget per frame.

## Build steps and decisions

Six steps, each testable on its own, with a small check-in after each. Fauna (later) reuses the niche check, the doom signal and the record approach.

1. **L1a: Plant materials and data.** Wood, leaf, sap, litter, humus, peat, resin, plus burning rows and a `life` setting per body. Test: materials load, burn and decay.
2. **L1b: Niches and species breeding.** Find niches from the generated world, run the chemistry check, breed species from genomes by glamour. Test: each body lists its niches and species; the dev panel shows them.
3. **L1c: Growing-tip engine.** The ten body plans grown from genomes, written as cells. Test: preview PNGs of a species gallery at glamour 0, 0.3 and 1 for you to judge.
4. **L1d: Placement, records and ground cover.** Plants seeded across each niche; grown on demand near the camera; ground-cover fringe in the tiles. Test: living world fly-over on your PC.
5. **L1e: Life cycle and defences.** Growth, spread, death, decay, regrowth, the defences hooked into drone damage, and the idle and attack animations. Test: forest cut, burned and regrowing; drone losses to defences.
6. **L1f: Doom reactions.** Local alarm, planet doom level, the four stages, launch pods as a camera event. Test: a full eat of the living world on your PC.

Decisions (answered):

- [x] Launched seed pods are for show. Pods that fly into the mother swarm add their mass to the take.
- [x] Defences are harsh, and only count when they damage a drone: caustic gas, vine whips that knock drones out of the air, carnivorous plants that catch and digest them.
- [x] Species don't matter to the player. Everything eaten becomes biomass plus its atomic mass totals; names stay in the dev panel only.
- [x] First deliverable: a small launchable tool that grows life for any body type, seed and size, so you can judge it before it goes in the game.
