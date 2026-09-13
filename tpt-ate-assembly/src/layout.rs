//! Placement reference model: the interposer/chiplet layout
//! `tpt-ate-assembly` verifies executed placements against.
//!
//! `tpt-silicon`'s RFC-001 interposer/chiplet layout types are not
//! implemented yet (confirmed against that repo), so this module defines the
//! reference shape `tpt-ate` consumes. Conventions deliberately mirror
//! `tpt-silicon`'s physical model so the layout data can be carried through
//! unmodified when the real handoff lands: nanometer `i64` coordinates,
//! positions are bottom-left corners, `hi` edges exclusive.

use serde::{Deserialize, Serialize};

/// A point in nanometers. Mirrors `tpt-silicon`'s `tpt_layout::geometry::Point`
/// (i64 nm, bottom-left origin) so layout data carries through unmodified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PointNm {
    pub x: i64,
    pub y: i64,
}

impl PointNm {
    pub fn new(x: i64, y: i64) -> PointNm {
        PointNm { x, y }
    }

    /// Manhattan distance to another point (the placement-error metric).
    pub fn manhattan_distance(&self, other: &PointNm) -> i64 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

/// Orientation for placed dies/chiplets. Mirrors `tpt-silicon`'s
/// `tpt_layout::placement::Orientation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    /// No transform.
    R0,
    /// Mirror X (vertical flip).
    MX,
    /// Mirror Y (horizontal flip).
    MY,
    /// Rotate 180 degrees.
    R180,
}

/// One chiplet/die footprint in the package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DieSpec {
    /// Design identifier of the chiplet/die.
    pub chiplet_id: String,
    pub width_nm: i64,
    pub height_nm: i64,
}

/// An intended placement site on the interposer (or substrate/leadframe —
/// the model is the same at this level).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteSpec {
    pub site_id: String,
    /// Which die belongs here.
    pub chiplet_id: String,
    /// Bottom-left corner of the footprint, in nm.
    pub position: PointNm,
    pub orientation: Orientation,
}

/// The complete placement reference: `tpt-silicon`'s interposer/chiplet
/// layout as `tpt-ate-assembly` consumes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterposerLayout {
    pub design_name: String,
    /// Interposer (or substrate) extent in nm: (width, height).
    pub extent_nm: (i64, i64),
    /// Footprints keyed by chiplet id.
    pub dies: Vec<DieSpec>,
    /// Intended placement sites.
    pub sites: Vec<SiteSpec>,
}

impl InterposerLayout {
    /// The footprint of a chiplet, if the layout defines it.
    pub fn die_spec(&self, chiplet_id: &str) -> Option<&DieSpec> {
        self.dies.iter().find(|d| d.chiplet_id == chiplet_id)
    }

    /// A site by id.
    pub fn site(&self, site_id: &str) -> Option<&SiteSpec> {
        self.sites.iter().find(|s| s.site_id == site_id)
    }

    /// The axis-aligned occupied rect of a site (bottom-left + footprint).
    pub fn site_rect(&self, site: &SiteSpec) -> Option<(PointNm, PointNm)> {
        let spec = self.die_spec(&site.chiplet_id)?;
        Some((
            site.position,
            PointNm::new(site.position.x + spec.width_nm, site.position.y + spec.height_nm),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manhattan_distance_and_rect() {
        let a = PointNm::new(0, 0);
        let b = PointNm::new(3, -4);
        assert_eq!(a.manhattan_distance(&b), 7);
    }

    #[test]
    fn site_rect_uses_die_footprint() {
        let layout = InterposerLayout {
            design_name: "pkg".into(),
            extent_nm: (10_000, 10_000),
            dies: vec![DieSpec { chiplet_id: "c1".into(), width_nm: 2000, height_nm: 3000 }],
            sites: vec![SiteSpec {
                site_id: "s1".into(),
                chiplet_id: "c1".into(),
                position: PointNm::new(1000, 2000),
                orientation: Orientation::R0,
            }],
        };
        let rect = layout.site_rect(layout.site("s1").unwrap()).unwrap();
        assert_eq!(rect, (PointNm::new(1000, 2000), PointNm::new(3000, 5000)));
    }
}
