//! The ledger: where every kilogram of the body has gone.
//!
//! Physical mass is conserved, so for every material:
//!
//!   initial + reconciled + changed + deposited + grown - mined - vented - aloft = what the world says is still in the ground
//!
//! `initial` is the body's estimated content when the run starts. `reconciled`
//! records the small corrections made when a region is first generated cell by
//! cell and turns out to hold slightly more or less than its estimate said.
//! `changed` records matter that turned from one material into another in
//! place (lava cooling to rock): it leaves one material's column and enters
//! another's, so the total never moves.
//! `grown` is matter life takes from (positive) or gives back to (negative)
//! the air and water it lives on: plants growing, wilting and rotting.
//! Credited Mass (the currency) is physical kilograms times each material's
//! processing multiplier; the difference is booked as bonus, never as matter.

use crate::materials::MaterialTable;

#[derive(Debug, Clone, PartialEq)]
pub struct Ledger {
    /// Estimated kg per material in the whole body at the start.
    pub initial: Vec<f64>,
    /// Physical kg removed by the swarm, per material.
    pub mined: Vec<f64>,
    /// Corrections from estimate to exact, per material (positive = more than estimated).
    pub reconciled: Vec<f64>,
    /// kg that turned into (positive) or out of (negative) each material in place.
    pub changed: Vec<f64>,
    /// kg of gas that drifted off into space, per material.
    pub vented: Vec<f64>,
    /// kg currently flying as loose rigid bodies (out of the grid until they land).
    pub aloft: Vec<f64>,
    /// kg added from outside the body: wrecked drones left where they fell,
    /// and an open sea closing up behind water that drained out of it.
    pub deposited: Vec<f64>,
    /// kg life made (positive) or gave back to the air (negative), per material.
    pub grown: Vec<f64>,
    /// Currency: physical kg times processing multiplier.
    pub credited: f64,
    /// The part of `credited` that came from multipliers rather than matter.
    pub bonus: f64,
    /// kg in all regions generated cell by cell so far (what `reconciled` is measured against).
    pub generated: f64,
    /// Count of mining operations, for reports.
    pub cell_ops: u64,
    pub node_ops: u64,
}

impl Ledger {
    pub fn new(initial_kg: Vec<f64>) -> Self {
        let n = initial_kg.len();
        Ledger {
            initial: initial_kg,
            mined: vec![0.0; n],
            reconciled: vec![0.0; n],
            changed: vec![0.0; n],
            vented: vec![0.0; n],
            aloft: vec![0.0; n],
            deposited: vec![0.0; n],
            grown: vec![0.0; n],
            credited: 0.0,
            bonus: 0.0,
            generated: 0.0,
            cell_ops: 0,
            node_ops: 0,
        }
    }

    /// Book `cells` cells of material `m` as mined.
    pub fn mine(&mut self, mats: &MaterialTable, m: crate::materials::MatId, cells: f64) {
        let mat = mats.get(m);
        let kg = cells * mat.density;
        self.mined[m as usize] += kg;
        self.credited += kg * mat.multiplier;
        self.bonus += kg * (mat.multiplier - 1.0);
    }

    /// Book a correction of `cells` cells (either sign) found when a region was generated exactly.
    pub fn reconcile(&mut self, mats: &MaterialTable, m: crate::materials::MatId, cells: f64) {
        self.reconciled[m as usize] += cells * mats.get(m).density;
    }

    /// Book one cell turning from material `from` into `to` (same mass).
    pub fn change(&mut self, mats: &MaterialTable, from: crate::materials::MatId, to: crate::materials::MatId) {
        self.changed[from as usize] -= mats.get(from).density;
        self.changed[to as usize] += mats.get(to).density;
    }

    pub fn mined_kg(&self) -> f64 {
        self.mined.iter().sum()
    }

    /// Expected kg still in the ground, per material.
    pub fn expected_remaining(&self) -> Vec<f64> {
        (0..self.initial.len()).map(|i| self.initial[i] + self.reconciled[i] + self.changed[i] + self.deposited[i] + self.grown[i] - self.mined[i] - self.vented[i] - self.aloft[i]).collect()
    }

    /// Check the books against the world's own count of what remains.
    /// Returns the largest mismatch in kg (zero up to rounding when balanced).
    pub fn imbalance(&self, world_remaining_kg: &[f64]) -> f64 {
        self.expected_remaining()
            .iter()
            .zip(world_remaining_kg)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
    }
}
