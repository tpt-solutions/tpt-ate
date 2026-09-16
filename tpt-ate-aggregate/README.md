# tpt-ate-aggregate

![CI](https://github.com/tpt-solutions/tpt-ate/actions/workflows/ci.yml/badge.svg)

Outcome aggregation for the test & assembly stage: extends RFC-002's `OutcomeReport` file
exchange with `TestOutcome` (wafer map, bin distribution, package-level electrical
measurements) instead of defining a new channel. Same schema, same file-based manually
sent loop, same `tpt-ai` ingestion path — **no parallel file format and no new
transmission mechanism** (RFC-003, `spec.txt` §2C).

Part of [`tpt-ate`](https://github.com/tpt-solutions/tpt-ate). Consumes the wafer maps
produced by [`tpt-ate-test`](../tpt-ate-test) and the verified placements produced by
[`tpt-ate-assembly`](../tpt-ate-assembly).

## What it implements

- **The RFC-002 §4.1 schema** ([`schema`]) — `OutcomeReport` with the spec's verbatim
  field list, `ManufacturingTrack` (`Pcb`, `WaferLitho(PatterningTech)`, extended with
  `TestAssembly(TestOutcome)` per RFC-003), `ConsentScope` (travels *inside* the report so
  the pipeline enforces it automatically), `SchemaVersion` (spec §4.5: same major, minor
  ≤ supported is readable; anything else rejected), and `Signature` for the offline
  signed-file mode. The supporting types (`ElectricalMeasurement`, `GeometryDeviation`,
  `YieldSummary`, `ProcessDeviation`) are first materializations of types the RFC named
  but never implemented; `tpt-silicon`'s `tpt-silicon-mfg-schema` crate now holds the canonical
  copies with identical serde shapes, so files interchange.
- **`TestOutcome`** ([`outcome`]) — `wafer_map` (per-die bins keyed by physical
  location), `bin_summary`, and `package_test`, carried inside the track variant: one
  schema, one ingestion path.
- **Minimum-cohort rule** ([`outcome`]) — spec §4.4 applied at the source: bin buckets
  smaller than the configured cohort get their exact counts withheld before the report
  leaves the OSAT, so "an aggregate of one data point" never ships.
- **File exchange** ([`file`]) — the spec §4.8 offline (signed-file) mode: JSON files,
  valid standalone, manually sent. `NoSharing` consent blocks export entirely; signatures
  are carried through untouched. There is deliberately no network transport anywhere in
  this crate.
- **Ingestion** ([`ingest`]) — the `ManufacturingOutcomeIngestor` trait verbatim from
  spec §4.1 plus a reference `RecordingIngestor`. The real `tpt-ai` pipeline implements
  the same trait; nothing here changes when it lands.

## Installation

```toml
[dependencies]
tpt-ate-aggregate = "0.1"
```

## Usage

Build a report from a run, write it as a standalone file, and ingest it manually:

```rust
use tpt_ate_aggregate::{
    BinDistribution, ConsentScope, JsonOutcomeFile, ManufacturingTrack,
    OutcomeFileReader, OutcomeFileWriter, OutcomeReport, RecordingIngestor,
    SchemaVersion, TestOutcome, YieldSummary, ManufacturingOutcomeIngestor,
};
use uuid::Uuid;

let mut bin_summary = BinDistribution::from_tally(&tally);
bin_summary.redact_below_cohort(5); // spec3 §4.4: small buckets withheld at the source

let report = OutcomeReport {
    payload_manifest_id: Uuid::new_v4(),
    track: ManufacturingTrack::TestAssembly(TestOutcome {
        wafer_map,
        bin_summary,
        package_test: vec![],
    }),
    measured_geometry: vec![],   // as-built placement deviations, keyed by site id
    electrical_test: vec![],
    yield_outcome: YieldSummary { total_units: 25, good_units: 23, yield_fraction: 0.92 },
    process_notes: vec![],
    consent: ConsentScope::PrivateBilateral,
    schema_version: SchemaVersion::CURRENT,
    signature: None,
};

let exchange = JsonOutcomeFile::new();
exchange.write_outcome_file(report.clone(), &path)?;   // file-based, manually sent
let received = exchange.read_outcome_file(&path)?;     // schema-version gated on read

let ingestor = RecordingIngestor::new();              // stands in for tpt-ai's pipeline
ingestor.ingest(received)?;
```

## Position in tpt-ate

```text
tpt-ate-test (STDF + WaferDieMap)  ─┐
                                    ├─▶ TestOutcome ─▶ OutcomeReport ─▶ .json file
tpt-ate-assembly (placements)      ─┘                                    │  manual send
                                                                        ▼
                                                        tpt-ai ingestion (shared path)
```

Bin-sort and yield ground truth is exactly the calibration input `tpt-ai`'s
yield-heatmap model and `tpt-fab-process`'s OPC/etch simulator want — a die that failed
in a location the etch simulator flagged as marginal is a direct, high-value calibration
point. The calibration wiring belongs to those consumers, not here.

## Scope and limitations

- No live API, no network — that is a spec3 §4.8 requirement, not an omission.
- The JSON encoding is the portfolio's file convention (`serde_json` pretty, like
  `tpt-drc`'s reports); no FlatBuffers/Protobuf variant yet.
- `ConsentScope::NoSharing` is enforced at the writer (export refused); pipeline-side
  enforcement lives with the ingestor.

## Testing

`cargo test -p tpt-ate-aggregate` — schema round-trips, the schema-version gate, cohort
redaction, consent-blocked export, ingestor behavior, and
`tests/phase3_milestone.rs`: a deterministic synthetic end-to-end run (design stand-in →
simulated wafer sort → simulated chiplet assembly → outcome file → manual re-ingestion)
that completes without a design partner — the RFC-003 Phase 3 gate.

## License

Dual-licensed under MIT or Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE` in the
repository root. Copyright TPT Solutions.

[`schema`]: https://docs.rs/tpt-ate-aggregate/latest/tpt_ate_aggregate/schema/index.html
[`outcome`]: https://docs.rs/tpt-ate-aggregate/latest/tpt_ate_aggregate/outcome/index.html
[`file`]: https://docs.rs/tpt-ate-aggregate/latest/tpt_ate_aggregate/file/index.html
[`ingest`]: https://docs.rs/tpt-ate-aggregate/latest/tpt_ate_aggregate/ingest/index.html
