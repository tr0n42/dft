# Element Abundance Master Table

Nov 9, 2026 · @Benjamin

The baseline is now natural: every element an ore brings sits at 0.5 to 3× Earth's crust on average, and anything richer comes from a body's rolled specialties, which the meta tree will be able to steer. Measured from the generator itself across the 23 bodies; nothing here is locked, it is all tuning data.

## How to read it

Numbers are parts per million by mass, measured by sampling each body's generator 2 million times per zone (tool: `abundance`, output `data/reference/abundance.csv`).

- **Skin** is the outer 500 m: what a run digs first. **Whole** is the entire cross-section, core included.
- **Avg** counts each of the 23 bodies once, so icy and metal worlds pull the rock-forming elements down and push carbon, nitrogen, sulfur and nickel up.
- **Boost** is the plain generator's skin average (no specialties) divided by Earth's crust: 1 is natural.
- **Ore bodies** is how many bodies have a mineral of the element at 0.1% or richer; elsewhere it is only a trace (three dots in the stockpile) or absent (a slash).
- **Tier** by skin average: Common over 1,000 ppm, Minor 10 to 1,000, Trace 0.1 to 10, Rare under 0.1, Radiogenic only for the four no body holds yet.
- **under 0.01** means the element is there but too thin for 6 million samples per body to measure.

## Master table

The plain generator, without specialties: 16 common, 14 minor, 20 trace, 21 rare, and 4 (technetium, polonium, astatine, francium) left to radioactivity. What still runs well above 1× is natural for the bodies we have rather than ore: carbon, nitrogen, sulfur and chlorine from carbon, ice and sulfur worlds; nickel, cobalt, chromium and the platinum metals from meteoritic iron and cores; lead from the lead frost hand-placed on the hothouse world.

| El | Name | Earth crust ppm | Game skin ppm | Game whole ppm | Boost | Tier | Ore bodies | Richest body (whole) | Main hosts |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| H | hydrogen | 1,400 | 22,110 | 10,729 | 16× | Common | 19/23 | ocean-world 80,754 | metallic hydrogen, hydrogen, methane ice |
| He | helium | 8.0e-3 | 1.4e-2 | 7.2e-3 | 1.8× | Rare | 13/23 | strata-demo 0.1 | helium, radon gas, radiogenic gas |
| Li | lithium | 20 | 6.8 | 3 | 0.34× | Trace | 6/23 | test-planetoid-3km 41 | spodumene, lepidolite |
| Be | beryllium | 2.8 | 1.5 | 0.75 | 0.52× | Trace | 6/23 | test-planetoid-3km 14 | beryl, emerald |
| B | boron | 10 | 1.7 | 0.61 | 0.17× | Trace | 8/23 | ocean-world 7.8 | borax |
| C | carbon | 200 | 34,103 | 30,716 | 171× | Common | 16/23 | carbon-world 526,345 | graphite, diamond, crude oil |
| N | nitrogen | 19 | 29,062 | 2,864 | 1,530× | Common | 7/23 | ocean-world 29,865 | nitrogen ice, liquid nitrogen, liquid ammonia |
| O | oxygen | 461,000 | 467,724 | 338,454 | 1× | Common | 23/23 | comet 708,196 | oxygen ice, liquid oxygen, pressure ice |
| F | fluorine | 585 | 201 | 93 | 0.34× | Minor | 12/23 | test-planetoid-3km 1,713 | fluorite, lepidolite, pyrochlore |
| Ne | neon | 5.0e-3 | 3.4e-3 | 2.9e-4 | 0.68× | Rare | 9/23 | strata-demo 3.6e-3 | radiogenic gas |
| Na | sodium | 23,600 | 10,065 | 4,511 | 0.43× | Common | 23/23 | sandbox-demo 18,415 | rock salt, caliche, lapis |
| Mg | magnesium | 23,300 | 51,137 | 96,967 | 2.2× | Common | 23/23 | sulfur-world 154,853 | ferropericlase, harzburgite, dunite |
| Al | aluminium | 82,300 | 34,445 | 18,195 | 0.42× | Common | 23/23 | sandbox-demo 78,398 | ruby, corundum, bauxite |
| Si | silicon | 282,000 | 190,248 | 144,550 | 0.67× | Common | 23/23 | carbon-world 243,278 | silicon carbide, gold-quartz, void glass |
| P | phosphorus | 1,050 | 518 | 935 | 0.49× | Minor | 23/23 | metal-asteroid 5,460 | apatite, xenotime, schreibersite |
| S | sulfur | 350 | 24,213 | 18,743 | 69× | Common | 20/23 | iron-world 54,351 | sulfur, pyrite, sulfur dioxide frost |
| Cl | chlorine | 145 | 2,787 | 1,370 | 19× | Common | 11/23 | ocean-world 9,047 | rock salt, bittern salt, sea ice |
| Ar | argon | 3.5 | 1.7 | 0.16 | 0.49× | Trace | 13/23 | strata-demo 2 | radiogenic gas, radon gas |
| K | potassium | 20,900 | 3,695 | 1,335 | 0.18× | Common | 22/23 | strata-demo 14,068 | bittern salt, lepidolite, gneiss |
| Ca | calcium | 41,500 | 34,318 | 15,348 | 0.83× | Common | 23/23 | sandbox-demo 68,513 | fluorite, apatite, limestone |
| Sc | scandium | 22 | 2.3 | 0.99 | 0.1× | Trace | 6/23 | test-planetoid-3km 18 | thortveitite |
| Ti | titanium | 5,650 | 5,764 | 2,226 | 1× | Common | 23/23 | sandbox-demo 20,446 | ilmenite, regolith, magma |
| V | vanadium | 120 | 85 | 52 | 0.71× | Minor | 7/23 | sandbox-demo 248 | vanadinite, magnetite |
| Cr | chromium | 102 | 801 | 3,000 | 7.9× | Minor | 22/23 | iron-world 10,335 | chromite, dunite, sulfur-rich core melt |
| Mn | manganese | 950 | 1,106 | 748 | 1.2× | Common | 23/23 | test-planetoid-3km 1,638 | pyrolusite, rhodochrosite, wolframite |
| Fe | iron | 56,300 | 81,996 | 281,134 | 1.5× | Common | 23/23 | iron-world 742,217 | kamacite, iron-nickel, platinum-iron alloy |
| Co | cobalt | 25 | 130 | 1,007 | 5.2× | Minor | 20/23 | metal-asteroid 3,778 | cobaltite, taenite, plessite |
| Ni | nickel | 84 | 4,303 | 26,608 | 51× | Common | 22/23 | metal-asteroid 120,423 | pentlandite, taenite, schreibersite |
| Cu | copper | 60 | 61 | 76 | 1× | Minor | 9/23 | ocean-world 748 | native copper, chalcopyrite |
| Zn | zinc | 70 | 35 | 17 | 0.5× | Minor | 6/23 | test-planetoid-3km 335 | sphalerite |
| Ga | gallium | 19 | 5.2 | 2.2 | 0.28× | Trace | 0/23 | sandbox-demo 18 | traces only |
| Ge | germanium | 1.5 | 1.8e-2 | 2.9 | 0.012× | Rare | 0/23 | metal-asteroid 29 | prismatic crystal, resonant crystal |
| As | arsenic | 1.8 | 5.3 | 4.9 | 2.9× | Trace | 11/23 | test-planetoid-3km 86 | arsenopyrite, cobaltite, sperrylite |
| Se | selenium | 5.0e-2 | under 0.01 | under 0.01 | — | Rare | 6/23 | — | clausthalite |
| Br | bromine | 2.4 | 5.1 | 2.6 | 2.1× | Trace | 8/23 | ice-moon 20 | bittern salt |
| Kr | krypton | 1.0e-4 | 1.7e-4 | 1.4e-5 | 1.7× | Rare | 0/23 | strata-demo 1.8e-4 | traces only |
| Rb | rubidium | 90 | 17 | 5.9 | 0.19× | Minor | 6/23 | rubble-asteroid 24 | lepidolite |
| Sr | strontium | 370 | 103 | 11 | 0.28× | Minor | 5/23 | strata-demo 205 | celestine, marble, limestone |
| Y | yttrium | 33 | 17 | 9 | 0.53× | Minor | 6/23 | test-planetoid-3km 171 | xenotime |
| Zr | zirconium | 165 | 92 | 19 | 0.55× | Minor | 13/23 | strata-demo 196 | zircon |
| Nb | niobium | 20 | 11 | 7.1 | 0.56× | Minor | 7/23 | test-planetoid-3km 141 | pyrochlore, columbite, tantalite |
| Mo | molybdenum | 1.2 | 0.65 | 0.27 | 0.54× | Trace | 6/23 | test-planetoid-3km 4.6 | molybdenite |
| Tc | technetium | 3.0e-15 | 0 | 0 | — | Radiogenic only | 0/23 | — | none |
| Ru | ruthenium | 1.0e-3 | under 0.01 | 0.39 | — | Rare | 5/23 | active-planetoid 6.4 | laurite, osmiridium |
| Rh | rhodium | 1.0e-3 | 8.1e-7 | 2.1e-2 | 0.00081× | Rare | 9/23 | dry-world 0.48 | hollingworthite, platinum nugget |
| Pd | palladium | 1.5e-2 | 1.1e-6 | 1.1 | 7.2e-05× | Rare | 15/23 | ice-moon 5.4 | braggite, platinum nugget, platinum-iron alloy |
| Ag | silver | 7.5e-2 | under 0.01 | under 0.01 | — | Rare | 6/23 | — | native silver |
| Cd | cadmium | 0.15 | 0.16 | 8.1e-2 | 1.1× | Trace | 6/23 | test-planetoid-3km 1.6 | greenockite, sphalerite |
| In | indium | 0.25 | 7.4e-3 | 3.7e-3 | 0.03× | Rare | 0/23 | test-planetoid-3km 7.1e-2 | traces only |
| Sn | tin | 2.3 | 3 | 1.3 | 1.3× | Trace | 6/23 | test-planetoid-3km 21 | cassiterite |
| Sb | antimony | 0.2 | 0.31 | 0.11 | 1.5× | Trace | 6/23 | test-planetoid-3km 2.3 | stibnite |
| Te | tellurium | 1.0e-3 | under 0.01 | under 0.01 | — | Rare | 6/23 | — | calaverite |
| I | iodine | 0.45 | 3.7e-3 | 1.9e-3 | 0.0082× | Rare | 3/23 | ice-moon 2.0e-2 | caliche |
| Xe | xenon | 3.0e-5 | 8.5e-5 | 7.2e-6 | 2.8× | Rare | 0/23 | strata-demo 9.0e-5 | traces only |
| Cs | caesium | 3 | 2.1 | 0.92 | 0.7× | Trace | 6/23 | test-planetoid-3km 17 | pollucite |
| Ba | barium | 425 | 284 | 56 | 0.67× | Minor | 7/23 | strata-demo 535 | baryte, volcanic ash, tuff |
| Hf | hafnium | 3 | 2.8 | 0.62 | 0.95× | Trace | 13/23 | strata-demo 6.5 | zircon |
| Ta | tantalum | 2 | 6 | 3 | 3× | Trace | 6/23 | test-planetoid-3km 63 | tantalite, columbite |
| W | tungsten | 1.2 | 4.2 | 2.3 | 3.3× | Trace | 6/23 | test-planetoid-3km 43 | wolframite |
| Re | rhenium | 7.0e-4 | 4.0e-3 | 1.7e-3 | 5.8× | Rare | 6/23 | test-planetoid-3km 2.9e-2 | molybdenite |
| Os | osmium | 1.5e-3 | 3.2e-2 | 4.6 | 21× | Rare | 9/23 | active-planetoid 64 | osmiridium, platinum nugget |
| Ir | iridium | 1.0e-3 | 6.6e-2 | 4 | 66× | Rare | 9/23 | active-planetoid 58 | osmiridium, platinum nugget |
| Pt | platinum | 5.0e-3 | 7.6e-2 | 4.2 | 15× | Rare | 19/23 | ice-moon 17 | platinum nugget, sperrylite, braggite |
| Au | gold | 4.0e-3 | 1.1e-2 | 2.4e-2 | 2.8× | Rare | 13/23 | metal-asteroid 0.38 | gold, calaverite, gold-quartz |
| Hg | mercury | 8.5e-2 | 0.18 | 0.11 | 2.1× | Trace | 6/23 | test-planetoid-3km 2.2 | cinnabar |
| Tl | thallium | 0.85 | 0.1 | 5.5e-2 | 0.12× | Trace | 6/23 | test-planetoid-3km 1.3 | lorandite |
| Pb | lead | 14 | 507 | 166 | 36× | Minor | 14/23 | hothouse-world 3,347 | galena, vanadinite, clausthalite |
| Bi | bismuth | 8.5e-3 | under 0.01 | under 0.01 | — | Rare | 6/23 | — | native bismuth, bismuthinite |
| Po | polonium | 2.0e-10 | 0 | 0 | — | Radiogenic only | 0/23 | — | none |
| At | astatine | 3.0e-27 | 0 | 0 | — | Radiogenic only | 0/23 | — | none |
| Rn | radon | 4.0e-13 | 3.6e-9 | 2.3e-9 | 8,992× | Rare | 0/23 | strata-demo 3.2e-8 | traces only |
| Fr | francium | 1.0e-24 | 0 | 0 | — | Radiogenic only | 0/23 | — | none |
| Ra | radium | 9.0e-7 | 2.6e-7 | 8.9e-8 | 0.28× | Rare | 0/23 | living-world 7.7e-7 | traces only |
| Th | thorium | 9.6 | 5.5 | 4.1 | 0.57× | Trace | 13/23 | strata-demo 38 | thorite, zircon |
| U | uranium | 2.7 | 7.7 | 0.56 | 2.8× | Trace | 14/23 | barren-world 5.6 | uraninite, pitchblende, zircon |

## Bodies

With a natural baseline, a body's specialties are what make it worth a trip: the standouts are measured with each body's rolled specialties (★ a jackpot) against the average plain body, at the seed each body file sets. A cache on an asteroid lifts its element tens of thousands of times over the baseline.

| Body | Radius km | Specialties (this seed) | Richest in (whole body, × the average plain body) | Elements present | With ore |
| --- | --- | --- | --- | --- | --- |
| active-planetoid | 30 | Vanadium black shale, Scandium pegmatites | Sc 27×, Mo 24×, Ru 15×, Ir 14×, Os 13× | 66 | 59 |
| barren-world | 20 | Platinum-rich iron, Germanium-gallium iron, Scandium laterite | Ge 36×, Ga 19×, U 10×, Pt 3.3×, Co 1.9× | 28 | 22 |
| carbon-world | 20 | Cobalt-bearing basalt, ★ Tellurium bonanza | Au 462×, C 17×, Pd 2.9×, Pt 2.3×, Si 1.7× | 40 | 29 |
| comet | 2 | Iodine ice | I 36,381×, N 7.3×, H 6.8×, C 4.5×, O 2.1× | 25 | 17 |
| dry-world | 30 | Xenon traps, Lithium clays, Helium field | Xe 6,027×, Kr 2,695×, He 89×, Rh 23×, Ne 3.4× | 52 | 45 |
| earth-test | 6,371 | Strontium-barium beds, Lithium clays | Cr 1.7×, S 1.6× | 67 | 58 |
| hothouse-world | 30 | Lithium pegmatite field | Pb 20×, Hf 3.3×, Th 3.3×, Zr 3.2×, K 3.2× | 65 | 56 |
| ice-moon | 10 | Cobalt-bearing basalt, Borate playa | I 10×, Br 7.7×, Cl 5.6×, Pd 5.1×, Pt 4× | 41 | 31 |
| iron-world | 30 | Iridium dust layer, Germanium-gallium iron | Ge 30×, Ga 16×, Pd 4×, Pt 4×, Cr 3.4× | 26 | 21 |
| lava-world | 20 | Lithium clays, Cobalt-bearing basalt | Ga 2.7×, Ti 2.6×, V 1.9×, Cr 1.9×, Na 1.6× | 36 | 29 |
| living-world | 35 | Germanium coal | Ru 6.6×, Th 6.4×, Ir 5.8×, Os 5.8×, Rn 5.1× | 71 | 64 |
| metal-asteroid | 3 | Niobium cache | Nb 216×, Au 16×, Ge 10×, P 5.9×, Ni 4.5× | 29 | 24 |
| methane-world | 30 | Bromine sea, Lithium brine | Br 269×, Li 57×, I 7.8×, H 4.5×, Cl 4.3× | 36 | 28 |
| ocean-world | 40 | Iodine ice, Iridium dust layer | I 657×, B 12×, N 10×, Cu 10×, H 7.5× | 32 | 25 |
| rogue-world | 40 | Iodine ice, Germanium-gallium iron | I 1,780×, Ge 52×, Ga 20×, B 5.7×, Cl 5.5× | 40 | 30 |
| rubble-asteroid | 2 | Rhodium cache | Rh 161,342×, As 501×, Rb 4×, Mn 2×, Na 1.9× | 26 | 20 |
| rubble-demo | 1 | Vanadium cache, ★ Uranium lode | Ra 35,136×, U 14,857×, Pb 392×, V 179×, Au 2.8× | 30 | 24 |
| sandbox-demo | 0.2 | Scandium laterite | Sc 45×, Ti 9.2×, Ga 8.2×, V 4.7×, Ca 4.5× | 14 | 11 |
| strata-demo | 2 | Scandium laterite, Uranium black shale | U 82×, Sr 20×, Xe 14×, Ne 14×, Kr 14× | 71 | 64 |
| sulfur-world | 25 | Iridium dust layer, Lithium clays, Rubidium granite | Li 3.2×, S 2.7×, Mg 1.6× | 39 | 28 |
| test-planetoid-3km | 2 | Nickel-palladium camp, Scandium laterite | Tl 28×, Sb 24×, Sc 20×, Zn 19×, In 19× | 66 | 59 |
| tutorial-1-2km | 1 | Vanadium cache, ★ Uranium lode | Ra 38,841×, U 16,424×, Pb 391×, V 178×, Rb 3.2× | 29 | 23 |
| tutorial-2-3km | 2 | Cobalt-bearing basalt, Molybdenum cache | Re 24,744×, Mo 24,744×, Rb 3.7×, S 2×, Mn 2× | 27 | 21 |

A specialty that only nudges a common rock (sulfur-world's lithium clays, earth-test's strontium beds) barely shows against a whole planet; vein and cache specialties stand out by orders of magnitude. Two of these bodies happen to share a seed (rubble-demo and tutorial-1-2km), which is why they rolled alike.

## Specialties

A body rolls 0 to 3 specialties from its seed and sometimes a jackpot; each is real geology turned up for the game. Veins raise one ore (a boost raises its scarcity, so ×25 means 25 times the natural amount), sometimes at the cost of others; throughout puts a trace element through all of a common rock, so the swarm gathers it slowly from everything it digs; jackpots and caches are a few huge pods. Odds are the share of 200 seeds per body type that rolled each, averaged over 22 body types (earth-test left out); the catalogue is `data/specialties.toml`.

**Asteroids and comets are pit stops.** They roll more (95% get at least one, 50% get two or more) and 35% hold a jackpot, against 8% for planets. Only they can roll the 22 element caches: 2 to 5 pods of one ore, 30 to 150 m across, in a body only 1 to 3 km wide, so a single stop can yield tens of kilotonnes to megatons of one element. An asteroid averages 1.3 caches.

| Cache | Ore |
| --- | --- |
| Cobalt, Nickel, Copper, Chromium, Manganese, Titanium, Vanadium | cobaltite, pentlandite, native copper, chromite, pyrolusite, ilmenite, vanadinite |
| Tungsten, Tin, Molybdenum, Tantalum, Niobium, Antimony, Bismuth, Mercury | wolframite, cassiterite, molybdenite, tantalite, pyrochlore, stibnite, native bismuth, cinnabar |
| Lithium, Beryllium, Germanium | spodumene, beryl, germanium-rich sphalerite |
| Silver, Palladium, Rhodium, Uranium | native silver, braggite, hollingworthite, pitchblende |

| Specialty | Kind | Odds | Most likely on |
| --- | --- | --- | --- |
| Germanium zinc ore | Veins | 1.0% | hothouse-world 6%, test-planetoid-3km 5% |
| Indium tin ore | Veins | 1.4% | test-planetoid-3km 7%, hothouse-world 7%, active-planetoid 7% |
| Rhenium molybdenite | Veins | 0.9% | thinly, wherever it fits |
| Gallium bauxite and zinc | Veins | 0.8% | thinly, wherever it fits |
| Telluride gold field | Veins | 0.9% | test-planetoid-3km 5%, hothouse-world 5%, active-planetoid 5% |
| Selenide veins | Veins | 3.4% | sulfur-world 17%, ocean-world 14%, ice-moon 14% |
| Lithium pegmatite field | Veins | 0.9% | active-planetoid 5% |
| Rare-metal pegmatites | Veins | 1.0% | thinly, wherever it fits |
| Scandium pegmatites | Veins | 0.8% | hothouse-world 6% |
| Caesium pegmatite | Veins | 0.9% | test-planetoid-3km 6% |
| Tin-tungsten belt | Veins | 1.0% | hothouse-world 6%, active-planetoid 5% |
| Five-element veins | Veins | 1.3% | dry-world 8%, test-planetoid-3km 6% |
| Mercury-thallium belt | Veins | 1.1% | active-planetoid 6%, hothouse-world 5% |
| Uranium province | Veins | 4.8% | lava-world 22%, carbon-world 17%, sulfur-world 14% |
| Platinum reef | Veins | 1.2% | test-planetoid-3km 6%, dry-world 6%, strata-demo 6% |
| Nickel-palladium camp | Veins | 2.2% | ocean-world 10%, ice-moon 10%, dry-world 8% |
| Niobium carbonatite | Veins | 0.9% | thinly, wherever it fits |
| Chromite district | Veins | 3.0% | iron-world 25%, lava-world 18%, dry-world 6% |
| Hafnium zircons | Veins | 2.0% | lava-world 8%, sulfur-world 8%, carbon-world 5% |
| Helium field | Veins | 2.5% | sulfur-world 16%, carbon-world 13%, dry-world 6% |
| Xenon traps | Veins | 2.5% | sulfur-world 16%, carbon-world 16%, dry-world 6% |
| Strontium-barium beds | Veins | 0.6% | thinly, wherever it fits |
| Fluorspar district | Veins | 0.9% | dry-world 6% |
| Borate playa | Veins | 2.9% | methane-world 14%, rogue-world 13%, ocean-world 12% |
| Iodine desert | Veins | 0.4% | dry-world 5% |
| Manganese basin | Veins | 0.8% | dry-world 6%, test-planetoid-3km 6% |
| Vanadium-lead oxide zone | Veins | 0.8% | test-planetoid-3km 6%, hothouse-world 6% |
| Silver bonanza veins | Veins | 1.2% | hothouse-world 8%, test-planetoid-3km 7%, active-planetoid 7% |
| Cobalt-bearing basalt | Throughout | 11.5% | sandbox-demo 48%, barren-world 30%, lava-world 22% |
| Lithium clays | Throughout | 2.2% | lava-world 18%, sulfur-world 6%, active-planetoid 6% |
| Rubidium granite | Throughout | 3.9% | sulfur-world 20%, rogue-world 12%, lava-world 12% |
| Vanadium black shale | Throughout | 0.6% | strata-demo 6%, living-world 6% |
| Uranium black shale | Throughout | 1.2% | carbon-world 14%, strata-demo 5%, living-world 5% |
| Germanium coal | Throughout | 0.8% | carbon-world 12% |
| Scandium laterite | Throughout | 10.2% | sandbox-demo 42%, barren-world 28%, iron-world 26% |
| Iridium dust layer | Throughout | 11.7% | sandbox-demo 52%, barren-world 28%, iron-world 20% |
| Bromine sea | Throughout | 2.5% | ocean-world 12%, methane-world 11%, rogue-world 10% |
| Lithium brine | Throughout | 2.5% | methane-world 17%, ice-moon 8%, dry-world 7% |
| Germanium-gallium iron | Throughout | 6.3% | barren-world 27%, iron-world 26%, ice-moon 16% |
| Platinum-rich iron | Throughout | 6.2% | barren-world 28%, iron-world 22%, ocean-world 14% |
| Iodine ice | Throughout | 2.6% | methane-world 14%, rogue-world 10%, ocean-world 10% |
| Noble ices | Throughout | 2.6% | methane-world 16%, rogue-world 12%, ocean-world 11% |
| Thallium-selenium soil | Throughout | 0.6% | dry-world 6% |
| Gold mother lode | Jackpot | 1.9% | the asteroids 6% each |
| Platinum mother lode | Jackpot | 2.5% | the asteroids 6% each |
| Osmiridium hoard | Jackpot | 1.3% | the asteroids, about 3% each |
| Silver mountain | Jackpot | 2.4% | the asteroids 6% each |
| Uranium lode | Jackpot | 1.9% | the asteroids, about 4% each |
| Tellurium bonanza | Jackpot | 2.2% | the asteroids 6% each |
| Element caches (22) | Asteroid only | 1.6% each | every asteroid and comet, 1.3 caches on average |

## Steering the roll (meta tree hooks)

The roll is built to be steered: every body spec now carries a `bias`, neutral by default, that the meta tree can fill in when the player picks a body. Nothing is locked; the baseline, the catalogue and the odds are all data.

| Bias field | What it does | A perk it could back |
| --- | --- | --- |
| `element = { Pt = 3 }` | Specialties that bring an element (by what they enrich, boost or lode) are that many times likelier | “Platinum-group elements appear 200% more often on asteroids” |
| `specialty = { "Helium field" = 2 }` | One specialty by name, likelier | “Gas giants' moons: helium fields twice as common” |
| `jackpot = 1.5` | Jackpot chance times this | “Prospector's luck: +50% jackpots” |
| `extra = 1` | One more specialty rolled | “Deep survey: every body rolls one more specialty” |
| `off = true` | None (used to measure the plain generator) | — |

A test checks it works: steered 40× toward tungsten, at least half of 40 asteroids roll a tungsten cache, more than three times as many as unsteered.

## Drone parts, priced by rarity

Every base part can be built from common elements, but its ideal material is the rarest one that does the job, and each step toward the common ones costs more: 1 kg of part takes 1 + 0.5 × log10(how many times more common in Earth's crust) kg of the stand-in. The last resort is anything solid in the stockpile, 2 kg over the commonest choice, so no world ever stalls a build. Generated by `tools/recipes.py`.

| Part | Cutter kg | Hauler kg | Elements, rarest first (kg per kg of part) |
| --- | --- | --- | --- |
| Frame | 36 | 760 | Be 1, Ti 2.65, Mg 2.96, Fe 3.15, Al 3.25, anything 5.25 |
| Motors | 8 | 160 | Co 1, Ni 1.26, Fe 2.68, anything 4.68 |
| Wiring | 10 | 120 | Au 1, Ag 1.64, Cu 3.09, Al 4.66, anything 6.66 |
| Electronics | 6 | 60 | In 1, Ge 1.39, Ga 1.94, Si 4.03, anything 6.03 |
| Battery | 6 | 140 | Li 1, Zn 1.27, Mg 2.53, Na 2.63, anything 4.63 |
| Electrodes | 4 | 80 | Co 1, Ni 1.26, Mn 1.79, Fe 2.68, anything 4.68 |
| Cutting head | 4 | — | Mo 1, W 1.1, Ta 1.2, Cr 1.96, C 2.11, Ti 2.84, Fe 3.34, anything 5.34 |
| Composites | 8 | 280 | C 1, Al 2.31, Si 2.57, anything 4.57 |
| Bulk | 18 | 400 | anything, 1 |

A cutter built entirely from the commonest choices weighs about 300 kg of stockpile instead of 100, so a world rich in the rare elements builds about three times as many drones from the same haul.

## What changed, and what is exaggerated

Third pass: every element, not just gold, is back to natural levels in the plain generator, and the richness moved into specialties, asteroid caches and the bias the meta tree will steer.

- **Scarcity per ore.** Each ore now forms in only a share of the places it could (the rest is quartz gangue or host rock), tuned automatically by `tools/tune_scarcity.py` until ores bring every element to 0.5 to 3× Earth's crust on average. Platinum nuggets dropped to 1 in 100,000 of their old spots, gold to 2 in 1,000, silver and bismuth to under 1 in 100; lithium, beryllium, fluorite and other under-supplied ores were raised up to 30×.
- **Natural compositions.** Planet cores' platinum alloy went from 60% platinum to 0.3% (still richer than any rock, at roughly Earth-core levels); the noble gas in rock pockets is now mostly argon with a little helium; pitchblende carries only its natural trace of radium, so polonium joins the elements radioactivity will make. Sphalerite keeps cadmium and two traces (gallium, indium).
- **Asteroid caches and odds** (Specialties above), and the **bias** hook for the meta tree.
- **Earlier passes:** the full 75-element table, two traces per rock, specialties, rarity-priced drone parts.

Still away from 1×, and why:

- **High, by body type:** carbon, nitrogen, sulfur and chlorine (carbon, ice and sulfur worlds), nickel, cobalt, chromium and the platinum metals (meteoritic iron and cores, about what meteorites hold), lead (the hothouse world's lead frost, placed by hand).
- **Low, no ore of their own:** gallium, germanium, indium, iodine, rubidium, scandium, strontium and thallium are dispersed traces in nature, so with two traces per rock they mostly come from specialties.
- **Too thin to measure:** silver, selenium, tellurium, bismuth and ruthenium are present but under 0.01 ppm, about as rare as in Earth's crust.

Open questions:

- [ ] Perks that bend generation (the bias table above): which belong in the meta tree first?
- [ ] Planet ranking for selection: parked until the design settles.

## Sources

- [Abundance of elements in Earth's crust](https://en.wikipedia.org/wiki/Abundance_of_elements_in_Earth%27s_crust) (CRC Handbook, 97th edition values), copied into `data/reference/crust-ppm.toml`.
- Rock trace-element profiles follow the standard averages (Turekian and Wedepohl 1961 for igneous and sedimentary rocks, Rudnick and Gao 2003 for the crust, Lodders 2003 for CI chondrites, McDonough 2003 for the core), from memory and so approximate; their values are in `tools/catalogue.py`.
