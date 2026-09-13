//! Phase 1 milestone gate (RFC-003 Section 4, Phase 1):
//!
//! "A simulated test run against synthetic ATPG patterns produces a valid
//! STDF file with correct bin categorization."
//!
//! The exercise spans the whole stack: equipment simulator over HSMS/GEM
//! (in-memory duplex), S6F11 event reports per die, host-side collection,
//! STDF recording, then file read-back and cross-validation of the bin
//! categorization against the seeded fault model.

use std::collections::BTreeMap;
use std::io::Write;

use tpt_ate_test::bin_sort::{sort_wafer, tally as make_tally, DieTestResult};
use tpt_ate_test::bins::BinTaxonomy;
use tpt_ate_test::patterns::{PatternGroup, TestProgram, TestSpec};
use tpt_ate_test::recording::{build_wafer_run_records, decode_wafer_map_gdr, RunMetadata};
use tpt_ate_test::secs::gem::ceid;
use tpt_ate_test::secs::hsms::HsmsSession;
use tpt_ate_test::secs::transport::duplex;
use tpt_ate_test::sim::{decode_die_report, FaultModel, SimulatedTester};
use tpt_ate_test::stdf::{Record, StdfReader, StdfWriter};
use tpt_ate_test::wafer::{DieCoord, DieState, WaferDieMap};

const SESSION_ID: u16 = 0;

fn program() -> TestProgram {
    TestProgram {
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
        pattern_groups: vec![
            PatternGroup {
                name: "ATPG_SCAN".into(),
                vector_count: 4096,
                test_number: 200,
                fail_soft_bin: 11,
            },
            PatternGroup {
                name: "ATPG_MBIST".into(),
                vector_count: 1024,
                test_number: 210,
                fail_soft_bin: 12,
            },
        ],
    }
}

fn wafer() -> WaferDieMap {
    WaferDieMap::rectangular("W01", 5, 5, 120.0, 160.0)
}

/// The seeded fault model: exactly two dies fail — one parametric (VDD_CORE
/// low) and one functional — so every downstream count is checkable.
fn fault_model() -> FaultModel {
    FaultModel::deterministic(0xA11CE, vec![DieCoord::new(1, 1), DieCoord::new(3, 2)])
}

#[test]
fn phase1_milestone_simulated_run_produces_valid_stdf() {
    let program = program();
    let faults = fault_model();
    let tester = SimulatedTester::new(program.clone(), faults.clone());
    let mut wafer_layout = wafer();

    // --- Equipment side: run the wafer over HSMS/GEM on a companion thread.
    let (equip_half, host_half) = duplex();
    let equipment_wafer = wafer_layout.clone();
    let equipment = std::thread::spawn(move || {
        tester.run_wafer_over_hsms(equip_half, SESSION_ID, &equipment_wafer)
    });

    // --- Host side: select, handshake, collect per-die event reports.
    let mut session = HsmsSession::new(host_half, SESSION_ID);
    session.select().expect("HSMS select");
    let mut gem = tpt_ate_test::secs::gem::HostGem::new(session);

    let (model, rev) = gem.online_data().expect("S1F1/S1F2");
    assert_eq!(model, "TPT-SIM-ATE");
    assert!(!rev.is_empty());

    let established = gem.establish_communications().expect("S1F13/S1F14");
    assert!(established);

    let mut gem_results: BTreeMap<DieCoord, tpt_ate_test::wafer::DieTestOutcome> = BTreeMap::new();
    let die_count = wafer_layout.dies.len() as u32;
    let mut wafer_done_seen = false;
    while !wafer_done_seen {
        let (dataid, ce, reports) = gem.receive_event_report().expect("S6F11");
        assert_eq!(dataid, 1);
        match ce {
            ceid::DIE_TESTED => {
                assert_eq!(reports.len(), 1, "one report per die event");
                let (rptid, variables) = &reports[0];
                assert_eq!(*rptid, tpt_ate_test::secs::gem::DIE_TESTED_RPTID);
                assert_eq!(variables.len(), 5, "x, y, hard bin, soft bin, pass");
                let (coord, outcome) = decode_die_report(variables).expect("die report");
                gem_results.insert(coord, outcome);
            }
            ceid::WAFER_DONE => {
                assert!(reports.is_empty());
                wafer_done_seen = true;
            }
            other => panic!("unexpected CEID {other}"),
        }
    }
    assert_eq!(gem_results.len(), die_count as usize, "every die reported");

    equipment.join().expect("equipment thread").expect("equipment flow");

    // --- The seeded fault model must show up exactly over GEM.
    let expected_failures: Vec<DieCoord> = faults.always_fail.clone();
    for coord in &expected_failures {
        assert!(!gem_results[coord].passed, "die {coord} must fail");
    }
    let gem_failed: Vec<_> =
        gem_results.iter().filter(|(_, o)| !o.passed).map(|(c, _)| *c).collect();
    assert_eq!(gem_failed.len(), expected_failures.len(), "only the seeded dies fail");

    // --- Record the run into STDF (recompute the deterministic per-die
    // results host-side; the simulator is seeded so this is identical to
    // what the equipment side executed).
    let tester = SimulatedTester::new(program.clone(), faults);
    let results: Vec<DieTestResult> = wafer_layout
        .dies
        .keys()
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .map(|c| tester.test_die(c))
        .collect();

    let taxonomy = BinTaxonomy::defaults();
    let unmatched = sort_wafer(&results, &mut wafer_layout, &program, &taxonomy);
    assert_eq!(unmatched, 0);
    let bin_tally = make_tally(&results, &program, &taxonomy);

    // GEM-carried bins and locally-sorted bins must agree per die.
    for result in &results {
        let assignment = tpt_ate_test::bin_sort::sort_die(result, &program, &taxonomy);
        let from_gem = &gem_results[&result.coord];
        assert_eq!(
            (from_gem.passed, from_gem.hard_bin, from_gem.soft_bin),
            (assignment.passed, assignment.hard_bin, assignment.soft_bin),
            "die {} bins must match between GEM and local sort",
            result.coord
        );
    }

    let start_t = 1_700_000_000;
    let records = build_wafer_run_records(
        &program,
        &wafer_layout,
        &results,
        &bin_tally,
        &taxonomy,
        &RunMetadata { start_t, finish_t: start_t + 900, ..RunMetadata::default() },
    );

    // --- Write a real STDF file, then read it back.
    let path = std::env::temp_dir().join(format!(
        "tpt-ate-milestone-{}-{}.stdf",
        std::process::id(),
        start_t
    ));
    {
        let file = std::fs::File::create(&path).expect("create STDF file");
        let mut writer = StdfWriter::new(std::io::BufWriter::new(file));
        writer.write_all(&records).expect("write STDF");
        writer.into_inner().flush().expect("flush STDF");
    }
    let file = std::fs::File::open(&path).expect("reopen STDF file");
    let mut reader = StdfReader::new(std::io::BufReader::new(file));
    let parsed = reader.read_all().expect("parse STDF");
    let _ = std::fs::remove_file(&path);

    // --- Validate the file: skeleton, order, counts, bins.
    assert_eq!(parsed.len(), records.len(), "record count survives the round trip");
    assert!(matches!(&parsed[0], Record::Far(far) if far.stdf_ver == 4), "V4 FAR first");
    assert!(matches!(&parsed[2], Record::Mir(_)), "MIR third (FAR, ATR, MIR)");
    assert!(matches!(parsed.last(), Some(Record::Mrr(_))), "MRR last");

    // 25 dies x (2 PTR + 2 FTR).
    let ptrs: Vec<_> = parsed
        .iter()
        .filter_map(|r| match r {
            Record::Ptr(p) => Some(p.clone()),
            _ => None,
        })
        .collect();
    let ftrs: Vec<_> = parsed
        .iter()
        .filter_map(|r| match r {
            Record::Ftr(f) => Some(f.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ptrs.len(), 50, "25 dies x 2 parametric tests");
    assert_eq!(ftrs.len(), 50, "25 dies x 2 pattern groups");

    // Each failing die has exactly one failing item (the simulator picks one).
    let failed_ptr_count = ptrs.iter().filter(|p| p.failed()).count();
    let failed_ftr_count = ftrs.iter().filter(|f| f.failed()).count();
    assert_eq!(
        failed_ptr_count + failed_ftr_count,
        expected_failures.len(),
        "one failing item per failed die"
    );

    // Bin summary records carry the correct categorization.
    let hbrs: BTreeMap<u16, u32> = parsed
        .iter()
        .filter_map(|r| match r {
            Record::Hbr(h) => Some((h.hbin_num, h.hbin_cnt)),
            _ => None,
        })
        .collect();
    assert_eq!(*hbrs.get(&1).unwrap_or(&0), 23, "23 good dies in hard bin 1");
    let total_failed_in_bins: u32 = hbrs.iter().filter(|(bin, _)| **bin != 1).map(|(_, c)| c).sum();
    assert_eq!(total_failed_in_bins, 2, "both failures categorized");

    let sbrs: BTreeMap<u16, u32> = parsed
        .iter()
        .filter_map(|r| match r {
            Record::Sbr(s) => Some((s.sbin_num, s.sbin_cnt)),
            _ => None,
        })
        .collect();
    let sbr_total: u32 = sbrs.values().sum();
    assert_eq!(sbr_total, 25, "every die has a software bin");

    // Wafer-level counters agree with the bins.
    let wrr = parsed
        .iter()
        .find_map(|r| match r {
            Record::Wrr(w) => Some(w.clone()),
            _ => None,
        })
        .expect("WRR present");
    assert_eq!(wrr.part_cnt, 25);
    assert_eq!(wrr.good_cnt, 23);

    let pcr_total = parsed
        .iter()
        .find_map(|r| match r {
            Record::Pcr(p) if p.site_num == 255 => Some(p.clone()),
            _ => None,
        })
        .expect("site-255 PCR");
    assert_eq!(pcr_total.good_cnt, 23);

    // The wafer map embedded in the GDR matches the fault model.
    let gdr = parsed
        .iter()
        .find_map(|r| match r {
            Record::Gdr(g) => Some(g.clone()),
            _ => None,
        })
        .expect("GDR with wafer map");
    let snapshot = decode_wafer_map_gdr(&gdr).expect("decode GDR map");
    assert_eq!(snapshot.wafer_id, "W01");
    assert_eq!(snapshot.dies.len(), 25);
    for (coord, outcome) in &snapshot.dies {
        let expected = gem_results[coord];
        match outcome {
            Some(tested) => assert_eq!(
                (tested.passed, tested.hard_bin),
                (expected.passed, expected.hard_bin),
                "GDR map bin for {coord}"
            ),
            None => panic!("die {coord} should be tested"),
        }
    }

    // --- Wafer map state consistent with the tally.
    let (tested, passed) = wafer_layout.tested_counts();
    assert_eq!((tested, passed), (25, 23));
    for coord in &expected_failures {
        assert!(matches!(wafer_layout.dies[coord], DieState::Tested(o) if !o.passed));
    }
}
