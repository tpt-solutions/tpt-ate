//! Bin-sort logic: turns per-die execution results into hardware/software
//! bin assignments keyed to the wafer map (RFC-003 Section 2A).
//!
//! Rules, in priority order:
//! 1. An aborted die bins to the ABORT soft bin (12 → hard 12 by default).
//! 2. The **first** failing item in execution order determines the bin: a
//!    failed measurement bins to its [`crate::patterns::TestSpec`]'s
//!    `fail_soft_bin`, a failed pattern group to its
//!    [`crate::patterns::PatternGroup`]'s `fail_soft_bin`; a zero soft bin
//!    falls back to the taxonomy default for that failure class.
//! 3. No failures → the pass bin (soft 1 → hard 1).

use crate::bins::{BinTally, BinTaxonomy};
use crate::patterns::TestProgram;
use crate::wafer::{DieCoord, DieTestOutcome, WaferDieMap};
use serde::{Deserialize, Serialize};

/// Default soft bin when a failing test does not name one: the taxonomy's
/// generic parametric fail bin.
pub const DEFAULT_PARAMETRIC_SOFT_BIN: u16 = 10;

/// Default soft bin for a failed pattern group: the taxonomy's generic
/// functional fail bin.
pub const DEFAULT_FUNCTIONAL_SOFT_BIN: u16 = 11;

/// Soft bin for an aborted die.
pub const ABORT_SOFT_BIN: u16 = 12;

/// Result of executing one die's program on the (real or simulated) tester,
/// in execution order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DieTestResult {
    pub coord: DieCoord,
    /// Parametric measurements, execution order.
    pub measurements: Vec<MeasurementOutcome>,
    /// Pattern-group executions, execution order.
    pub patterns: Vec<PatternOutcome>,
    /// The tester aborted before completing the program.
    pub aborted: bool,
}

/// One parametric measurement on one die.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementOutcome {
    pub test_number: u32,
    pub value: f32,
    pub passed: bool,
}

/// One pattern-group execution on one die.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternOutcome {
    pub name: String,
    pub test_number: u32,
    pub passed: bool,
}

/// The bin assignment for one die.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinAssignment {
    pub coord: DieCoord,
    pub passed: bool,
    pub hard_bin: u16,
    pub soft_bin: u16,
    /// Test number of the first failing item, if the die failed.
    pub failing_test: Option<u32>,
}

/// Sort one die's results into its bin assignment, consulting the program's
/// per-test bin policy.
pub fn sort_die(
    result: &DieTestResult,
    program: &TestProgram,
    taxonomy: &BinTaxonomy,
) -> BinAssignment {
    let assign_fail = |soft_bin: u16, fallback: u16, failing_test: Option<u32>| {
        let soft = if soft_bin == 0 { fallback } else { soft_bin };
        let hard = taxonomy.hard_bin_of(soft);
        BinAssignment {
            coord: result.coord,
            passed: false,
            hard_bin: hard,
            soft_bin: soft,
            failing_test,
        }
    };

    if result.aborted {
        return assign_fail(ABORT_SOFT_BIN, ABORT_SOFT_BIN, None);
    }
    for measurement in &result.measurements {
        if !measurement.passed {
            let declared = program
                .tests
                .iter()
                .find(|s| s.test_number == measurement.test_number)
                .map(|s| s.fail_soft_bin)
                .unwrap_or(0);
            return assign_fail(
                declared,
                DEFAULT_PARAMETRIC_SOFT_BIN,
                Some(measurement.test_number),
            );
        }
    }
    for pattern in &result.patterns {
        if !pattern.passed {
            let declared = program
                .pattern_groups
                .iter()
                .find(|g| g.test_number == pattern.test_number)
                .map(|g| g.fail_soft_bin)
                .unwrap_or(0);
            return assign_fail(declared, DEFAULT_FUNCTIONAL_SOFT_BIN, Some(pattern.test_number));
        }
    }
    BinAssignment {
        coord: result.coord,
        passed: true,
        hard_bin: taxonomy.hard_bin_of(1),
        soft_bin: 1,
        failing_test: None,
    }
}

/// Sort a full wafer's results: fills the wafer map and the bin tally.
/// Returns the number of results whose coordinate is not on the layout
/// (counted in the tally but impossible to place on the map).
pub fn sort_wafer(
    results: &[DieTestResult],
    wafer: &mut WaferDieMap,
    program: &TestProgram,
    taxonomy: &BinTaxonomy,
) -> u32 {
    let mut unmatched = 0u32;
    for result in results {
        let assignment = sort_die(result, program, taxonomy);
        if !wafer.record(
            assignment.coord,
            DieTestOutcome {
                passed: assignment.passed,
                hard_bin: assignment.hard_bin,
                soft_bin: assignment.soft_bin,
            },
        ) {
            unmatched += 1;
        }
    }
    unmatched
}

/// Build the run-level [`BinTally`] from a set of sorted results.
pub fn tally(results: &[DieTestResult], program: &TestProgram, taxonomy: &BinTaxonomy) -> BinTally {
    let mut out = BinTally::default();
    for result in results {
        let assignment = sort_die(result, program, taxonomy);
        out.record(assignment.passed, assignment.hard_bin, assignment.soft_bin);
        if result.aborted {
            out.aborted += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{PatternGroup, TestSpec};

    fn taxonomy() -> BinTaxonomy {
        BinTaxonomy::defaults()
    }

    fn program() -> TestProgram {
        TestProgram {
            design_name: "D".into(),
            lot_id: "L".into(),
            part_type: "P".into(),
            sblot_id: "S".into(),
            tests: vec![
                TestSpec {
                    test_number: 10,
                    name: "IDDQ".into(),
                    unit: "uA".into(),
                    nominal: 1.0,
                    lo_limit: Some(0.0),
                    hi_limit: Some(2.0),
                    fail_soft_bin: 0, // falls back to the default parametric bin
                },
                TestSpec {
                    test_number: 11,
                    name: "VDD_MIN".into(),
                    unit: "V".into(),
                    nominal: 3.3,
                    lo_limit: Some(3.0),
                    hi_limit: Some(3.6),
                    fail_soft_bin: 10,
                },
            ],
            pattern_groups: vec![PatternGroup {
                name: "ATPG_SCAN".into(),
                vector_count: 512,
                test_number: 20,
                fail_soft_bin: 0,
            }],
        }
    }

    #[test]
    fn passing_die_bins_pass() {
        let result = DieTestResult {
            coord: DieCoord::new(1, 1),
            measurements: vec![MeasurementOutcome { test_number: 10, value: 3.3, passed: true }],
            patterns: vec![PatternOutcome {
                name: "ATPG_SCAN".into(),
                test_number: 20,
                passed: true,
            }],
            aborted: false,
        };
        let assignment = sort_die(&result, &program(), &taxonomy());
        assert!(assignment.passed);
        assert_eq!((assignment.hard_bin, assignment.soft_bin), (1, 1));
        assert_eq!(assignment.failing_test, None);
    }

    #[test]
    fn failed_measurement_uses_spec_soft_bin() {
        let result = DieTestResult {
            coord: DieCoord::new(0, 0),
            measurements: vec![
                MeasurementOutcome { test_number: 10, value: 3.3, passed: true },
                MeasurementOutcome { test_number: 11, value: 2.9, passed: false },
            ],
            patterns: vec![],
            aborted: false,
        };
        let assignment = sort_die(&result, &program(), &taxonomy());
        assert!(!assignment.passed);
        assert_eq!(assignment.failing_test, Some(11));
        assert_eq!(assignment.soft_bin, 10, "spec's declared soft bin");
        assert_eq!(assignment.hard_bin, 10);
    }

    #[test]
    fn failed_pattern_falls_back_to_functional_default() {
        let result = DieTestResult {
            coord: DieCoord::new(0, 1),
            measurements: vec![MeasurementOutcome { test_number: 10, value: 3.3, passed: true }],
            patterns: vec![PatternOutcome {
                name: "ATPG_SCAN".into(),
                test_number: 20,
                passed: false,
            }],
            aborted: false,
        };
        let assignment = sort_die(&result, &program(), &taxonomy());
        assert_eq!(assignment.soft_bin, DEFAULT_FUNCTIONAL_SOFT_BIN);
        assert_eq!(assignment.hard_bin, 11);
    }

    #[test]
    fn earlier_failure_wins_over_later() {
        let result = DieTestResult {
            coord: DieCoord::new(0, 1),
            measurements: vec![MeasurementOutcome { test_number: 10, value: 9.9, passed: false }],
            patterns: vec![PatternOutcome {
                name: "ATPG_SCAN".into(),
                test_number: 20,
                passed: false,
            }],
            aborted: false,
        };
        assert_eq!(
            sort_die(&result, &program(), &taxonomy()).soft_bin,
            DEFAULT_PARAMETRIC_SOFT_BIN,
            "the measurement failed first, so its class wins"
        );
    }

    #[test]
    fn aborted_die_bins_abort() {
        let result = DieTestResult {
            coord: DieCoord::new(2, 2),
            measurements: vec![],
            patterns: vec![],
            aborted: true,
        };
        let assignment = sort_die(&result, &program(), &taxonomy());
        assert!(!assignment.passed);
        assert_eq!((assignment.hard_bin, assignment.soft_bin), (12, ABORT_SOFT_BIN));
    }

    #[test]
    fn sort_wafer_fills_map_and_tally() {
        let mut wafer = WaferDieMap::rectangular("W1", 2, 2, 100.0, 100.0);
        let results = vec![
            DieTestResult {
                coord: DieCoord::new(0, 0),
                measurements: vec![],
                patterns: vec![],
                aborted: false,
            },
            DieTestResult {
                coord: DieCoord::new(1, 1),
                measurements: vec![MeasurementOutcome {
                    test_number: 1,
                    value: 0.0,
                    passed: false,
                }],
                patterns: vec![],
                aborted: false,
            },
        ];
        let unmatched = sort_wafer(&results, &mut wafer, &program(), &taxonomy());
        assert_eq!(unmatched, 0);
        let tally = tally(&results, &program(), &taxonomy());
        assert_eq!(tally.total_tested, 2);
        assert_eq!(tally.total_passed, 1);
        assert_eq!(wafer.tested_counts(), (2, 1));
    }
}
