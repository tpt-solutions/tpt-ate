//! The `AssemblyBackend` trait — one core, swappable technique
//! implementations (wire bonding, flip-chip, chiplet pick-and-place),
//! mirroring `tpt-fab-litho`'s pluggable-backend pattern from RFC-002.
//!
//! The core (this trait, the plan/report types, and
//! [`crate::verify`]) carries no equipment-technique assumptions:
//! everything technique-specific lives behind a backend.

use crate::layout::{Orientation, PointNm};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The physical assembly techniques this crate can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssemblyTechnique {
    WireBond,
    FlipChip,
    ChipletPickPlace,
}

impl AssemblyTechnique {
    /// The GEM remote-command label the corresponding equipment class speaks.
    pub fn rcmd(self) -> &'static str {
        match self {
            AssemblyTechnique::WireBond => "PLACE_WB",
            AssemblyTechnique::FlipChip => "PLACE_FC",
            AssemblyTechnique::ChipletPickPlace => "PLACE_PP",
        }
    }
}

/// One placement command for the equipment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacementCommand {
    pub site_id: String,
    pub chiplet_id: String,
    /// Nominal bottom-left corner, nm.
    pub target: PointNm,
    pub orientation: Orientation,
}

/// A planned assembly run: the ordered commands the backend executes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacementPlan {
    pub plan_id: String,
    pub commands: Vec<PlacementCommand>,
}

/// What the equipment reports back for one executed command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacementRecord {
    pub site_id: String,
    pub chiplet_id: String,
    pub technique: AssemblyTechnique,
    pub nominal: PointNm,
    /// As-placed bottom-left corner reported by the equipment, nm.
    pub placed: PointNm,
    pub orientation: Orientation,
}

/// Errors from driving assembly equipment.
#[derive(Debug, Error)]
pub enum AssemblyError {
    #[error("equipment communication error: {0}")]
    Comm(#[from] tpt_ate_comm::secs::SecsError),

    #[error("plan {plan_id} is empty")]
    EmptyPlan { plan_id: String },

    #[error("equipment rejected command for site {site_id} (HCACK {hcack})")]
    CommandRejected { site_id: String, hcack: u8 },

    #[error("placement event for site {site_id} missing variable {name}")]
    MalformedEvent { site_id: String, name: String },
}

/// The pluggable backend contract: one [`AssemblyTechnique`], one
/// implementation. Cores never branch on technique — they dispatch through
/// this trait (the `tpt-fab-litho` pattern).
pub trait AssemblyBackend {
    fn technique(&self) -> AssemblyTechnique;

    /// Execute the whole plan against the backend's equipment connection
    /// and return one record per executed command, in execution order.
    fn place(&mut self, plan: &PlacementPlan) -> Result<Vec<PlacementRecord>, AssemblyError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn technique_rcmds_are_distinct() {
        let all = [
            AssemblyTechnique::WireBond,
            AssemblyTechnique::FlipChip,
            AssemblyTechnique::ChipletPickPlace,
        ];
        let rcmds: Vec<_> = all.iter().map(|t| t.rcmd()).collect();
        for (i, a) in rcmds.iter().enumerate() {
            for b in &rcmds[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
