//! Equipment simulator harness: a synthetic ATE that executes a
//! [`TestProgram`] per die and speaks the same SECS/GEM flow a real tester
//! would, so bin-sort logic and the recording path are testable without
//! real equipment access (RFC-003 Section 2A).
//!
//! Outcomes are fully deterministic: each die's fate is seeded from the
//! fault model plus its coordinates, so the milestone run reproduces
//! bit-for-bit.

use crate::bin_sort::{sort_die, BinAssignment, DieTestResult, MeasurementOutcome, PatternOutcome};
use crate::bins::BinTaxonomy;
use crate::patterns::TestProgram;
use crate::rng::Rng;
use crate::secs::error::{Result, SecsError};
use crate::secs::gem::{ceid, EquipmentGem, DIE_TESTED_RPTID};
use crate::secs::hsms::HsmsSession;
use crate::secs::item::SecsItem;
use crate::wafer::{DieCoord, DieTestOutcome, WaferDieMap};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// How the simulated tester decides which dies fail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultModel {
    /// Seed for the deterministic per-die RNG.
    pub seed: u64,
    /// Fraction of dies in [0, 1) that fail on a random draw.
    pub fail_rate: f32,
    /// Dies that always fail, regardless of the random draw.
    pub always_fail: Vec<DieCoord>,
}

impl FaultModel {
    pub fn deterministic(seed: u64, always_fail: Vec<DieCoord>) -> FaultModel {
        FaultModel { seed, fail_rate: 0.0, always_fail }
    }
}

/// The simulated automatic test equipment.
pub struct SimulatedTester {
    program: TestProgram,
    taxonomy: BinTaxonomy,
    faults: FaultModel,
}

impl SimulatedTester {
    pub fn new(program: TestProgram, faults: FaultModel) -> SimulatedTester {
        SimulatedTester { program, taxonomy: BinTaxonomy::defaults(), faults }
    }

    pub fn with_taxonomy(mut self, taxonomy: BinTaxonomy) -> Self {
        self.taxonomy = taxonomy;
        self
    }

    /// The program this tester executes.
    pub fn program(&self) -> &TestProgram {
        &self.program
    }

    /// Execute the full program against one die. Deterministic per
    /// (seed, coordinate).
    pub fn test_die(&self, coord: DieCoord) -> DieTestResult {
        let mut rng = Rng::from_parts(&[self.faults.seed, coord.x as u64, coord.y as u64]);
        let forced_fail = self.faults.always_fail.contains(&coord);
        let random_fail =
            !forced_fail && self.faults.fail_rate > 0.0 && rng.next_f32() < self.faults.fail_rate;
        let fail_this_die = forced_fail || random_fail;

        // Pick which single item fails when the die is a fail (stable within
        // the run; derived from the same per-die stream).
        let item_count = (self.program.tests.len() + self.program.pattern_groups.len()) as u64;
        let failing_item =
            if fail_this_die { (rng.next_u64() % item_count.max(1)) as usize } else { usize::MAX };

        let mut measurements = Vec::new();
        for (index, spec) in self.program.tests.iter().enumerate() {
            let passed = index != failing_item;
            let value = self.measurement_value(&mut rng, spec, passed);
            measurements.push(MeasurementOutcome { test_number: spec.test_number, value, passed });
        }
        let mut patterns = Vec::new();
        for (offset, group) in self.program.pattern_groups.iter().enumerate() {
            let index = self.program.tests.len() + offset;
            patterns.push(PatternOutcome {
                name: group.name.clone(),
                test_number: group.test_number,
                passed: index != failing_item,
            });
        }

        DieTestResult { coord, measurements, patterns, aborted: false }
    }

    /// A passing measurement jitters inside the limit window; a failing one
    /// lands just outside the nearest limit. One-sided limits jitter around
    /// the nominal.
    fn measurement_value(
        &self,
        rng: &mut Rng,
        spec: &crate::patterns::TestSpec,
        passed: bool,
    ) -> f32 {
        match (spec.lo_limit, spec.hi_limit) {
            (Some(lo), Some(hi)) => {
                if passed {
                    rng.uniform(lo + 0.05 * (hi - lo), lo + 0.95 * (hi - lo))
                } else if rng.next_f32() < 0.5 {
                    lo - 0.05 * (hi - lo).abs().max(1.0)
                } else {
                    hi + 0.05 * (hi - lo).abs().max(1.0)
                }
            }
            _ => {
                let span = spec.nominal.abs().max(1.0);
                if passed {
                    rng.uniform(spec.nominal - 0.1 * span, spec.nominal + 0.1 * span)
                } else if spec.lo_limit.is_some() {
                    spec.nominal - 0.2 * span
                } else {
                    spec.nominal + 0.2 * span
                }
            }
        }
    }

    /// Execute, then bin-sort one die.
    pub fn test_and_sort_die(&self, coord: DieCoord) -> (DieTestResult, BinAssignment) {
        let result = self.test_die(coord);
        let assignment = sort_die(&result, &self.program, &self.taxonomy);
        (result, assignment)
    }

    /// Equipment side of a full wafer run over an HSMS connection: accepts
    /// the Select, answers the GEM handshake (S1F1/S1F13), then sends one
    /// S6F11 event report per die in coordinate order, each carrying the
    /// die's bin assignment, and a final WAFER_DONE event.
    pub fn run_wafer_over_hsms<S: Read + Write>(
        &self,
        stream: S,
        session_id: u16,
        wafer: &WaferDieMap,
    ) -> Result<()> {
        let mut session = HsmsSession::new(stream, session_id);
        session.accept_select()?;
        let mut gem = EquipmentGem::new(session, "TPT-SIM-ATE", env!("CARGO_PKG_VERSION"));

        // Answer whatever handshake the host performs (S1F1 and/or S1F13).
        let mut served = 0;
        while served < 2 {
            gem.serve_one()?;
            served += 1;
        }

        let mut coords: Vec<DieCoord> = wafer.dies.keys().copied().collect();
        coords.sort();
        for coord in coords {
            let (_, assignment) = self.test_and_sort_die(coord);
            let outcome = DieTestOutcome {
                passed: assignment.passed,
                hard_bin: assignment.hard_bin,
                soft_bin: assignment.soft_bin,
            };
            gem.send_event_report(
                1,
                ceid::DIE_TESTED,
                &[(DIE_TESTED_RPTID, encode_die_report(coord, &outcome))],
            )?;
        }
        gem.send_event_report(1, ceid::WAFER_DONE, &[])?;
        Ok(())
    }
}

/// The per-die report variables carried in the S6F11 event (VID order per
/// [`crate::secs::gem::vid`]).
pub fn encode_die_report(coord: DieCoord, outcome: &DieTestOutcome) -> Vec<SecsItem> {
    vec![
        SecsItem::u2(coord.x as u16),
        SecsItem::u2(coord.y as u16),
        SecsItem::u2(outcome.hard_bin),
        SecsItem::u2(outcome.soft_bin),
        SecsItem::Boolean(vec![outcome.passed]),
    ]
}

/// Decode a per-die report from S6F11 V-list positional order.
pub fn decode_die_report(variables: &[SecsItem]) -> Result<(DieCoord, DieTestOutcome)> {
    if variables.len() != 5 {
        return Err(SecsError::Gem(format!(
            "die report has {} variables, expected 5",
            variables.len()
        )));
    }
    let x = variables[0].as_u2().ok_or_else(|| SecsError::Gem("DIE_X is not U2".into()))?;
    let y = variables[1].as_u2().ok_or_else(|| SecsError::Gem("DIE_Y is not U2".into()))?;
    let hard_bin =
        variables[2].as_u2().ok_or_else(|| SecsError::Gem("HARD_BIN is not U2".into()))?;
    let soft_bin =
        variables[3].as_u2().ok_or_else(|| SecsError::Gem("SOFT_BIN is not U2".into()))?;
    let passed = match &variables[4] {
        SecsItem::Boolean(v) => v.first().copied().unwrap_or(false),
        _ => return Err(SecsError::Gem("DIE_PASS is not BOOLEAN".into())),
    };
    Ok((DieCoord::new(x as i32, y as i32), DieTestOutcome { passed, hard_bin, soft_bin }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{PatternGroup, TestSpec};
    use crate::secs::gem::parse_s6f11;

    fn program() -> TestProgram {
        TestProgram {
            design_name: "chiplet-a1".into(),
            lot_id: "LOT-1".into(),
            part_type: "CHIPLET-A1".into(),
            sblot_id: "W01".into(),
            tests: vec![TestSpec {
                test_number: 10,
                name: "IDDQ".into(),
                unit: "uA".into(),
                nominal: 1.0,
                lo_limit: Some(0.0),
                hi_limit: Some(2.0),
                fail_soft_bin: 0,
            }],
            pattern_groups: vec![PatternGroup {
                name: "ATPG_SCAN".into(),
                vector_count: 256,
                test_number: 20,
                fail_soft_bin: 0,
            }],
        }
    }

    #[test]
    fn deterministic_per_die_outcomes() {
        let tester = SimulatedTester::new(
            program(),
            FaultModel::deterministic(42, vec![DieCoord::new(1, 1)]),
        );
        let first = tester.test_die(DieCoord::new(1, 1));
        let second = tester.test_die(DieCoord::new(1, 1));
        assert_eq!(first, second, "same die must simulate identically");
        let failing = sort_die(&first, &tester.program, &BinTaxonomy::defaults());
        assert!(!failing.passed, "(1,1) is in the always-fail list");
    }

    #[test]
    fn passing_dies_stay_in_limits() {
        let tester = SimulatedTester::new(program(), FaultModel::deterministic(7, vec![]));
        for x in 0..3 {
            for y in 0..3 {
                let result = tester.test_die(DieCoord::new(x, y));
                for m in &result.measurements {
                    assert!(m.passed, "no forced failures, so all pass");
                    assert!((0.0..=2.0).contains(&m.value));
                }
                assert!(result.patterns.iter().all(|p| p.passed));
            }
        }
    }

    #[test]
    fn die_report_roundtrip_through_secs_items() {
        let coord = DieCoord::new(3, 4);
        let outcome = DieTestOutcome { passed: false, hard_bin: 11, soft_bin: 12 };
        let items = encode_die_report(coord, &outcome);
        let (decoded_coord, decoded_outcome) = decode_die_report(&items).unwrap();
        assert_eq!(decoded_coord, coord);
        assert_eq!(decoded_outcome, outcome);
    }

    #[test]
    fn event_report_carries_die_report() {
        let items = encode_die_report(
            DieCoord::new(0, 0),
            &DieTestOutcome { passed: true, hard_bin: 1, soft_bin: 1 },
        );
        let body =
            crate::secs::gem::s6f11_event_report(1, ceid::DIE_TESTED, &[(DIE_TESTED_RPTID, items)]);
        let (_, parsed_ceid, reports) = parse_s6f11(&body).unwrap();
        assert_eq!(parsed_ceid, ceid::DIE_TESTED);
        assert_eq!(reports[0].1.len(), 5);
    }
}
