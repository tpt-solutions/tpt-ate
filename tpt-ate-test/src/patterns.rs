//! The test-program input `tpt-ate-test` consumes and executes: what
//! `tpt-silicon`'s ATPG/DFT flow will hand over once those crates exist
//! (RFC-003 Phase 1 calls this a cross-repo dependency; none of it is
//! implemented in `tpt-silicon` yet).
//!
//! The shape here carries exactly what bin sorting needs — test numbers,
//! names, units, limits, pattern-group sizes, and the per-test soft-bin
//! policy — and is intentionally a placeholder until the real handoff format
//! lands in `tpt-silicon`. It deliberately carries no test *authoring*
//! semantics (fault targeting, pattern compaction): `tpt-ate-test` executes
//! what it is given.

use serde::{Deserialize, Serialize};

/// One parametric test in the program (executes as a STDF PTR).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestSpec {
    pub test_number: u32,
    pub name: String,
    pub unit: String,
    /// Expected value the simulator measures around.
    pub nominal: f32,
    pub lo_limit: Option<f32>,
    pub hi_limit: Option<f32>,
    /// Software bin assigned when this test fails (0 → taxonomy default).
    pub fail_soft_bin: u16,
}

/// One ATPG pattern group in the program (executes as a STDF FTR).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternGroup {
    pub name: String,
    /// Number of pattern vectors in the group (STDF FTR `CYCL_CNT` input).
    pub vector_count: u32,
    /// The FTR/PTR test number this group reports under.
    pub test_number: u32,
    /// Software bin assigned when this group fails (0 → taxonomy default).
    pub fail_soft_bin: u16,
}

/// The executable test program for one design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestProgram {
    /// Design name (DFT-inserted netlist the patterns were generated for).
    pub design_name: String,
    pub lot_id: String,
    pub part_type: String,
    pub sblot_id: String,
    pub tests: Vec<TestSpec>,
    pub pattern_groups: Vec<PatternGroup>,
}
