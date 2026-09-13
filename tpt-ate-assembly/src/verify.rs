//! Placement verification: check executed placements against
//! `tpt-silicon`'s interposer/chiplet layout as the placement reference
//! (RFC-003 Section 2B). Pure geometry over reported positions — the same
//! checks apply no matter which assembly backend produced the records.

use crate::backend::PlacementRecord;
use crate::layout::{InterposerLayout, PointNm};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One verification failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlacementViolation {
    /// A record names a site the layout does not define.
    UnknownSite { site_id: String },
    /// Two records claim the same site.
    DuplicatePlacement { site_id: String },
    /// A layout site received no placement.
    MissingPlacement { site_id: String },
    /// As-placed position deviates more than the tolerance from nominal
    /// (Manhattan distance, nm).
    OffTarget { site_id: String, deviation_nm: i64, tolerance_nm: i64 },
    /// The placed footprint is not fully inside the interposer extent.
    OutOfBounds { site_id: String, placed: PointNm },
    /// Two placed footprints overlap.
    Overlap { first: String, second: String },
    /// Placed orientation differs from the reference.
    OrientationMismatch { site_id: String },
}

/// Verification outcome for a set of placement records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacementReport {
    pub violations: Vec<PlacementViolation>,
    /// Number of records verified.
    pub placed_count: usize,
}

impl PlacementReport {
    /// True when no violations were found.
    pub fn is_ok(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Verify placement records against the reference layout.
///
/// `tolerance_nm` is the allowed Manhattan deviation of the as-placed
/// bottom-left corner from the nominal site position (equipment process
/// capability, not a layout property).
pub fn verify_placements(
    records: &[PlacementRecord],
    layout: &InterposerLayout,
    tolerance_nm: i64,
) -> PlacementReport {
    let mut violations = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();

    for record in records {
        let site = match layout.site(&record.site_id) {
            Some(site) => site,
            None => {
                violations
                    .push(PlacementViolation::UnknownSite { site_id: record.site_id.clone() });
                continue;
            }
        };
        if !seen.insert(&record.site_id) {
            violations
                .push(PlacementViolation::DuplicatePlacement { site_id: record.site_id.clone() });
            continue;
        }
        if record.orientation != site.orientation {
            violations
                .push(PlacementViolation::OrientationMismatch { site_id: record.site_id.clone() });
        }
        let deviation = record.placed.manhattan_distance(&record.nominal);
        if deviation > tolerance_nm {
            violations.push(PlacementViolation::OffTarget {
                site_id: record.site_id.clone(),
                deviation_nm: deviation,
                tolerance_nm,
            });
        }
        // Bounds: the placed footprint must sit inside the interposer.
        if let Some(spec) = layout.die_spec(&record.chiplet_id) {
            let hi =
                PointNm::new(record.placed.x + spec.width_nm, record.placed.y + spec.height_nm);
            if record.placed.x < 0
                || record.placed.y < 0
                || hi.x > layout.extent_nm.0
                || hi.y > layout.extent_nm.1
            {
                violations.push(PlacementViolation::OutOfBounds {
                    site_id: record.site_id.clone(),
                    placed: record.placed,
                });
            }
        }
    }

    // Every layout site must have been placed exactly once.
    for site in &layout.sites {
        if !seen.contains(site.site_id.as_str()) {
            violations.push(PlacementViolation::MissingPlacement { site_id: site.site_id.clone() });
        }
    }

    // Overlap between placed footprints (pairwise; plans are small).
    for (i, a) in records.iter().enumerate() {
        let (a_lo, a_hi) = match placed_rect(a, layout) {
            Some(rect) => rect,
            None => continue,
        };
        for b in &records[i + 1..] {
            let (b_lo, b_hi) = match placed_rect(b, layout) {
                Some(rect) => rect,
                None => continue,
            };
            let overlaps = a_lo.x < b_hi.x && b_lo.x < a_hi.x && a_lo.y < b_hi.y && b_lo.y < a_hi.y;
            if overlaps {
                violations.push(PlacementViolation::Overlap {
                    first: a.site_id.clone(),
                    second: b.site_id.clone(),
                });
            }
        }
    }

    PlacementReport { violations, placed_count: records.len() }
}

/// The axis-aligned rect a placement occupies (bottom-left inclusive, top
/// exclusive), or `None` for records without a known footprint.
fn placed_rect(record: &PlacementRecord, layout: &InterposerLayout) -> Option<(PointNm, PointNm)> {
    let spec = layout.die_spec(&record.chiplet_id)?;
    Some((
        record.placed,
        PointNm::new(record.placed.x + spec.width_nm, record.placed.y + spec.height_nm),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::AssemblyTechnique;
    use crate::layout::{DieSpec, Orientation, SiteSpec};

    fn layout() -> InterposerLayout {
        InterposerLayout {
            design_name: "pkg-2chiplet".into(),
            extent_nm: (10_000, 8_000),
            dies: vec![
                DieSpec { chiplet_id: "core".into(), width_nm: 2000, height_nm: 2000 },
                DieSpec { chiplet_id: "io".into(), width_nm: 1500, height_nm: 2500 },
            ],
            sites: vec![
                SiteSpec {
                    site_id: "s_core".into(),
                    chiplet_id: "core".into(),
                    position: PointNm::new(1000, 1000),
                    orientation: Orientation::R0,
                },
                SiteSpec {
                    site_id: "s_io".into(),
                    chiplet_id: "io".into(),
                    position: PointNm::new(5000, 1000),
                    orientation: Orientation::R0,
                },
            ],
        }
    }

    fn record(site_id: &str, chiplet_id: &str, placed: PointNm) -> PlacementRecord {
        PlacementRecord {
            site_id: site_id.into(),
            chiplet_id: chiplet_id.into(),
            technique: AssemblyTechnique::ChipletPickPlace,
            nominal: PointNm::new(0, 0),
            placed,
            orientation: Orientation::R0,
        }
    }

    #[test]
    fn perfect_placements_pass() {
        let reference = layout();
        let records = reference
            .sites
            .iter()
            .map(|site| PlacementRecord {
                site_id: site.site_id.clone(),
                chiplet_id: site.chiplet_id.clone(),
                technique: AssemblyTechnique::ChipletPickPlace,
                nominal: site.position,
                placed: site.position,
                orientation: site.orientation,
            })
            .collect::<Vec<_>>();
        let report = verify_placements(&records, &reference, 50);
        assert!(report.is_ok(), "violations: {:?}", report.violations);
    }

    #[test]
    fn off_target_and_out_of_bounds_flagged() {
        let reference = layout();
        let mut records = vec![
            record("s_core", "core", PointNm::new(1000 + 500, 1000)), // way off target
            record("s_io", "io", PointNm::new(9_000, 7_000)),         // footprint overflows
        ];
        records[0].nominal = PointNm::new(1000, 1000);
        records[1].nominal = PointNm::new(5000, 1000);
        let report = verify_placements(&records, &reference, 50);
        assert!(report.violations.contains(&PlacementViolation::OffTarget {
            site_id: "s_core".into(),
            deviation_nm: 500,
            tolerance_nm: 50
        }));
        assert!(report.violations.iter().any(
            |v| matches!(v, PlacementViolation::OutOfBounds { site_id, .. } if site_id == "s_io")
        ));
    }

    #[test]
    fn missing_duplicate_unknown_and_overlap() {
        let reference = layout();
        let mut records = vec![
            record("s_core", "core", PointNm::new(1000, 1000)),
            record("s_core", "core", PointNm::new(1000, 1000)), // duplicate
            record("ghost", "core", PointNm::new(0, 0)),        // unknown site
        ];
        records[0].nominal = PointNm::new(1000, 1000);
        records[1].nominal = PointNm::new(1000, 1000);
        records[2].nominal = PointNm::new(0, 0);
        let report = verify_placements(&records, &reference, 50);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, PlacementViolation::DuplicatePlacement { .. })));
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, PlacementViolation::UnknownSite { .. })));
        assert!(report.violations.iter().any(
            |v| matches!(v, PlacementViolation::MissingPlacement { site_id } if site_id == "s_io")
        ));
    }

    #[test]
    fn overlap_detected_between_placed_footprints() {
        // Place both dies so their footprints intersect.
        let reference = layout();
        let mut records = vec![
            record("s_core", "core", PointNm::new(5000, 1500)),
            record("s_io", "io", PointNm::new(5500, 2000)),
        ];
        records[0].nominal = PointNm::new(1000, 1000); // deliberately off, ignored here
        records[1].nominal = PointNm::new(5000, 1000);
        let report = verify_placements(&records, &reference, 10_000);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, PlacementViolation::Overlap { first, second }
                if first == "s_core" && second == "s_io")));
    }

    #[test]
    fn orientation_mismatch_flagged() {
        let reference = layout();
        let mut r = record("s_core", "core", PointNm::new(1000, 1000));
        r.nominal = PointNm::new(1000, 1000);
        r.orientation = Orientation::R180;
        let report = verify_placements(&[r], &reference, 50);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, PlacementViolation::OrientationMismatch { .. })));
    }
}
