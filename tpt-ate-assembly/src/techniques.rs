//! The concrete assembly-technique backends. Each one is a thin
//! configuration wrapper: technique identity, its process parameters, and
//! the tolerance it guarantees — the placement execution flow itself
//! (GEM host command → acknowledge → placement event) is shared in
//! [`crate::link::AssemblyHost`], exactly the pluggable-backend shape
//! RFC-002 established for `tpt-fab-litho`.

use crate::backend::{
    AssemblyBackend, AssemblyError, AssemblyTechnique, PlacementPlan, PlacementRecord,
};
use crate::equipment::PLACEMENT_JITTER_NM;
use crate::link::{AssemblyHost, SimStream};

/// Wire bonding backend: die/substrate placement plus bond-wire process
/// parameters.
pub struct WireBondBackend {
    host: AssemblyHost<SimStream>,
    /// Bond wire pitch, µm.
    pub wire_pitch_um: f32,
    /// Longest wire the bonder can form, µm.
    pub max_wire_length_um: f32,
}

impl WireBondBackend {
    /// Take over an already-connected equipment link.
    pub fn new(host: AssemblyHost<SimStream>, wire_pitch_um: f32, max_wire_length_um: f32) -> Self {
        WireBondBackend { host, wire_pitch_um, max_wire_length_um }
    }
}

impl AssemblyBackend for WireBondBackend {
    fn technique(&self) -> AssemblyTechnique {
        AssemblyTechnique::WireBond
    }

    fn place(&mut self, plan: &PlacementPlan) -> Result<Vec<PlacementRecord>, AssemblyError> {
        execute_plan(&mut self.host, AssemblyTechnique::WireBond, plan)
    }
}

/// Flip-chip placement backend: bump-reflow attachment parameters.
pub struct FlipChipBackend {
    host: AssemblyHost<SimStream>,
    /// Bump pitch, µm.
    pub bump_pitch_um: f32,
    /// Alignment accuracy the tool guarantees, µm.
    pub alignment_tolerance_um: f32,
}

impl FlipChipBackend {
    pub fn new(
        host: AssemblyHost<SimStream>,
        bump_pitch_um: f32,
        alignment_tolerance_um: f32,
    ) -> Self {
        FlipChipBackend { host, bump_pitch_um, alignment_tolerance_um }
    }
}

impl AssemblyBackend for FlipChipBackend {
    fn technique(&self) -> AssemblyTechnique {
        AssemblyTechnique::FlipChip
    }

    fn place(&mut self, plan: &PlacementPlan) -> Result<Vec<PlacementRecord>, AssemblyError> {
        execute_plan(&mut self.host, AssemblyTechnique::FlipChip, plan)
    }
}

/// Chiplet pick-and-place backend: vacuum-nozzle pick of known-good dies
/// onto interposer sites.
pub struct ChipletPickPlaceBackend {
    host: AssemblyHost<SimStream>,
    /// Nozzle diameter, µm (pick tooling).
    pub nozzle_diameter_um: f32,
    /// Placement accuracy the tool guarantees, µm.
    pub placement_tolerance_um: f32,
}

impl ChipletPickPlaceBackend {
    pub fn new(
        host: AssemblyHost<SimStream>,
        nozzle_diameter_um: f32,
        placement_tolerance_um: f32,
    ) -> Self {
        ChipletPickPlaceBackend { host, nozzle_diameter_um, placement_tolerance_um }
    }
}

impl AssemblyBackend for ChipletPickPlaceBackend {
    fn technique(&self) -> AssemblyTechnique {
        AssemblyTechnique::ChipletPickPlace
    }

    fn place(&mut self, plan: &PlacementPlan) -> Result<Vec<PlacementRecord>, AssemblyError> {
        execute_plan(&mut self.host, AssemblyTechnique::ChipletPickPlace, plan)
    }
}

/// The shared execution flow every backend runs: one host command per plan
/// entry, in order, collecting equipment-reported positions. This is the
/// whole "core" — it never branches on technique.
fn execute_plan(
    host: &mut AssemblyHost<SimStream>,
    technique: AssemblyTechnique,
    plan: &PlacementPlan,
) -> Result<Vec<PlacementRecord>, AssemblyError> {
    let rcmd = technique.rcmd();
    let mut records = Vec::with_capacity(plan.commands.len());
    for command in &plan.commands {
        let placed = host.place(rcmd, command)?;
        records.push(PlacementRecord {
            site_id: command.site_id.clone(),
            chiplet_id: command.chiplet_id.clone(),
            technique,
            nominal: command.target,
            placed,
            orientation: command.orientation,
        });
    }
    Ok(records)
}

/// The placement tolerance the simulated equipment actually exhibits
/// (jitter half-window, Manhattan worst case ≈ 2× per-axis jitter) — used
/// by tests and callers to size the verification tolerance.
pub fn simulated_equipment_tolerance_nm() -> i64 {
    (PLACEMENT_JITTER_NM * 2.0 * 2.0) as i64
}
