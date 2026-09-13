//! `tpt-ate-comm` — the shared equipment-communication core for `tpt-ate`:
//! SECS/GEM (SEMI E5/E30/E37) built directly against the published standards,
//! plus the deterministic RNG used by the equipment simulators.
//!
//! Extracted from `tpt-ate-test` once Phase 2 (`tpt-ate-assembly`) made the
//! duplication concrete, per the RFC-003 Section 5 locked decision. Both
//! `tpt-ate-test` (automated test equipment) and `tpt-ate-assembly` (bonders,
//! placers) drive different equipment classes over this same layer; neither
//! shares it with `tpt-fab`.

pub mod rng;
pub mod secs;
