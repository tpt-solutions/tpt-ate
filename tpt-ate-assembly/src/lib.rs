//! `tpt-ate-assembly` — equipment communication for physical assembly
//! (RFC-003, `spec.txt` Section 2B): wire bonding, flip-chip placement,
//! chiplet pick-and-place, guided by `tpt-silicon`'s interposer/chiplet
//! layout as the placement reference.
//!
//! Design intent (the `tpt-fab-litho` pluggable-backend pattern): assembly
//! technique is a swappable [`backend::AssemblyBackend`] behind one core —
//! the GEM execution flow ([`link::AssemblyHost`]) and the placement
//! verification ([`verify`]) are identical regardless of which physical
//! technique an OSAT runs. The equipment-communication core lives in
//! `tpt-ate-comm`, extracted from `tpt-ate-test` once this crate made the
//! duplication concrete (RFC-003 Section 5 decision, now resolved).

pub mod backend;
pub mod equipment;
pub mod layout;
pub mod link;
pub mod techniques;
pub mod verify;

pub use backend::{
    AssemblyBackend, AssemblyError, AssemblyTechnique, PlacementCommand, PlacementPlan,
    PlacementRecord,
};
pub use layout::{DieSpec, InterposerLayout, Orientation, PointNm, SiteSpec};
pub use verify::{verify_placements, PlacementReport, PlacementViolation};
