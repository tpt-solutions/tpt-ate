//! Bin taxonomy: the hardware (HBR) and software (SBR) bin definitions a
//! test program sorts dies into, plus the aggregate tallies the STDF
//! summary records and `BinDistribution` are built from.
//!
//! Defaults follow common ATE practice: hard bin 1 = pass; failing hard
//! bins group by failure class (parametric / functional / abort); soft bins
//! identify the specific failing test group. A real program's bin policy
//! comes from `tpt-silicon`'s test program (see [`crate::patterns`]), so the
//! taxonomy here is configurable rather than hard-coded.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Definition of one hardware bin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardBin {
    pub number: u16,
    /// Whether this bin is a pass bin.
    pub pass: bool,
    pub name: String,
}

/// Definition of one software bin and the hardware bin it rolls up into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftBin {
    pub number: u16,
    pub name: String,
    pub hard_bin: u16,
}

/// The bin taxonomy for a test program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinTaxonomy {
    pub hard_bins: BTreeMap<u16, HardBin>,
    pub soft_bins: BTreeMap<u16, SoftBin>,
}

impl BinTaxonomy {
    /// Default taxonomy used when the incoming test program does not carry
    /// its own bin policy:
    ///
    /// - hard bins: 1 PASS, 10 PARAMETRIC_FAIL, 11 FUNCTIONAL_FAIL, 12 ABORT
    /// - soft bins: 1 PASS_ALL → 1; 10 FAIL_PARAMETRIC → 10;
    ///   11 FAIL_FUNCTIONAL → 11; 12 FAIL_ABORT → 12
    pub fn defaults() -> BinTaxonomy {
        let hard = [
            HardBin { number: 1, pass: true, name: "PASS".into() },
            HardBin { number: 10, pass: false, name: "PARAMETRIC_FAIL".into() },
            HardBin { number: 11, pass: false, name: "FUNCTIONAL_FAIL".into() },
            HardBin { number: 12, pass: false, name: "ABORT".into() },
        ];
        let soft = [
            SoftBin { number: 1, name: "PASS_ALL".into(), hard_bin: 1 },
            SoftBin { number: 10, name: "FAIL_PARAMETRIC".into(), hard_bin: 10 },
            SoftBin { number: 11, name: "FAIL_FUNCTIONAL".into(), hard_bin: 11 },
            SoftBin { number: 12, name: "FAIL_ABORT".into(), hard_bin: 12 },
        ];
        BinTaxonomy {
            hard_bins: hard.into_iter().map(|b| (b.number, b)).collect(),
            soft_bins: soft.into_iter().map(|b| (b.number, b)).collect(),
        }
    }

    /// The hardware bin a software bin rolls up into (falls back to the
    /// ABORT bin for unknown soft bins).
    pub fn hard_bin_of(&self, soft_bin: u16) -> u16 {
        self.soft_bins.get(&soft_bin).map(|b| b.hard_bin).unwrap_or(12)
    }

    /// Is this hard bin a pass bin? (Unknown bins are treated as fail.)
    pub fn is_pass_hard_bin(&self, hard_bin: u16) -> bool {
        self.hard_bins.get(&hard_bin).map(|b| b.pass).unwrap_or(false)
    }
}

impl Default for BinTaxonomy {
    fn default() -> Self {
        BinTaxonomy::defaults()
    }
}

/// Aggregate bin tallies over one wafer run — the source for STDF's HBR/SBR
/// summary records and Phase 3's `BinDistribution`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BinTally {
    pub total_tested: u32,
    pub total_passed: u32,
    /// Dies per hardware bin.
    pub hard_bins: BTreeMap<u16, u32>,
    /// Dies per software bin.
    pub soft_bins: BTreeMap<u16, u32>,
    pub aborted: u32,
}

impl BinTally {
    /// Count one more die with the given assignment.
    pub fn record(&mut self, passed: bool, hard_bin: u16, soft_bin: u16) {
        self.total_tested += 1;
        self.total_passed += passed as u32;
        *self.hard_bins.entry(hard_bin).or_insert(0) += 1;
        *self.soft_bins.entry(soft_bin).or_insert(0) += 1;
    }

    /// Die yield over tested dies (0.0-1.0); 0 for an empty run.
    pub fn yield_fraction(&self) -> f64 {
        if self.total_tested == 0 {
            return 0.0;
        }
        self.total_passed as f64 / self.total_tested as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_taxonomy_rolls_up() {
        let taxonomy = BinTaxonomy::defaults();
        assert_eq!(taxonomy.hard_bin_of(1), 1);
        assert_eq!(taxonomy.hard_bin_of(10), 10);
        assert_eq!(taxonomy.hard_bin_of(99), 12, "unknown soft bins abort-bin");
        assert!(taxonomy.is_pass_hard_bin(1));
        assert!(!taxonomy.is_pass_hard_bin(11));
        assert!(!taxonomy.is_pass_hard_bin(77), "unknown hard bins are not pass bins");
    }

    #[test]
    fn tally_counts_and_yield() {
        let mut tally = BinTally::default();
        tally.record(true, 1, 1);
        tally.record(true, 1, 1);
        tally.record(false, 11, 12);
        assert_eq!(tally.total_tested, 3);
        assert_eq!(tally.total_passed, 2);
        assert_eq!(tally.hard_bins.get(&1), Some(&2));
        assert_eq!(tally.soft_bins.get(&12), Some(&1));
        assert!((tally.yield_fraction() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(BinTally::default().yield_fraction(), 0.0);
    }
}
