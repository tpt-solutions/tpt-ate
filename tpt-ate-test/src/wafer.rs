//! Wafer map model: per-die state keyed by physical die location on the
//! wafer.
//!
//! Per the RFC-003 Section 5 locked decision, die-location data is carried
//! through **unmodified** from `tpt-silicon`'s layout output — `tpt-ate`
//! applies no interchange transformation, and no shared format with
//! `tpt-fab` is introduced unless a real mismatch surfaces during
//! integration. Coordinates are wafer-level die indices: X increases right,
//! Y increases up, origin per the source layout.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Physical die location on a wafer, in die-index coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DieCoord {
    pub x: i32,
    pub y: i32,
}

impl DieCoord {
    pub fn new(x: i32, y: i32) -> DieCoord {
        DieCoord { x, y }
    }
}

impl fmt::Display for DieCoord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.x, self.y)
    }
}

/// Test result for one die.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DieTestOutcome {
    pub passed: bool,
    pub hard_bin: u16,
    pub soft_bin: u16,
}

/// State of one die on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DieState {
    /// Present on the wafer, not yet tested.
    Untested,
    /// Excluded from testing (edge exclusion, mark dies, etc.).
    Excluded,
    /// Tested; carries the bin assignment.
    Tested(DieTestOutcome),
}

/// JSON helpers: `serde_json` requires string map keys, so the
/// coordinate-keyed map exchanges as a list of `{coord, state}` entries.
mod coord_map_serde {
    use super::{DieCoord, DieState};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize)]
    struct Entry {
        coord: DieCoord,
        state: DieState,
    }

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<DieCoord, DieState>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(
            map.iter().map(|(coord, state)| Entry { coord: *coord, state: *state }),
        )
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<DieCoord, DieState>, D::Error> {
        Ok(Vec::<Entry>::deserialize(deserializer)?
            .into_iter()
            .map(|e| (e.coord, e.state))
            .collect())
    }
}

/// Geometry of the wafer being tested: which die sites exist, their size in
/// mils (STDF WCR convention), and the coordinate of the wafer center.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaferDieMap {
    pub wafer_id: String,
    /// All valid die sites (both testable and excluded ones).
    #[serde(with = "coord_map_serde")]
    pub dies: BTreeMap<DieCoord, DieState>,
    /// Die height in mils (STDF WCR `DIE_HT`).
    pub die_height_mils: f32,
    /// Die width in mils (STDF WCR `DIE_WID`).
    pub die_width_mils: f32,
    /// Die coordinate of the wafer center (STDF WCR `CENTER_X/Y`).
    pub center: DieCoord,
}

impl WaferDieMap {
    /// A rectangular grid of testable dies centered at the grid middle —
    /// the synthetic wafer the simulator and tests run against.
    pub fn rectangular(
        wafer_id: &str,
        cols: i32,
        rows: i32,
        die_width_mils: f32,
        die_height_mils: f32,
    ) -> WaferDieMap {
        let mut dies = BTreeMap::new();
        for x in 0..cols {
            for y in 0..rows {
                dies.insert(DieCoord::new(x, y), DieState::Untested);
            }
        }
        WaferDieMap {
            wafer_id: wafer_id.to_string(),
            dies,
            die_height_mils,
            die_width_mils,
            center: DieCoord::new(cols / 2, rows / 2),
        }
    }

    /// Record a test result for a die. Returns `false` if the coordinate is
    /// not a valid die site.
    pub fn record(&mut self, coord: DieCoord, outcome: DieTestOutcome) -> bool {
        match self.dies.get_mut(&coord) {
            Some(slot @ (DieState::Untested | DieState::Tested(_))) => {
                *slot = DieState::Tested(outcome);
                true
            }
            _ => false,
        }
    }

    /// Count of dies in each tested state bucket: (tested, passed).
    pub fn tested_counts(&self) -> (u32, u32) {
        let mut tested = 0;
        let mut passed = 0;
        for state in self.dies.values() {
            if let DieState::Tested(outcome) = state {
                tested += 1;
                passed += outcome.passed as u32;
            }
        }
        (tested, passed)
    }

    /// Dies per hardware bin, over tested dies only.
    pub fn hard_bin_counts(&self) -> BTreeMap<u16, u32> {
        let mut counts = BTreeMap::new();
        for state in self.dies.values() {
            if let DieState::Tested(outcome) = state {
                *counts.entry(outcome.hard_bin).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Dies per software bin, over tested dies only.
    pub fn soft_bin_counts(&self) -> BTreeMap<u16, u32> {
        let mut counts = BTreeMap::new();
        for state in self.dies.values() {
            if let DieState::Tested(outcome) = state {
                *counts.entry(outcome.soft_bin).or_insert(0) += 1;
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangular_layout_and_counts() {
        let mut wafer = WaferDieMap::rectangular("W1", 4, 4, 100.0, 200.0);
        assert_eq!(wafer.dies.len(), 16);
        assert_eq!(wafer.center, DieCoord::new(2, 2));
        assert!(wafer.record(
            DieCoord::new(0, 0),
            DieTestOutcome { passed: true, hard_bin: 1, soft_bin: 1 }
        ));
        assert!(wafer.record(
            DieCoord::new(3, 2),
            DieTestOutcome { passed: false, hard_bin: 11, soft_bin: 12 }
        ));
        assert!(!wafer.record(
            DieCoord::new(9, 9),
            DieTestOutcome { passed: true, hard_bin: 1, soft_bin: 1 }
        ));
        let (tested, passed) = wafer.tested_counts();
        assert_eq!((tested, passed), (2, 1));
        assert_eq!(wafer.hard_bin_counts().get(&11), Some(&1));
        assert_eq!(wafer.soft_bin_counts().get(&12), Some(&1));
    }

    #[test]
    fn coord_display_and_ordering() {
        let mut coords = vec![DieCoord::new(1, 0), DieCoord::new(0, 1), DieCoord::new(0, 0)];
        coords.sort();
        // Derived Ord is x-major, then y.
        assert_eq!(coords, vec![DieCoord::new(0, 0), DieCoord::new(0, 1), DieCoord::new(1, 0)]);
        assert_eq!(DieCoord::new(3, -1).to_string(), "(3,-1)");
    }
}
