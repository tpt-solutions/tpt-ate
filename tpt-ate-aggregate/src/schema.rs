//! The RFC-002 (`tpt-silicon/spec3.txt` Section 4) outcome schema, extended
//! with the test & assembly track per RFC-003.
//!
//! RFC-002 defines `OutcomeReport`'s field list verbatim but leaves several
//! supporting types (`ElectricalMeasurement`, `GeometryDeviation`,
//! `YieldSummary`, `ProcessDeviation`, `SchemaVersion`, `ManufacturingTrack`)
//! unimplemented anywhere in the portfolio — this module is their first
//! materialization. When `tpt-silicon` lands the real shared schema crate,
//! these definitions should move there and this module should re-export
//! them; the shapes here were kept minimal with that move in mind.
//!
//! One schema, no parallel file format: test/assembly results ride inside
//! the same `OutcomeReport` the wafer-fab stage uses, via the
//! [`ManufacturingTrack::TestAssembly`] variant.

use crate::outcome::TestOutcome;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// The shared manufacturing track discriminator from RFC-002 Section 4.1
/// (`Pcb | WaferLitho(PatterningTech)`), extended with RFC-003's stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ManufacturingTrack {
    /// PCB fabrication (RFC-001 downstream extension).
    Pcb,
    /// Wafer-level patterning; payload identifies the litho technology
    /// (RFC-002 Section 2B's backend list: EUV, DUV multi-patterning, NIL,
    /// e-beam).
    WaferLitho(PatterningTech),
    /// Test & assembly (RFC-003): the stage this repo covers. Carries the
    /// `TestOutcome` inside the track variant — one schema, one ingestion
    /// path.
    TestAssembly(TestOutcome),
}

/// Patterning technology, mirroring `tpt-fab-litho`'s backend list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatterningTech {
    Euv,
    DuvMultiPattern,
    Nil,
    Ebeam,
}

/// As-built vs. as-designed geometry, keyed by a semantic ID (RFC-002:
/// "everything is keyed against the same semantic IDs the outbound payload
/// used … rather than raw coordinates"). At the assembly stage the key is
/// the placement site ID and the deviation is the placement error in nm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryDeviation {
    /// Semantic key: `FabricLink.id` at fab stage, site ID here.
    pub key: String,
    /// Signed magnitude of the deviation, in the stage's native unit
    /// (nanometers for placement geometry).
    pub deviation: f64,
    pub note: String,
}

/// An electrical measurement (RFC-002's `electrical_test` element shape).
/// At the fab stage these are impedance/skew/IR-drop keyed to `FabricLink`
/// IDs; at the test & assembly stage they are package-level measurements
/// keyed the same way, so the ingestion path and calibration use are
/// identical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElectricalMeasurement {
    /// Semantic key of the measured path (package net / die-to-die link ID).
    pub key: String,
    pub quantity: MeasurementQuantity,
    pub measured_value: f64,
    pub unit: String,
    pub passed: bool,
}

/// What a measurement measures (RFC-002 names impedance, skew, IR-drop;
/// package test adds leakage and continuity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementQuantity {
    Impedance,
    Skew,
    IrDrop,
    Leakage,
    Continuity,
}

/// Lot/run-level yield summary (RFC-002 `yield_outcome`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YieldSummary {
    pub total_units: u32,
    pub good_units: u32,
    /// Good / total over tested units.
    pub yield_fraction: f64,
}

/// A deviation observed in process (RFC-002 `process_notes` element).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessDeviation {
    /// Short machine-readable code (e.g. "PLACEMENT_OFFSET_HIGH").
    pub code: String,
    pub description: String,
}

/// Consent scope travelling inside the report (RFC-002 Section 4.3,
/// verbatim): the ingestion pipeline enforces it automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsentScope {
    /// Raw data, shared only between this fab and tpt-solutions.
    PrivateBilateral,
    /// Fab-side aggregation only; feeds the shared public model.
    AggregatedContribution,
    /// Outcome tooling used locally, nothing transmitted.
    NoSharing,
}

/// Semver-style schema version (RFC-002 Section 4.5): the ingestion
/// pipeline rejects or best-effort-migrates unrecognized minor versions,
/// never silently misinterpreting a changed field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
}

impl SchemaVersion {
    /// The version this crate writes.
    pub const CURRENT: SchemaVersion = SchemaVersion { major: 0, minor: 1 };

    /// RFC-002 Section 4.5 compatibility: same major and a known-or-older
    /// minor is readable; anything else is rejected.
    pub fn is_readable_by(&self, ingestor: &SchemaVersion) -> bool {
        self.major == ingestor.major && self.minor <= ingestor.minor
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Detached signature for the offline signed-file mode (RFC-002 Section
/// 4.8): the file is valid standalone; the pipeline verifies out of band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// Signature algorithm identifier (e.g. "ed25519").
    pub algorithm: String,
    /// Opaque signature value (base64 in the JSON encoding).
    pub value: String,
}

/// The RFC-002 Section 4.1 outcome report, verbatim field list. This is the
/// one file format for every stage; `track` discriminates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeReport {
    /// Ties back to the exact payload sent out for this run.
    pub payload_manifest_id: Uuid,
    pub track: ManufacturingTrack,
    /// As-built vs. as-designed, keyed by semantic ID.
    pub measured_geometry: Vec<GeometryDeviation>,
    /// Electrical results keyed to the same semantic IDs.
    pub electrical_test: Vec<ElectricalMeasurement>,
    pub yield_outcome: YieldSummary,
    pub process_notes: Vec<ProcessDeviation>,
    pub consent: ConsentScope,
    pub schema_version: SchemaVersion,
    /// Present only when the fab signs its offline exports.
    pub signature: Option<Signature>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_gate() {
        let current = SchemaVersion::CURRENT;
        assert!(SchemaVersion { major: 0, minor: 0 }.is_readable_by(&current));
        assert!(SchemaVersion { major: 0, minor: 1 }.is_readable_by(&current));
        assert!(!SchemaVersion { major: 0, minor: 2 }.is_readable_by(&current));
        assert!(!SchemaVersion { major: 1, minor: 0 }.is_readable_by(&current));
    }

    #[test]
    fn outcome_report_json_roundtrip() {
        let report = OutcomeReport {
            payload_manifest_id: Uuid::nil(),
            track: ManufacturingTrack::TestAssembly(crate::outcome::TestOutcome::empty()),
            measured_geometry: vec![GeometryDeviation {
                key: "s_core".into(),
                deviation: 12.0,
                note: String::new(),
            }],
            electrical_test: vec![ElectricalMeasurement {
                key: "ugi_link_0".into(),
                quantity: MeasurementQuantity::Impedance,
                measured_value: 84.6,
                unit: "ohm".into(),
                passed: true,
            }],
            yield_outcome: YieldSummary { total_units: 25, good_units: 23, yield_fraction: 0.92 },
            process_notes: vec![],
            consent: ConsentScope::PrivateBilateral,
            schema_version: SchemaVersion::CURRENT,
            signature: None,
        };
        let json = serde_json::to_string_pretty(&report).expect("serialize");
        let back: OutcomeReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, report);
    }
}
