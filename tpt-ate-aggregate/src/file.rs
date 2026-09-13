//! File-based outcome exchange — RFC-002 Section 4.8's offline
//! (signed-file) mode, reused unchanged by RFC-003: reports are standalone
//! JSON files, manually sent. **No new transmission mechanism** is defined
//! here or anywhere in `tpt-ate`; there is no live API and no network
//! transport in this crate, by design.

use crate::schema::{OutcomeReport, SchemaVersion};
use serde_json::to_string_pretty;
use std::path::Path;
use thiserror::Error;

/// Errors from the file exchange and ingestion path.
#[derive(Debug, Error)]
pub enum AggregateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("outcome file is not valid JSON: {0}")]
    MalformedJson(String),

    #[error("outcome file schema version {found} is not readable (supports up to {supported})")]
    UnsupportedSchemaVersion { found: String, supported: String },

    #[error("consent scope {0:?} forbids transmission; refusing to write a shareable file")]
    ConsentForbidsSharing(crate::schema::ConsentScope),
}

/// RFC-002 Section 4.1's writer trait, verbatim.
pub trait OutcomeFileWriter {
    fn write_outcome_file(&self, report: OutcomeReport, path: &Path) -> Result<(), AggregateError>;
}

/// RFC-002 Section 4.1's reader trait, verbatim.
pub trait OutcomeFileReader {
    fn read_outcome_file(&self, path: &Path) -> Result<OutcomeReport, AggregateError>;
}

/// The JSON implementation of the file exchange. Files are standalone
/// (RFC-002 Section 4.8): no sidecar state, no network, manually carried.
/// Signing is out of band — a report's `signature` field is carried through
/// untouched, and this writer neither adds nor strips one.
pub struct JsonOutcomeFile {
    /// Highest schema minor version this side understands (RFC-002 Section
    /// 4.5: same major, minor ≤ supported).
    pub supported_schema: SchemaVersion,
}

impl JsonOutcomeFile {
    pub fn new() -> JsonOutcomeFile {
        JsonOutcomeFile { supported_schema: SchemaVersion::CURRENT }
    }
}

impl Default for JsonOutcomeFile {
    fn default() -> Self {
        JsonOutcomeFile::new()
    }
}

impl OutcomeFileWriter for JsonOutcomeFile {
    fn write_outcome_file(&self, report: OutcomeReport, path: &Path) -> Result<(), AggregateError> {
        // Guardrail, not enforcement: NoSharing means the tooling is used
        // locally — nothing is serialized for transport in the first place.
        if report.consent == crate::schema::ConsentScope::NoSharing {
            return Err(AggregateError::ConsentForbidsSharing(report.consent));
        }
        let json =
            to_string_pretty(&report).map_err(|e| AggregateError::MalformedJson(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

impl OutcomeFileReader for JsonOutcomeFile {
    fn read_outcome_file(&self, path: &Path) -> Result<OutcomeReport, AggregateError> {
        let text = std::fs::read_to_string(path)?;
        let report: OutcomeReport = serde_json::from_str(&text)
            .map_err(|e| AggregateError::MalformedJson(format!("{}: {e}", path.display())))?;
        if !report.schema_version.is_readable_by(&self.supported_schema) {
            return Err(AggregateError::UnsupportedSchemaVersion {
                found: report.schema_version.to_string(),
                supported: self.supported_schema.to_string(),
            });
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::TestOutcome;
    use crate::schema::{ConsentScope, ManufacturingTrack, OutcomeReport, YieldSummary};
    use uuid::Uuid;

    fn sample_report(consent: ConsentScope) -> OutcomeReport {
        OutcomeReport {
            payload_manifest_id: Uuid::new_v4(),
            track: ManufacturingTrack::TestAssembly(TestOutcome::empty()),
            measured_geometry: vec![],
            electrical_test: vec![],
            yield_outcome: YieldSummary { total_units: 1, good_units: 1, yield_fraction: 1.0 },
            process_notes: vec![],
            consent,
            schema_version: SchemaVersion::CURRENT,
            signature: None,
        }
    }

    #[test]
    fn roundtrip_through_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("tpt-ate-file-{}-x.json", std::process::id()));
        let exchange = JsonOutcomeFile::new();
        exchange.write_outcome_file(sample_report(ConsentScope::PrivateBilateral), &path).unwrap();
        let back = exchange.read_outcome_file(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(back.track, ManufacturingTrack::TestAssembly(_)));
    }

    #[test]
    fn no_sharing_consent_blocks_export() {
        let exchange = JsonOutcomeFile::new();
        let err = exchange
            .write_outcome_file(
                sample_report(ConsentScope::NoSharing),
                Path::new("/tmp/never.json"),
            )
            .unwrap_err();
        assert!(matches!(err, AggregateError::ConsentForbidsSharing(ConsentScope::NoSharing)));
    }

    #[test]
    fn future_schema_rejected() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("tpt-ate-file-{}-future.json", std::process::id()));
        let mut report = sample_report(ConsentScope::AggregatedContribution);
        report.schema_version = crate::schema::SchemaVersion { major: 9, minor: 9 };
        let exchange = JsonOutcomeFile::new();
        // Write via std::fs directly (the writer would accept it; the gate
        // is on the reading side).
        std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        let err = exchange.read_outcome_file(&path).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            err,
            AggregateError::UnsupportedSchemaVersion { ref found, .. } if found == "9.9"
        ));
    }
}
