//! Economy numbers, loaded from data/economy.toml.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SeedSwarm {
    pub start_harvesters: u32,
    pub start_energy: f64,
    pub base_power: f64,
    pub free_build_every_s: f64,
    pub processing_energy_per_tonne: f64,
    pub hover_m: f64,
    pub hub_ring: u32,
    /// Seconds for the seed swarm to orbit the body once.
    pub orbit_period_s: f64,
    /// Metres of flight a drone will accept to take a cell one metre higher up,
    /// so the swarm peels the body from the outside in.
    pub peel: f64,
    /// Bodies whose circumference is at most this are surveyed whole from
    /// orbit at the start; bigger ones are opened up one landing zone at a time.
    #[serde(default = "full_survey")]
    pub full_survey_m: f64,
    /// Half-width (along the surface) of the ground opened at each landing point.
    #[serde(default = "zone")]
    pub zone_m: f64,
    /// The mother swarm: its length on arrival (metres), the mass it takes
    /// to grow a metre longer, and the share of delivered mass put into it.
    #[serde(default = "mother_start")]
    pub mother_start_m: f64,
    #[serde(default = "mother_kg_per_m")]
    pub mother_kg_per_m: f64,
    #[serde(default = "mother_share")]
    pub mother_growth_share: f64,
}

fn mother_start() -> f64 {
    400.0
}

fn mother_kg_per_m() -> f64 {
    20_000.0
}

fn mother_share() -> f64 {
    0.25
}

fn full_survey() -> f64 {
    25_000.0
}

fn zone() -> f64 {
    500.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct Harvester {
    pub mass_cost: f64,
    pub energy_cost: f64,
    pub energy_cost_growth: f64,
    /// kg carried home per trip; a cell takes several trips.
    pub cargo_kg: f64,
    pub upkeep: f64,
    pub speed: f64,
    /// Seconds to cut one load of hardness-3 rock.
    pub dig_s: f64,
    /// Each point of hardness above (or below) 3 multiplies the dig time by this.
    pub hardness_step: f64,
    pub life_s: f64,
    pub search_m: f64,
    /// kg a cutter's hold takes before it turns back to its hauler (denser
    /// rock fills it in fewer cells).
    #[serde(default = "hold")]
    pub hold_kg: f64,
    /// Energy a cutter carries for itself (E). It burns part of its cargo to
    /// keep this topped up, and is lost if it runs dry.
    #[serde(default = "tank")]
    pub tank_e: f64,
    /// How far a cutter looks from the cell it just cut for its next one
    /// before it turns back with a part-full hold (metres).
    #[serde(default = "chain")]
    pub chain_m: f64,
    /// Heat: what a cutter can soak up (kg of rock it is worth, warming
    /// alike), how fast its heat sink sheds heat (kg-worth per second, per
    /// kelvin of difference; faster pressed against hot or cold ground),
    /// where heat starts to wear it, and where it melts (kelvin).
    #[serde(default = "heat_capacity")]
    pub heat_capacity_kg: f64,
    #[serde(default = "heat_sink")]
    pub heat_sink: f64,
    #[serde(default = "contact_sink")]
    pub contact_sink: f64,
    #[serde(default = "damage_k")]
    pub damage_k: f64,
    #[serde(default = "melt_k")]
    pub melt_k: f64,
    /// Seconds of life lost per second, per 100 K over damage_k.
    #[serde(default = "heat_wear")]
    pub heat_wear: f64,
    /// Most of a molten cell (hotter than damage_k) taken in one sip (kg).
    #[serde(default = "sip")]
    pub sip_kg: f64,
    /// Cold: below cold_ref_k a cutter cuts faster and wears quicker, by up
    /// to cold_gain more at absolute zero.
    #[serde(default = "cold_ref")]
    pub cold_ref_k: f64,
    #[serde(default = "cold_gain")]
    pub cold_gain: f64,
}

fn heat_capacity() -> f64 {
    500.0
}

fn heat_sink() -> f64 {
    25.0
}

fn contact_sink() -> f64 {
    20.0
}

fn damage_k() -> f64 {
    500.0
}

fn melt_k() -> f64 {
    1300.0
}

fn heat_wear() -> f64 {
    1.0
}

fn sip() -> f64 {
    300.0
}

fn cold_ref() -> f64 {
    250.0
}

fn cold_gain() -> f64 {
    0.6
}

fn hold() -> f64 {
    10_000.0
}

fn tank() -> f64 {
    10.0
}

fn chain() -> f64 {
    24.0
}

/// Haulers: big drones that hover over a working flock, fill up with what
/// the cutters bring them, and carry it up to the mother swarm.
#[derive(Debug, Clone, Deserialize)]
pub struct HaulerSpec {
    /// kg it takes before it lifts off.
    pub capacity_kg: f64,
    /// It lifts off part-full after this long at its flock (seconds).
    pub max_stay_s: f64,
    pub speed: f64,
    /// Seconds to unload at the mother swarm.
    pub unload_s: f64,
    /// Seconds of service before it breaks down.
    pub life_s: f64,
    pub mass_cost: f64,
    pub energy_cost: f64,
    /// At most this many haulers serve one flock.
    pub max_per_flock: u32,
    /// Another hauler is sent for when this many cutters wait at a flock
    /// with nowhere to unload (or this share of the flock, if more).
    pub wait_trigger: u32,
    pub wait_share: f64,
    /// The mother swarm's height above the highest ground on the body (metres).
    pub mother_alt_m: f64,
}

impl Default for HaulerSpec {
    fn default() -> Self {
        HaulerSpec { capacity_kg: 1_000_000.0, max_stay_s: 600.0, speed: 40.0, unload_s: 30.0, life_s: 3600.0, mass_cost: 2000.0, energy_cost: 1000.0, max_per_flock: 4, wait_trigger: 6, wait_share: 0.1, mother_alt_m: 400.0 }
    }
}

/// How flocks decide among themselves (see Sim::flock_work).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FlockSpec {
    /// How long a flock remembers what it did (seconds).
    pub memory_s: f64,
    /// Share of the way its hauler drifts toward the work each 5 s.
    pub follow: f64,
    /// How far out from the face the hauler hovers (metres).
    pub hover_m: f64,
    /// A flock this big splits; the new one goes this far along the surface.
    pub split_cutters: u32,
    pub split_m: f64,
    /// Flocks this close merge; flocks talk to others this close.
    pub merge_m: f64,
    pub comm_m: f64,
    /// Searches coming up empty (decaying count) for a flock to give up and merge.
    pub dry_merge: f64,
    /// Hard going: this many times slower per tonne than a neighbour.
    pub hard_factor: f64,
    /// Share of a neighbour's cutters sent to help.
    pub help_share: f64,
    /// A flock waits this long after a split, merge or help before the next.
    pub calm_s: f64,
    /// A spare hauler standing empty this long blows up.
    pub spare_hauler_s: f64,
}

impl Default for FlockSpec {
    fn default() -> Self {
        FlockSpec { memory_s: 60.0, follow: 0.3, hover_m: 8.0, split_cutters: 300, split_m: 200.0, merge_m: 40.0, comm_m: 500.0, dry_merge: 3.0, hard_factor: 2.0, help_share: 0.2, calm_s: 60.0, spare_hauler_s: 120.0 }
    }
}

impl Harvester {
    /// Seconds to cut one load of rock of this hardness (liquids count as 1).
    pub fn dig_s_for<H: Into<f64>>(&self, hardness: H) -> f64 {
        let h: f64 = hardness.into();
        self.dig_s * crate::noise::powi(self.hardness_step, (h.max(1.0) - 3.0).round() as i32)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scout {
    pub unlock_cost: f64,
    pub mass_cost: f64,
    pub energy_cost: f64,
    pub upkeep: f64,
    pub per_harvesters: f64,
    pub search_bonus_m: f64,
}

/// Tunnelers: big, wide boring machines that sink shafts straight down, opening
/// the way for Harvesters. Their drill is strong but hungry, and most of what
/// it cuts is crushed to fines, so they cost far more than they earn.
#[derive(Debug, Clone, Deserialize)]
pub struct Tunneler {
    pub unlock_cost: f64,
    pub mass_cost: f64,
    pub energy_cost: f64,
    /// E per second while boring.
    pub upkeep: f64,
    /// The swarm keeps about one Tunneler per this many Harvesters...
    pub per_harvesters: f64,
    /// ...and never more than this many.
    pub max_count: u32,
    /// Width of the shaft, in metres.
    pub bore_m: f64,
    /// Cuts rock this many times faster than a Harvester.
    pub drill_power: f64,
    /// E burned per tonne cut (processing only gives back seed_swarm.processing_energy_per_tonne).
    pub energy_per_tonne: f64,
    /// Share of the cut rock recovered as Mass; the rest is crushed to waste.
    pub spoil_recovery: f64,
    /// Rock this close to the shaft wall is cracked by the boring...
    pub fracture_m: f64,
    /// ...and Harvesters cut it in this share of the usual time.
    pub fracture_dig: f64,
    /// Longest shaft, in metres.
    pub max_length_m: f64,
    /// Never bores deeper than this share of the body's radius.
    pub min_radius: f64,
    /// Furthest the shaft may lean from straight down, in degrees.
    pub max_deviation_deg: f64,
    /// How fast it can turn, in degrees per metre bored: unwieldy.
    pub turn_deg_per_m: f64,
    /// How far ahead it looks when choosing where to aim, in metres.
    pub aim_depth_m: f64,
    /// A staging point for Harvesters every this many metres down a shaft.
    pub hub_every_m: f64,
    /// Seconds to pull out and move to a new spot after a shaft is done.
    pub relocate_s: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Refiner {
    pub unlock_cost: f64,
    pub mass_cost: f64,
    pub energy_cost: f64,
    pub upkeep: f64,
    pub energy_bonus: f64,
    pub min_haul_m: f64,
    pub spacing_m: f64,
    /// Kilograms of raw stockpile the mother swarm refines into pure
    /// elements each second (until refiner ships take over).
    #[serde(default = "swarm_refine")]
    pub swarm_kg_per_s: f64,
}

fn swarm_refine() -> f64 {
    50000.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct Launch {
    pub threshold: f64,
    pub seed_mass_k: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Upgrade {
    pub name: String,
    pub base_cost: f64,
    pub cost_growth: f64,
    /// Each level multiplies the ability by this.
    pub effect: f64,
}

/// The scripted idle player: checks in now and then and buys a little.
#[derive(Debug, Clone, Deserialize)]
pub struct Player {
    pub check_in_every_s: f64,
    pub buys_per_check_in: u32,
}

/// A permanent starting advantage bought with Seed Mass between bodies.
#[derive(Debug, Clone, Deserialize)]
pub struct Perk {
    pub name: String,
    pub cost: f64,
    #[serde(default = "one")]
    pub cargo: f64,
    #[serde(default = "one")]
    pub speed: f64,
    #[serde(default = "one")]
    pub dig: f64,
    #[serde(default = "one")]
    pub energy_cost: f64,
    #[serde(default)]
    pub start_extra: u32,
}

fn one() -> f64 {
    1.0
}

/// How well drones judge what to dig. Each level of the "swarm intelligence"
/// upgrade reads the next entry: 0 = blind (every cell looks average, so
/// drones take the nearest), 1 = sees the truth exactly.
#[derive(Debug, Clone, Deserialize)]
pub struct Intelligence {
    /// How well drones see the effort a cell takes (hardness, kg per load), by level.
    pub effort: Vec<f64>,
    /// How well drones see what a cell is worth (its processing multiplier), by level.
    pub value: Vec<f64>,
    /// With Scouts out, the swarm chooses as if this many levels smarter.
    pub scout_bonus_levels: u32,
    /// A drone that judges effort turns down cells paying less than this share
    /// of what ordinary rock would (scaled by how well it judges).
    pub patience: f64,
    /// How much the smell of richer rock behind a cell adds to its worth.
    pub scent_weight: f64,
    /// What a blind drone assumes every cell is like.
    pub typical_hardness: f64,
    pub typical_load_kg: f64,
}

/// Liquids: never dug like rock. An exposed face cools into solid over time
/// (see each material's cools_to and cool_s).
#[derive(Debug, Clone, Deserialize)]
pub struct Lava {
    /// A drone may swallow a lava cell only at the tip of a pocket: at most this
    /// many of its four neighbours are also liquid.
    pub max_liquid_neighbours: u32,
    /// Seconds of drone life burned by swallowing one load of lava.
    pub life_cost_s: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Economy {
    pub tick_s: f64,
    pub individual_cap: u32,
    pub seed_swarm: SeedSwarm,
    pub harvester: Harvester,
    #[serde(default)]
    pub hauler: HaulerSpec,
    #[serde(default)]
    pub flock: FlockSpec,
    pub scout: Scout,
    pub refiner: Refiner,
    pub tunneler: Tunneler,
    pub launch: Launch,
    pub player: Player,
    pub upgrade: Vec<Upgrade>,
    #[serde(default)]
    pub perk: Vec<Perk>,
    pub intelligence: Intelligence,
    pub lava: Lava,
    /// The parts catalogue, and what cutters and haulers are made of (with
    /// the economy on).
    #[serde(default)]
    pub recipe: crate::recipes::Recipes,
    /// The fleet's starting allowance, the fabricator and the factory.
    #[serde(default)]
    pub production: crate::production::ProductionSpec,
    /// The Universal Drone Replicator and the specialists it builds.
    #[serde(default)]
    pub units: UnitsSpec,
}

/// `[units]`: the Universal Drone Replicator, a testing stand-in for the
/// meta tree: bays that build every specialist this body has a use for
/// (sappers and tunnelers anywhere, heat-sink bots where there is magma,
/// foresters where there are plants, abductors where there are animals).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UnitsSpec {
    pub universal_replicator: bool,
    /// Its bays, one kind of specialist each (bays for a kind the body has
    /// no use for are left out).
    pub bay: Vec<crate::production::BaySpec>,
    pub sapper: SapperSpec,
    pub tunneler: TunnelerUnitSpec,
    pub heatsink: HeatSinkSpec,
    pub forester: ForesterSpec,
    pub abductor: AbductorSpec,
}

/// A sapper: small and sacrificial, carrying a small nuclear charge. It
/// drops to where the going is hard, crawls down cracks and caves (or drills
/// where there are none), arms and goes off deep in the rock.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct SapperSpec {
    pub energy_cost: f64,
    /// Sappers out at once.
    pub keep: u32,
    /// Its yield (kilotonnes), and the blast's radius for 1 kt (m; it grows
    /// with the cube root of the yield).
    pub yield_kt: f64,
    pub radius_m_per_cbrt_kt: f64,
    /// It wants to go off this far under the ground beyond its blast radius.
    pub bury_m: f64,
    /// Farthest it crawls through cracks and caves (m of path), and drills
    /// down where there is no way deep enough (m).
    pub crawl_m: f64,
    pub drill_m: f64,
    /// Speeds (m/s): dropping, crawling, drilling.
    pub fly_speed: f64,
    pub crawl_speed: f64,
    pub drill_speed: f64,
    /// Seconds armed before it goes off.
    pub arm_s: f64,
    /// Share of what it drills through that is kept (the rest is spoil).
    pub spoil_recovery: f64,
}

/// A tunneler as a ground unit: the arthropod drill, much wider than the
/// old Tunneler's shaft. Its other numbers are the old Tunneler's.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct TunnelerUnitSpec {
    pub energy_cost: f64,
    /// About one per this many cutters, at least one, at most `keep`.
    pub per_cutters: f64,
    pub keep: u32,
    pub upkeep: f64,
    pub bore_m: f64,
    pub drill_power: f64,
    pub energy_per_tonne: f64,
    pub spoil_recovery: f64,
    pub max_length_m: f64,
    pub hub_every_m: f64,
}

/// A heat-sink bot: hovers over magma, growing cooling tentacles down
/// through it, freezing it to stone; the heat becomes power for the swarm.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct HeatSinkSpec {
    pub energy_cost: f64,
    /// At most this many at once (no more than there are pools of magma).
    pub keep: u32,
    pub speed: f64,
    /// Seconds over magma before it burns out.
    pub life_s: f64,
    /// Its tentacles' reach (m): growing this fast, up to this far.
    pub grow_m_per_s: f64,
    pub reach_m: f64,
    /// Cells it freezes a second.
    pub freeze_per_s: u32,
    /// Power it beams up per tonne frozen (E).
    pub power_per_tonne: f64,
    /// Only liquids that freeze at or above this are worth it (magma, molten
    /// metal and sulfur; not water or methane).
    pub min_freeze_k: f64,
}

/// A forester: a crab on three legs. It strides to the thickest plant life,
/// shears trees into its clippings bag and burns the ground cover with its
/// laser for the cutters to collect.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ForesterSpec {
    pub energy_cost: f64,
    pub keep: u32,
    pub life_s: f64,
    /// Speeds (m/s): striding to the next stand of plants, creeping while it works.
    pub stride_speed: f64,
    pub work_speed: f64,
    /// Its claws' reach (m), and seconds per cell cut.
    pub reach_m: f64,
    pub cut_s: f64,
    /// It sets the ground cover alight this often (s), this far out (m).
    pub burn_every_s: f64,
    pub laser_m: f64,
    /// Its bag holds this much (kg) before couriers carry it up.
    pub bag_kg: f64,
}

/// An abductor: a spider on three legs that snatches live animals. No body
/// has animals yet, so none is ever built.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AbductorSpec {
    pub energy_cost: f64,
    pub keep: u32,
}

impl Economy {
    pub fn parse(text: &str) -> Result<Self, String> {
        let e: Economy = toml::from_str(text).map_err(|e| e.to_string())?;
        e.recipe.check()?;
        Ok(e)
    }
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&text)
    }
}
