//! Ingestion: the same `ManufacturingOutcomeIngestor` path RFC-002 defines
//! for wafer-fab outcomes, reused for test & assembly reports (RFC-003:
//! "wired into the same `tpt-ai` ingestion path `tpt-fab`'s outcome data
//! already uses").
//!
//! The `tpt-ai` side of that path is a cross-repo dependency and does not
//! exist as code yet; this module defines the ingestor contract verbatim
//! from spec3 Section 4.1 plus a local recording implementation for tests
//! and offline ingestion. When `tpt-ai` materializes, its pipeline should
//! implement this trait directly — nothing in `tpt-ate` needs to change.
//!
//! Value point (not built here, per RFC-003): bin-sort/yield ground truth
//! is a calibration input for `tpt-ai`'s yield-heatmap model and
//! `tpt-fab-process`'s OPC/etch simulator — a die that failed in a location
//! the etch simulator flagged as marginal is a direct, high-value
//! calibration point. The calibration wiring belongs to those consumers.

use crate::file::AggregateError;
use crate::schema::OutcomeReport;

/// RFC-002 Section 4.1's ingestor trait, verbatim.
pub trait ManufacturingOutcomeIngestor {
    fn ingest(&self, report: OutcomeReport) -> Result<(), AggregateError>;
}

/// Offline ingestion: keeps received reports in memory. Stands in for
/// `tpt-ai`'s pipeline in the synthetic end-to-end run, and serves as a
/// drop-in reference for any consumer wiring the real path.
#[derive(Default)]
pub struct RecordingIngestor {
    received: std::sync::Mutex<Vec<OutcomeReport>>,
}

impl RecordingIngestor {
    pub fn new() -> RecordingIngestor {
        RecordingIngestor::default()
    }

    /// The ingested reports, oldest first.
    pub fn received(&self) -> std::sync::MutexGuard<'_, Vec<OutcomeReport>> {
        self.received.lock().expect("ingestor mutex")
    }
}

impl ManufacturingOutcomeIngestor for RecordingIngestor {
    fn ingest(&self, report: OutcomeReport) -> Result<(), AggregateError> {
        self.received.lock().expect("ingestor mutex").push(report);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::TestOutcome;
    use crate::schema::{
        ConsentScope, ManufacturingTrack, OutcomeReport, SchemaVersion, YieldSummary,
    };
    use uuid::Uuid;

    #[test]
    fn ingestor_collects_reports() {
        let ingestor = RecordingIngestor::new();
        let report = OutcomeReport {
            payload_manifest_id: Uuid::new_v4(),
            track: ManufacturingTrack::TestAssembly(TestOutcome::empty()),
            measured_geometry: vec![],
            electrical_test: vec![],
            yield_outcome: YieldSummary { total_units: 0, good_units: 0, yield_fraction: 0.0 },
            process_notes: vec![],
            consent: ConsentScope::AggregatedContribution,
            schema_version: SchemaVersion::CURRENT,
            signature: None,
        };
        ingestor.ingest(report).unwrap();
        assert_eq!(ingestor.received().len(), 1);
    }
}
