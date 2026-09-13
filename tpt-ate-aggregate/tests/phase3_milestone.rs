//! Phase 3 milestone gate (RFC-003 Section 4, Phase 4 in spec numbering):
//!
//! "A synthetic end-to-end run — design, DFT insertion, simulated test,
//! simulated assembly, outcome file generated and manually ingested —
//! completes without a design partner."
//!
//! The design half (DFT-inserted netlist + ATPG patterns + interposer
//! layout) is a cross-repo blocker owned by `tpt-silicon` and stands in
//! here as the synthetic [`TestProgram`] and [`InterposerLayout`];
//! everything downstream is real: wafer sort over the simulated tester,
//! bin sort keyed to the wafer map, chiplet placement over GEM against the
//! simulated placer, and the RFC-002 outcome file written and manually
//! re-ingested.

use std::thread;

use tpt_ate_aggregate::{
    BinDistribution, ConsentScope, ElectricalMeasurement, GeometryDeviation, JsonOutcomeFile,
    ManufacturingOutcomeIngestor, ManufacturingTrack, MeasurementQuantity, OutcomeFileReader,
    OutcomeFileWriter, OutcomeReport, ProcessDeviation, RecordingIngestor, SchemaVersion,
    TestOutcome, YieldSummary,
};
use tpt_ate_assembly::backend::{
    AssemblyBackend, AssemblyTechnique, PlacementCommand, PlacementPlan, PlacementRecord,
};
use tpt_ate_assembly::equipment::AssemblyEquipment;
use tpt_ate_assembly::layout::{DieSpec, InterposerLayout, Orientation, PointNm, SiteSpec};
use tpt_ate_assembly::link::AssemblyHost;
use tpt_ate_assembly::techniques::{simulated_equipment_tolerance_nm, ChipletPickPlaceBackend};
use tpt_ate_assembly::{verify_placements, PlacementReport};
use tpt_ate_comm::secs::transport::duplex;
use tpt_ate_test::bin_sort::{sort_wafer, tally as make_tally};
use tpt_ate_test::bins::{BinTally, BinTaxonomy};
use tpt_ate_test::patterns::{PatternGroup, TestProgram, TestSpec};
use tpt_ate_test::sim::{FaultModel, SimulatedTester};
use tpt_ate_test::stdf::{Record, StdfWriter};
use tpt_ate_test::wafer::{DieCoord, WaferDieMap};
use uuid::Uuid;

const SESSION_ID: u16 = 0;

// --- The design-partner stand-in: what `tpt-silicon` will export once its
// DFT/ATPG crates and RFC-001 layout exist. --------------------------------

fn design_outputs() -> (TestProgram, InterposerLayout) {
    let program = TestProgram {
        design_name: "chiplet-a1-scan".into(),
        lot_id: "LOT-2026-091".into(),
        part_type: "CHIPLET-A1".into(),
        sblot_id: "W01".into(),
        tests: vec![
            TestSpec {
                test_number: 100,
                name: "IDDQ".into(),
                unit: "uA".into(),
                nominal: 1.2,
                lo_limit: Some(0.0),
                hi_limit: Some(5.0),
                fail_soft_bin: 0,
            },
            TestSpec {
                test_number: 110,
                name: "VDD_CORE_MIN".into(),
                unit: "V".into(),
                nominal: 3.3,
                lo_limit: Some(3.0),
                hi_limit: Some(3.6),
                fail_soft_bin: 10,
            },
        ],
        pattern_groups: vec![PatternGroup {
            name: "ATPG_SCAN".into(),
            vector_count: 4096,
            test_number: 200,
            fail_soft_bin: 11,
        }],
    };
    let layout = InterposerLayout {
        design_name: "pkg-a1-2chiplet".into(),
        extent_nm: (10_000, 8_000),
        dies: vec![
            DieSpec { chiplet_id: "core".into(), width_nm: 2500, height_nm: 2500 },
            DieSpec { chiplet_id: "io".into(), width_nm: 1500, height_nm: 3000 },
        ],
        sites: vec![
            SiteSpec {
                site_id: "s_core".into(),
                chiplet_id: "core".into(),
                position: PointNm::new(1500, 2500),
                orientation: Orientation::R0,
            },
            SiteSpec {
                site_id: "s_io".into(),
                chiplet_id: "io".into(),
                position: PointNm::new(5000, 2500),
                orientation: Orientation::R0,
            },
        ],
    };
    (program, layout)
}

// --- Stage 1: simulated wafer sort. ----------------------------------------

fn run_wafer_sort(program: &TestProgram) -> (WaferDieMap, BinTally, Vec<u8>) {
    let faults = FaultModel::deterministic(0xA11CE, vec![DieCoord::new(1, 1), DieCoord::new(3, 2)]);
    let tester = SimulatedTester::new(program.clone(), faults);
    let mut wafer = WaferDieMap::rectangular("W01", 5, 5, 120.0, 160.0);

    let results: Vec<_> = wafer
        .dies
        .keys()
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .map(|c| tester.test_die(c))
        .collect();
    let bin_tally = make_tally(&results, program, &BinTaxonomy::defaults());
    let unmatched = sort_wafer(&results, &mut wafer, program, &BinTaxonomy::defaults());
    assert_eq!(unmatched, 0);

    // STDF recording of the same run (the file the tester writes).
    let records = tpt_ate_test::recording::build_wafer_run_records(
        program,
        &wafer,
        &results,
        &bin_tally,
        &BinTaxonomy::defaults(),
        &tpt_ate_test::recording::RunMetadata::default(),
    );
    assert!(matches!(records[0], Record::Far(_)));
    let mut stdf_bytes = Vec::new();
    StdfWriter::new(&mut stdf_bytes).write_all(&records).expect("STDF write");
    (wafer, bin_tally, stdf_bytes)
}

// --- Stage 2: simulated assembly. ------------------------------------------

fn run_assembly(layout: &InterposerLayout) -> (Vec<PlacementRecord>, PlacementReport) {
    let plan = PlacementPlan {
        plan_id: "pkg-a1-run-1".into(),
        commands: layout
            .sites
            .iter()
            .map(|site| PlacementCommand {
                site_id: site.site_id.clone(),
                chiplet_id: site.chiplet_id.clone(),
                target: site.position,
                orientation: site.orientation,
            })
            .collect(),
    };
    let (equip_half, host_half) = duplex();
    let equipment = AssemblyEquipment::new(AssemblyTechnique::ChipletPickPlace, 0xC0FFEE);
    let commands = plan.commands.clone();
    let machine = thread::spawn(move || equipment.run(equip_half, SESSION_ID, &commands));
    let host = AssemblyHost::connect(host_half, SESSION_ID).expect("connect placer");
    let mut backend = ChipletPickPlaceBackend::new(host, 300.0, 8.0);
    let records = backend.place(&plan).expect("placement");
    machine.join().expect("placer thread").expect("placer flow");

    let tolerance = simulated_equipment_tolerance_nm();
    let report = verify_placements(&records, layout, tolerance);
    (records, report)
}

/// As-built vs. as-designed geometry, keyed by site ID (RFC-002's
/// "semantic IDs, not raw coordinates").
fn measured_geometry_from(records: &[PlacementRecord]) -> Vec<GeometryDeviation> {
    records
        .iter()
        .map(|record| GeometryDeviation {
            key: record.site_id.clone(),
            deviation: record.placed.manhattan_distance(&record.nominal) as f64,
            note: format!("{} placement offset (nm)", record.site_id),
        })
        .collect()
}

// --- The end-to-end milestone. ----------------------------------------------

#[test]
fn phase3_milestone_end_to_end_outcome_file_roundtrip() {
    let (program, layout) = design_outputs();

    // Stage 1: wafer sort → wafer map + bins + STDF.
    let (wafer_map, bin_tally, _stdf) = run_wafer_sort(&program);
    let (tested, passed) = wafer_map.tested_counts();
    assert_eq!((tested, passed), (25, 23));

    // Stage 2: assembly → verified placements and as-built geometry.
    let (placement_records, placement) = run_assembly(&layout);
    assert!(placement.is_ok(), "{:?}", placement.violations);
    let measured_geometry = measured_geometry_from(&placement_records);
    assert_eq!(measured_geometry.len(), 2);
    assert!(measured_geometry
        .iter()
        .all(|g| g.deviation <= simulated_equipment_tolerance_nm() as f64));

    // Stage 3: aggregate — TestOutcome carried inside the shared
    // OutcomeReport (one schema, no parallel file format).
    let mut bin_summary = BinDistribution::from_tally(&bin_tally);
    bin_summary.redact_below_cohort(5);

    let outcome = TestOutcome {
        wafer_map,
        bin_summary,
        package_test: vec![
            ElectricalMeasurement {
                key: "ugi-core-io-link0".into(),
                quantity: MeasurementQuantity::Impedance,
                measured_value: 84.7,
                unit: "ohm".into(),
                passed: true,
            },
            ElectricalMeasurement {
                key: "vdd-core-ir".into(),
                quantity: MeasurementQuantity::IrDrop,
                measured_value: 41.0,
                unit: "mV".into(),
                passed: true,
            },
        ],
    };

    let report = OutcomeReport {
        payload_manifest_id: Uuid::new_v4(),
        track: ManufacturingTrack::TestAssembly(outcome),
        measured_geometry,
        electrical_test: vec![],
        yield_outcome: YieldSummary {
            total_units: 25,
            good_units: 23,
            yield_fraction: 23.0 / 25.0,
        },
        process_notes: vec![ProcessDeviation {
            code: "PLACEMENT_WITHIN_TOLERANCE".into(),
            description: "all chiplet placements within process tolerance".into(),
        }],
        consent: ConsentScope::PrivateBilateral,
        schema_version: SchemaVersion::CURRENT,
        signature: None,
    };

    // Stage 4: file-based, manually sent (RFC-002 Section 4.8 mode).
    let path = std::env::temp_dir().join(format!(
        "tpt-ate-e2e-{}-{}.outcome.json",
        std::process::id(),
        report.payload_manifest_id.simple()
    ));
    let exchange = JsonOutcomeFile::new();
    exchange.write_outcome_file(report.clone(), &path).expect("write outcome file");

    // Stage 5: manual ingestion into the shared path.
    let ingestor = RecordingIngestor::new();
    let received = exchange.read_outcome_file(&path).expect("read outcome file");
    ingestor.ingest(received).expect("ingest");
    std::fs::remove_file(&path).unwrap();

    let ingested = &ingestor.received()[0];
    assert_eq!(ingested.payload_manifest_id, report.payload_manifest_id);
    match &ingested.track {
        ManufacturingTrack::TestAssembly(outcome) => {
            assert_eq!(outcome.wafer_map.tested_counts(), (25, 23));
            assert_eq!(outcome.bin_summary.total_passed, 23);
            // Cohort rule: the 23-die pass bucket publishes exactly; every
            // bucket smaller than N=5 (both failing bins) is withheld.
            assert!(!outcome.bin_summary.hard_bins[&1].withheld);
            assert!(outcome.bin_summary.soft_bins.values().any(|b| b.withheld));
            assert!(outcome
                .bin_summary
                .soft_bins
                .values()
                .filter(|b| b.withheld)
                .all(|b| b.count < 5));
            assert_eq!(outcome.package_test.len(), 2);
        }
        other => panic!("expected TestAssembly track, got {other:?}"),
    }
    assert_eq!(ingested.yield_outcome.good_units, 23);
    assert_eq!(ingested.measured_geometry.len(), 2);
    assert!(ingested.process_notes.len() == 1);
}

#[test]
fn end_to_end_is_deterministic() {
    // The whole run is seeded — two passes produce identical outcome files,
    // which is what makes the synthetic run a usable regression gate.
    let (program, layout) = design_outputs();

    let (wafer_a, tally_a, _) = run_wafer_sort(&program);
    let (wafer_b, tally_b, _) = run_wafer_sort(&program);
    assert_eq!(wafer_a, wafer_b);
    assert_eq!(tally_a, tally_b);

    let (records_a, report_a) = run_assembly(&layout);
    let (records_b, report_b) = run_assembly(&layout);
    assert_eq!(records_a, records_b);
    assert_eq!(report_a, report_b);
    assert_eq!(measured_geometry_from(&records_a), measured_geometry_from(&records_b));
}
