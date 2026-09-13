//! `TestOutcome` — the RFC-003 Section 2C extension to RFC-002's outcome
//! loop: bin/yield data from wafer test, plus package-level electrical
//! results from post-assembly test.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tpt_ate_test::bins::BinTally;
use tpt_ate_test::wafer::WaferDieMap;

use crate::schema::ElectricalMeasurement;

/// Bin/yield summary for the test & assembly stage, aggregated per the
/// RFC-002 Section 4.4 minimum-cohort rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinDistribution {
    pub total_tested: u32,
    pub total_passed: u32,
    pub yield_fraction: f64,
    /// Dies per hardware bin. Counts marked withheld by
    /// [`BinDistribution::redact_below_cohort`] keep their keys but hide
    /// their sizes.
    pub hard_bins: BTreeMap<u16, BinCount>,
    pub soft_bins: BTreeMap<u16, BinCount>,
}

/// One bucket of the bin distribution, with its privacy state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinCount {
    pub count: u32,
    /// True once [`BinDistribution::redact_below_cohort`] suppressed the
    /// exact count (bucket too small to publish under the cohort rule).
    pub withheld: bool,
}

impl BinDistribution {
    /// Build from the run tally produced by `tpt-ate-test`'s bin sort.
    pub fn from_tally(tally: &BinTally) -> BinDistribution {
        let map = |bins: &BTreeMap<u16, u32>| {
            bins.iter().map(|(&bin, &count)| (bin, BinCount { count, withheld: false })).collect()
        };
        BinDistribution {
            total_tested: tally.total_tested,
            total_passed: tally.total_passed,
            yield_fraction: tally.yield_fraction(),
            hard_bins: map(&tally.hard_bins),
            soft_bins: map(&tally.soft_bins),
        }
    }

    /// RFC-002 Section 4.4 minimum-cohort rule, applied at the source: any
    /// bin bucket with fewer than `min_cohort` dies gets its exact count
    /// withheld before the report leaves the OSAT, so an "aggregate of one
    /// data point" never ships. The spec's placeholder for the shared
    /// pipeline is N ≥ 5; callers working purely locally can pass 1.
    pub fn redact_below_cohort(&mut self, min_cohort: u32) {
        for bucket in self.hard_bins.values_mut().chain(self.soft_bins.values_mut()) {
            if bucket.count < min_cohort {
                bucket.withheld = true;
            }
        }
    }

    /// The publishable count of a bucket: exact when not withheld, otherwise
    /// a lower bound of `min_cohort` ("at least this many, no more detail").
    pub fn publishable_count(bucket: &BinCount, min_cohort: u32) -> u32 {
        if bucket.withheld {
            min_cohort
        } else {
            bucket.count
        }
    }
}

/// The RFC-003 Section 2C test & assembly outcome, carried inside
/// [`crate::schema::ManufacturingTrack::TestAssembly`] — not a parallel
/// file format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestOutcome {
    /// Per-die bin result keyed by physical wafer location, carried through
    /// unmodified from `tpt-silicon`'s layout output (RFC-003 Section 5
    /// decision).
    pub wafer_map: WaferDieMap,
    /// Yield by bin category, aggregated per spec3 Section 4.4's
    /// minimum-cohort rule.
    pub bin_summary: BinDistribution,
    /// Post-assembly package test, same measurement shape as RFC-002's
    /// `electrical_test`.
    pub package_test: Vec<ElectricalMeasurement>,
}

impl TestOutcome {
    /// An empty outcome (used for schema round-trips).
    pub fn empty() -> TestOutcome {
        TestOutcome {
            wafer_map: WaferDieMap::rectangular("EMPTY", 1, 1, 1.0, 1.0),
            bin_summary: BinDistribution::from_tally(&BinTally::default()),
            package_test: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_ate_test::bin_sort::{sort_die, DieTestResult, MeasurementOutcome};
    use tpt_ate_test::bins::BinTaxonomy;
    use tpt_ate_test::patterns::{PatternGroup, TestProgram};
    use tpt_ate_test::wafer::DieCoord;

    fn program() -> TestProgram {
        TestProgram {
            design_name: "d".into(),
            lot_id: "l".into(),
            part_type: "p".into(),
            sblot_id: "w".into(),
            tests: vec![],
            pattern_groups: vec![PatternGroup {
                name: "g".into(),
                vector_count: 1,
                test_number: 1,
                fail_soft_bin: 0,
            }],
        }
    }

    #[test]
    fn cohort_redaction_withholds_small_buckets() {
        let taxonomy = BinTaxonomy::defaults();
        let program = program();
        // 3 passing dies and 1 failing die → pass bucket below N=5.
        let results: Vec<DieTestResult> = (0..3)
            .map(|i| DieTestResult {
                coord: DieCoord::new(i, 0),
                measurements: vec![MeasurementOutcome { test_number: 1, value: 1.0, passed: true }],
                patterns: vec![],
                aborted: false,
            })
            .chain(std::iter::once(DieTestResult {
                coord: DieCoord::new(9, 9),
                measurements: vec![MeasurementOutcome {
                    test_number: 1,
                    value: 9.0,
                    passed: false,
                }],
                patterns: vec![],
                aborted: false,
            }))
            .collect();
        let tally = tpt_ate_test::bin_sort::tally(&results, &program, &taxonomy);
        let mut distribution = BinDistribution::from_tally(&tally);
        distribution.redact_below_cohort(5);
        assert_eq!(BinDistribution::publishable_count(&distribution.hard_bins[&1], 5), 5);
        assert_eq!(BinDistribution::publishable_count(&distribution.hard_bins[&10], 5), 5);
        assert!(distribution.hard_bins[&1].withheld);
        assert!(distribution.soft_bins[&10].withheld);

        // With N=1 nothing is withheld.
        let mut full = BinDistribution::from_tally(&tally);
        full.redact_below_cohort(1);
        assert!(!full.hard_bins.values().any(|b| b.withheld));
    }

    #[test]
    fn sort_die_matches_tally_through_distribution() {
        let taxonomy = BinTaxonomy::defaults();
        let program = program();
        let result = DieTestResult {
            coord: DieCoord::new(0, 0),
            measurements: vec![],
            patterns: vec![],
            aborted: false,
        };
        let assignment = sort_die(&result, &program, &taxonomy);
        assert!(assignment.passed);
        let mut tally = BinTally::default();
        tally.record(assignment.passed, assignment.hard_bin, assignment.soft_bin);
        let distribution = BinDistribution::from_tally(&tally);
        assert_eq!(distribution.total_passed, 1);
        assert!((distribution.yield_fraction - 1.0).abs() < 1e-9);
    }
}
