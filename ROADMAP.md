# The Dark Forest Theory: what's next

New session? Read HANDOFF.md first.

**Next up: production, step 2 of the skeleton** (plan agreed with Benjamin
10 Nov, see HANDOFF.md "Production plan"): machines placed along the mother
swarm as it wraps the planet, material flowing between them with
distance-based delays, and the swarm building its own machines (storage
ships, refiners, fabricators, factories and factory bays) block by block
from refined elements. Then the drawing (fleet arriving, the modular
factory with lasers, couriers drawn along the flows), then each hauler's
own landing spot. Energy (ground units beam it up), heat-sink bots and
tunnelers after. Premium units (magma diver, seer, beam relay, deep hauler,
core lance) come later through the per-run tech tree.

## M6, the native game (in progress)

- G1 window and planet: done.
- G2 live simulation, drones, speed keys, F1 dev panel: done.
- Optimization pass (O1 drones drawn cheaply, O3 work search, O4 detail
  only where you look): done. O2 (screen updates without waiting) deferred:
  the logs show it costs the simulation about 2%.
- G3 looks (textures, bevels, colour schemes, ghost backdrop, stars,
  atmosphere): done. The backdrop's parallax waits for a custom shader.
- G4 fog of war: done.
- G5 camera director, zoom limit, manual override: done.
- Falling chunks (orphan push, soft rock crumbling and shattering, hard rock
  rolling, gouging and crushing, weak strips snapping): done, shelved for
  tuning.
- **G6 saves and autosave, plus a PC benchmark: built, waiting on a PC test.**
  F5 saves, F9 loads the newest, the F1 panel lists and loads saves, autosave
  every 5 minutes and on closing (last three kept), continue-game.bat. The
  benchmark suite saves and loads at every size. Kept only roughly: drones
  mid-trip come home empty, rock in the air is lost.

- **Production skeleton, step 1: built, waiting on a PC test
  (test-barren.bat).** The swarm arrives as a fleet: the factory releases
  1,000 cutters, the storage ship brings ~97 t of refined elements and 6,000
  E ([production] in data/economy.toml), and there are no haulers. The
  fabricator (src/production.rs, the work in swarm.rs) makes parts from
  refined elements only, one kind at a time (3 s to switch, 1 t/s), the
  next hauler's parts first when a hauler bay is free, then the next
  cutter's, finishing each run (every order waiting and parts for 100
  cutters and 2 haulers kept ahead) before switching. The factory has 4
  cutter bays (0.5 s a cutter: ready as soon as the parts are) and 2 hauler
  bays (5 s); haulers and cutters are ordered (the director's orders queue)
  and built there, paying energy when a bay starts. Benjamin: units must
  reach the many thousands, so building is quick and the fabricator's parts
  are the limit. On the tutorial body the first hauler is out by 1 minute;
  10 minutes in: 1,902 cutters and 49 haulers built (2,380 cutters alive).
  F1
  shows a "production" section. Saves from before still load. The game
  always plays the real economy now: free drones are only for benchmarks
  (`--bench-drones`).
- **Production phase 1, parts: built, waiting on a PC test (test-barren.bat).**
  The design's ten parts (neural core, behaviour chip, servo, power cell,
  frame strut, armour plate, cutting head, hold module, heat sink, lift
  vane) are in the [recipe] tables of data/economy.toml, written by
  tools/recipes.py. Each part is a mix of components (wiring, electronics,
  frame...) priced by rarity as before; units are built from parts (a
  cutter is still 100 kg, a hauler 2 t; the design's part masses were
  first guesses). The swarm keeps parts ahead for 20 cutters and a hauler,
  made at once from the stockpile, and makes any part short on the spot
  (fabricators come in phase 2). The stockpile window's Parts tab shows
  each part on hand, made, per cutter and hauler, and what it is made of
  now; matter counts "made into parts". Saves from before still load
  (parts stock empty). Heat sinks are in the catalogue but nothing uses
  them yet.
- **E1 stockpile and recipes: built, waiting on a PC test (test-barren.bat).**
  Every element delivered goes into the mother swarm's stockpile (recipes.rs).
  Cutters (100 kg) and haulers (2 t) are built from parts (frame, motors,
  wiring, electronics, battery, electrodes, cutting head, composites, bulk),
  each made of the best element on hand, with stand-ins that take more mass,
  ending in aluminium or silicon so plain rock always builds. The swarm
  arrives with 1,000 cutters, builds only as many as the flocks have work
  for, has no cap, and opens a new flock when every flock is full and the
  stockpile can pay for its hauler and a first crew. F1 shows the stockpile
  and what cutters are being made of. Next: upgrades that add to the starting
  swarm; recipes for scouts, refiners and tunnelers; rare elements buying
  better drones (tungsten heads cut faster, copper wiring saves energy).
- **Stockpile window, finder, storage drones: built, waiting on a PC test.**
  A periodic table of the stockpile (its own window, starts collapsed, the
  round's score in its title: all matter the swarm holds). Clicking an
  element glides the view (zoom kept) to the nearest ground made with it and
  outlines the largest body of that material in view (others marked).
  The mother swarm thickens with the stockpile, and big storage robots (one
  per 20 kt held, 30 m long) drift in it. Next: storage drones as real units
  with a recipe and a capacity the stockpile can't exceed.
- **Eyedropper: built, waiting on a PC test.** Rest the mouse on the ground
  for two seconds: a card with the material, its elements, its worth, and how
  it behaves (hardness and cutting time, density, melting and boiling,
  temperature now). Uncharted ground under the fog says so.

- **Couriers, falling storage, blasts: built, waiting on a PC test.** What
  the swarm spends from its stockpile is carried by couriers from the newest
  storage drone into the swarm; that drone's hold empties from the top, and
  empty, it drops out of the sky and blows a crater for its size (F9 drops
  one, as a cheat). Every explosion now digs: the ground in its crater is
  thrown out and up as loose matter. Explosions are drawn in blocks: a
  white-hot core, fire cooling to smoke, a dust ring on the shock, falling
  embers, the swarm's lights scattering. Next: refining matter into
  elements, and factories that build drones (couriers will feed them).
- **Production phase 0, the full element table: built, waiting on a PC
  test.** 75 elements (everything to radium but the lanthanides, plus
  thorium and uranium). 33 new minerals carry them (pegmatites by granite,
  platinum-group minerals with chromite, pods of zircon, uranium, helium and
  radon gas, celestine, baryte, borax, bromine salt and iodine caliche
  anywhere their host rock is); each common rock carries the two trace
  elements it is known for. The stockpile shows the whole table: a slash for an
  element not on this body, three dots for one only in traces, and a Raw /
  Refined split (the swarm refines 50 t/s for now; refiner ships later).
  Measured abundance per body: `cargo run --release --bin abundance`
  writes data/reference/abundance.csv; the master table lives in the
  "Element Abundance Master Table" doc. Not done yet: warm ground around
  uranium and thorium (comes with radioactivity).
- **Planet specialties, priced recipes: built, waiting on a PC test.** Rock
  keeps at most two trace elements; each body instead rolls 0 to 3
  specialties from its seed (data/specialties.toml: rich veins, a trace
  element all through a common rock, districts rich in some ores and poor in
  others) and sometimes a jackpot lode (megatons of gold, platinum, silver,
  uranium...). The stockpile window names them ("This body:", hover for the
  real deposit it riffs on). Gold is down near the other precious metals.
  Drone parts list their elements rarest first; every step toward common
  ones costs more (tools/recipes.py), ending in "anything solid".
  `cargo run --release --bin abundance -- --odds 200` writes how often each
  specialty rolls per body type.
- **Natural baseline, asteroid pit stops, meta-tree bias: built, waiting
  on a PC test.** Every ore forms in only a share of the places it could
  ([geology.scarcity], tuned by tools/tune_scarcity.py from
  `abundance -- --plain`) so the plain generator holds each element at
  roughly Earth's-crust levels; cores, meteoritic iron and gas pockets have
  natural compositions. Richness comes from specialties: asteroids and
  comets roll more, often hold a jackpot, and alone roll 22 element caches
  (big pods of one ore). Each body spec has a `bias` (element and specialty
  weights, jackpot odds, extra rolls) for the meta tree to steer.

## Later phases

- **Map icons.** Zoomed out, drones are now plain grey specks whatever
  they're doing. Give the zoomed-out view proper minimap icons instead:
  flocks, haulers in flight, the mother swarm, storage, hot spots.

- **Boulder testing.** A test bed for falling chunks: scripted scenes (an
  overhang cut free, a slab on a slope, a soft block dropped from height, a
  boulder rolling through a working flock, a chunk hanging by a one-cell
  strip) on bodies from the rubble asteroid to Earth size, run by script with
  the camera on them, logged (pieces, drones crushed, ground gouged, how far
  things went, time to rest, cost) so the chaos can be tuned by numbers.
- **Drone behaviour, rebuilt (in progress).** Cutters and haulers, no
  steering, reckless drones. Scouts, Refiners and Tunnelers are parked until
  the economy pass.
  - D1 cutters and haulers: done, waiting on a PC look. Cutters cut cell
    after cell into a hold (denser rock fills it sooner), burn part of their
    cargo for their own energy and are lost if they run dry, and unload into
    the hauler over their flock. Haulers fill visibly, lift straight up to
    the mother swarm when full (or after 10 minutes), unload and come back.
    More haulers are sent where cutters stand waiting; broken ones are
    replaced.
  - D2 the mother swarm and the flock AI: done, waiting on a PC look. The
    mother swarm arrives over one spot and grows into a single long cloud
    around the body from a share of what is delivered, on a circle 400 m
    above the body's highest ground; haulers unload at its nearest point.
    Flocks follow their work, split to cover new ground, merge when out of
    work or on top of each other, and send cutters to a
    neighbour on hard going. Spare and broken haulers blow up, scattering
    their scrap nearby; worn-out cutters just shut down and fall as scrap.
  - D3 the swirling cloud up close: done (drawing only). Cutters waiting
    at their flock circle the hauler in a loose cloud, weave in flight, and
    hover and dart about the cell they cut; a slow shared flow makes nearby
    drones drift together. Off when drones are too small to see apart.
  - D4 out-of-sight shortcuts and camera events: done, waiting on the PC
    benchmark. Out of sight, the search for big chunks of rock cut free
    gets a tenth of the effort and skips rock hanging by a thread (rock
    checks went from half the simulation's time to about a fifth). The
    swarm reports what happens (a hauler lifting off full, a new flock,
    flocks merging, explosions, breaches into caves, lava or seas, rich
    ore, drones lost) and the camera director goes to look; F1 shows why.
    New cutters drop from the mother swarm in 20 s at most.
    After the PC benchmark: falling rock out of sight lands at once instead
    of tumbling; scrap and worn-out cutters drop to the ground (and can be
    eaten again) instead of hanging in the air; flocks left without cutters
    end and their haulers retire; the mother swarm finds the true highest
    point; the camera only counts what happens where drones are at work, and
    cuts instead of sweeping across a long way; zooming in no longer blinks
    (a piece of the coarser tile shows until the finer one arrives); drones
    on screen keep moving through slow simulation steps; big releases of
    waiting drones are spread over a few seconds.
    After the second look: the half-second hitch was the big rock-check
    (every 10 game seconds, 70-100 ms); it now runs a little every step, as
    do the small rock check and the mother swarm's height sweep (a sixtieth
    of the way round each second). The log records the longest step each
    second. The backdrop crumbles where the ground above it is eaten (only
    tunnels and caves under standing ground keep their ghost); out-of-date
    coarse tiles are no longer used as stand-ins; tiles are a level sharper
    for the zoom. The pan shot is out of the benchmark plans.
  - The mother swarm comes down every minute to stay 400 m over the highest
    ground left; the mass it no longer needs (and its energy) goes back to
    the swarm's balance, for the upgrade tree to use.
- **Heat out of sight.** Heat on active bodies keeps every hot cell awake
  forever; out of sight it could step less often in bigger steps.
- **Physics batch (in progress).**
  - P1 drone heat and liquids: done, waiting on a PC look. Every drone has
    a heat capacity and a heat sink. Cutters drink lava and molten metal
    in sips that freeze inside them; the heat goes into the drone, and if
    it passes its melting point it melts (otherwise it wears while hot).
    Working beside lava or glowing rock heats it the same way (this
    replaced the flat chance of being wrecked by heat). A hot drone rests
    by its hauler to cool; short of melting point it carries on, reckless.
    Cold ground makes drones cut faster and wear quicker. Every liquid is
    eaten by mass, seas included.
  - P2 burning: done, waiting on a PC look. data/burning.toml says what
    burns: how hot it must get to catch, its flame, how fast it burns
    away. It needs air (open space on a body whose air holds oxygen: the
    living world and earth-test) or a touching oxidizer (liquid oxygen,
    oxygen ice, liquid air, air snow). Lava, glowing rock, burning
    neighbours and explosions light it; it creeps through what burns; a
    fire smothered by fallen soil goes out. Burned cells go off as smoke.
  - P3 freezing: checked, works as heat flows. Very cold ice freezes water
    against it; plenty of warmer water melts a little ice. Fixed on the
    way: near the surface (top 100 m) nothing holds melting points up, so
    surface ice on a warm world melts (before, it only melted 100 K past
    the ground's own heat).
- Life L1 (plants, living ground), then the economy pass, then M7, then
  Life L2 with civilizations. Design: the "Life L1 - Procedural Flora
  Design" doc. Benjamin's answers: seed pods are for show (pods that reach
  the mother swarm add their mass); defences are harsh and only count if
  they damage drones (caustic gas, vine whips, carnivorous traps);
  species don't matter to the player (all biomass and its elements);
  flora gets idle and attack animations unless rigid.
  - L1a-c done: plant materials and burning (data/life.toml), the
    chemistry survey and niches, species breeding by glamour, the
    growing-tip engine (ten body plans, space-colonisation crowns), and
    placement in 128 m chunks over the generator. Bodies opt in with
    `life` (none do yet, so the game is unchanged). Tool:
    run-floragen.bat (report, species gallery, scenes for any body,
    seed, size, glamour and life). Flora viewer: run-floraview.bat, a live
    window to browse one plant at a time from any world and seed (wheel or
    arrows), regrow the same species, play idle motion and attacks.
  - L1d done: flora is on in the game on every body whose chemistry
    allows it. Plants are joined edge to edge to their ground (cut a
    trunk and the tree falls as a loose body). Chunks only look where
    life can be: under the ground's skin, on sea floors, in caves found by
    a sparse probe (about 3 ms a surface chunk, from 20). Tiles coarser
    than 16 m a texel leave plants out. Sim cost off-screen +14%, memory
    about +50 MB on the living world.
  - Next: L1e life cycle, defences and animation (the plant draw layer
    near the camera), L1f doom reactions. Later: bound the flora caches,
    a canopy tint for far-out tiles.
  - Economy pass, Benjamin's sketch: two tech trees.
    - The base tree is unlocked step by step on every planet: from a base,
      branching out hub-and-spoke. Each upgrade has a total mass
      requirement and unlocks when it is reached. Mass is shared equally
      among all available nodes; when they unlock, the next set opens.
    - Prestiging or finishing a planet saves a portion of the take for the
      next planet, so its first few levels unlock sooner.
    - The meta tree unlocks branches (spokes) of the base tree, and gives
      bonuses such as better material values.
    - Between runs: what's left in the swarm at launch or core collection
      goes into a grand total in space, by element. Meta upgrades cost
      cores and large amounts of specific elements. Picking the next
      planet sends a small share down as seed stock to fast-unlock part of
      the per-run tree; meta upgrades raise that share.
    - Planet selection will be steered by the meta tree through each body's
      `bias` (e.g. "these elements appear x% more often on body y");
      ranking bodies for selection is parked until Benjamin settles it.
- **Backdrop behind eaten ground (Benjamin's call whether to revisit).** The
  ghost of eaten ground is switched off (`GHOST` in game/src/tiles.rs): it
  didn't match the ground's sharpness, so eaten ground shows the starfield.
  Options when revisited: a ghost drawn at the same detail as the ground
  in front of it, a parallax backdrop with its own shader, or keep stars.
- **Unit beautification and graphics.** Once the units' behaviour is nailed
  down: a pass on how cutters, haulers, the mother swarm and any later units
  look, with flavour (shapes, animation, trails, damage, death and
  explosion effects). Separate from the interface design and from planet
  refinement.
- The game's own interface, designed from scratch (only the F1 dev panel
  uses a stock look).
