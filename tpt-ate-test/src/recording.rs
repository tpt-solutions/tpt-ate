//! Wafer-run recording: assembles the STDF V4 record stream for one
//! simulated (or real) wafer-sort run — FAR/ATR/MIR up front, WIR/WCR per
//! wafer, PTR/FTR per die, TSR summaries, WRR/PCR/HBR/SBR counters, MRR
//! last.

use crate::bin_sort::DieTestResult;
use crate::bins::{BinTally, BinTaxonomy};
use crate::patterns::TestProgram;
use crate::stdf::codec::{cpu_type, STDF_VERSION_V4};
use crate::stdf::records::{
    Atr, Bps, Eps, Far, Ftr, Gdr, Hbr, Mir, Mrr, Pcr, Pmr, Ptr, Record, Sbr, Tsr, Wcr, Wir, Wrr,
};
use crate::wafer::{DieState, DieTestOutcome, WaferDieMap};
use std::time::{SystemTime, UNIX_EPOCH};

/// Run identity written into the MIR/ATR records.
#[derive(Debug, Clone)]
pub struct RunMetadata {
    pub node_name: String,
    pub tester_type: String,
    pub operator: String,
    /// Run start (STDF U*4 Unix time).
    pub start_t: u32,
    /// Run finish (STDF U*4 Unix time).
    pub finish_t: u32,
}

impl Default for RunMetadata {
    fn default() -> Self {
        RunMetadata {
            node_name: "TPT-SIM".into(),
            tester_type: "TPT-SIM-ATE".into(),
            operator: "tpt-ate-test".into(),
            start_t: unix_now().saturating_sub(900),
            finish_t: unix_now(),
        }
    }
}

/// Current Unix time as the STDF U*4 timestamp (saturating at u32::MAX).
pub fn unix_now() -> u32 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(u32::MAX)
}

/// Build the full STDF record stream for a completed wafer-sort run.
///
/// `wafer` must already carry the tested results (see
/// [`crate::bin_sort::sort_wafer`]); `results` supplies the per-die
/// PTR/FTR payloads in per-die coordinate order.
pub fn build_wafer_run_records(
    program: &TestProgram,
    wafer: &WaferDieMap,
    results: &[DieTestResult],
    tally: &BinTally,
    taxonomy: &BinTaxonomy,
    metadata: &RunMetadata,
) -> Vec<Record> {
    let (start_t, finish_t) = (metadata.start_t, metadata.finish_t);
    let mut records = Vec::new();

    records.push(Record::Far(Far { cpu_type: cpu_type::X86, stdf_ver: STDF_VERSION_V4 }));
    records.push(Record::Atr(Atr {
        mod_timn: start_t,
        cmd_line: format!("tpt-ate-test wafer sort: {}", program.design_name),
    }));
    records.push(Record::Mir(Mir {
        setup_t: start_t,
        start_t,
        stat_bin: 0,
        mode_cod: b' ',
        rtn_cod: b' ',
        prot_cod: b' ',
        burn_day: 0,
        cmt_cnt: 0,
        lot_id: program.lot_id.clone(),
        part_typ: program.part_type.clone(),
        node_nam: metadata.node_name.clone(),
        tstr_typ: metadata.tester_type.clone(),
        job_nam: program.design_name.clone(),
        job_rev: String::new(),
        sblot_id: wafer.wafer_id.clone(),
        oper_nam: metadata.operator.clone(),
        exec_typ: "tpt-ate-test".into(),
        exec_ver: env!("CARGO_PKG_VERSION").into(),
        test_cod: "W".into(),
        tst_temp: String::new(),
        user_txt: String::new(),
        aux_file: String::new(),
        pkg_typ: String::new(),
        family_id: String::new(),
        date_cod: String::new(),
        facil_id: String::new(),
        floor_id: String::new(),
        proc_id: String::new(),
        oper_frq: String::new(),
        spec_nam: String::new(),
        spec_ver: String::new(),
        flow_id: String::new(),
        setup_id: String::new(),
        dsgn_rev: String::new(),
        eng_id: String::new(),
        rom_cod: String::new(),
        serl_num: String::new(),
        supr_nam: String::new(),
    }));
    records.push(Record::Pmr(Pmr {
        pin_idx: 1,
        chan_typ: 0,
        chan_nam: "SIM-CHAN-1".into(),
        phy_nam: "PHYS-1".into(),
        log_nam: "LOG-1".into(),
        head_num: Some(1),
        site_num: Some(1),
    }));

    records.push(Record::Wir(Wir {
        head_num: 1,
        site_grp: 0,
        start_t,
        wafer_id: wafer.wafer_id.clone(),
    }));
    records.push(Record::Wcr(Wcr {
        wafr_siz: 150.0,
        die_ht: wafer.die_height_mils,
        die_wid: wafer.die_width_mils,
        wf_units: 3, // mils
        wf_flat: 0b100,
        center_x: wafer.center.x as i16,
        center_y: wafer.center.y as i16,
        pos_x: b'R',
        pos_y: b'T',
    }));

    // Per-die test data in coordinate order (deterministic file layout).
    let mut sorted = results.to_vec();
    sorted.sort_by_key(|r| r.coord);
    for result in &sorted {
        records.push(Record::Bps(Bps));
        for measurement in &result.measurements {
            let spec = program.tests.iter().find(|s| s.test_number == measurement.test_number);
            let (lo, hi) = spec
                .map(|s| (s.lo_limit.unwrap_or(0.0), s.hi_limit.unwrap_or(0.0)))
                .unwrap_or((0.0, 0.0));
            let mut ptr = Ptr::measured(
                measurement.test_number,
                1,
                1,
                measurement.value,
                spec.map(|s| s.name.as_str()).unwrap_or(""),
                spec.map(|s| s.unit.as_str()).unwrap_or(""),
                lo,
                hi,
            );
            if !measurement.passed {
                ptr.test_flg |= crate::stdf::records::test_flg::TEST_FAILED;
            }
            records.push(Record::Ptr(ptr));
        }
        for pattern in &result.patterns {
            records.push(Record::Ftr(Ftr::pattern(
                pattern.test_number,
                1,
                1,
                pattern.passed,
                &pattern.name,
            )));
        }
        records.push(Record::Eps(Eps));
    }

    // Per-test synopses over all dies.
    for spec in &program.tests {
        let (exec, fail) = summarize(results, |r| {
            r.measurements.iter().find(|m| m.test_number == spec.test_number).map(|m| m.passed)
        });
        records.push(Record::Tsr(Tsr {
            head_num: 1,
            site_num: 255,
            test_typ: b'P',
            test_num: spec.test_number,
            exec_cnt: exec,
            fail_cnt: fail,
            alarm_cnt: 0,
            test_nam: spec.name.clone(),
            seq_name: String::new(),
            test_lbl: String::new(),
        }));
    }
    for group in &program.pattern_groups {
        let (exec, fail) = summarize(results, |r| {
            r.patterns.iter().find(|p| p.test_number == group.test_number).map(|p| p.passed)
        });
        records.push(Record::Tsr(Tsr {
            head_num: 1,
            site_num: 255,
            test_typ: b'F',
            test_num: group.test_number,
            exec_cnt: exec,
            fail_cnt: fail,
            alarm_cnt: 0,
            test_nam: group.name.clone(),
            seq_name: String::new(),
            test_lbl: String::new(),
        }));
    }

    let (tested, passed) = wafer.tested_counts();
    records.push(Record::Wrr(Wrr {
        head_num: 1,
        site_grp: 0,
        finish_t,
        part_cnt: tested,
        rtst_cnt: 0,
        abrt_cnt: tally.aborted,
        good_cnt: passed,
        func_cnt: 0,
        wafer_id: wafer.wafer_id.clone(),
        fabwf_id: String::new(),
        frame_id: String::new(),
        mask_id: String::new(),
        usr_desc: String::new(),
        exc_desc: String::new(),
    }));

    // Site 1 counters (the simulated tester is single-site) and the
    // site-255 totals.
    records.push(Record::Pcr(Pcr {
        head_num: 1,
        site_num: 1,
        part_cnt: tested,
        rtst_cnt: 0,
        abrt_cnt: tally.aborted,
        good_cnt: passed,
        func_cnt: 0,
    }));
    records.push(Record::Pcr(Pcr {
        head_num: 1,
        site_num: 255,
        part_cnt: tested,
        rtst_cnt: 0,
        abrt_cnt: tally.aborted,
        good_cnt: passed,
        func_cnt: 0,
    }));

    for (number, count) in &tally.hard_bins {
        let definition = taxonomy.hard_bins.get(number);
        records.push(Record::Hbr(Hbr {
            head_num: 1,
            site_num: 255,
            hbin_num: *number,
            hbin_cnt: *count,
            hbin_pf: definition.map_or(b'F', |b| if b.pass { b'P' } else { b'F' }),
            hbin_nam: definition.map(|b| b.name.clone()).unwrap_or_default(),
        }));
    }
    for (number, count) in &tally.soft_bins {
        let definition = taxonomy.soft_bins.get(number);
        records.push(Record::Sbr(Sbr {
            head_num: 1,
            site_num: 255,
            sbin_num: *number,
            sbin_cnt: *count,
            sbin_pf: definition.map_or(b'F', |b| {
                if taxonomy.is_pass_hard_bin(b.hard_bin) {
                    b'P'
                } else {
                    b'F'
                }
            }),
            sbin_nam: definition.map(|b| b.name.clone()).unwrap_or_default(),
        }));
    }

    // Generic data: the die-coordinate -> (hard bin, soft bin) map, so the
    // wafer map survives inside the STDF itself (little-endian i32 pairs).
    records.push(Record::Gdr(Gdr { gen_data: encode_wafer_map_gdr(wafer) }));

    records.push(Record::Mrr(Mrr {
        finish_t,
        disp_cod: b' ',
        usr_cod: 0,
        exc_cod: 0,
        usr_desc: String::new(),
        exc_desc: String::new(),
    }));

    records
}

fn summarize(
    results: &[DieTestResult],
    pick: impl Fn(&DieTestResult) -> Option<bool>,
) -> (u32, u32) {
    let mut exec = 0;
    let mut fail = 0;
    for result in results {
        if let Some(passed) = pick(result) {
            exec += 1;
            fail += !passed as u32;
        }
    }
    (exec, fail)
}

/// GDR payload: wafer ID length + ID, then per die (in coordinate order)
/// x:i32, y:i32, hard_bin:u16, soft_bin:u16, passed:u8 — all little-endian.
/// This is how the wafer map rides inside the STDF file itself.
fn encode_wafer_map_gdr(wafer: &WaferDieMap) -> Vec<u8> {
    let mut out = Vec::new();
    let id = wafer.wafer_id.as_bytes();
    out.push(id.len() as u8);
    out.extend_from_slice(id);
    let mut coords: Vec<_> = wafer.dies.iter().collect();
    coords.sort_by_key(|(coord, _)| **coord);
    for (coord, state) in coords {
        out.extend_from_slice(&coord.x.to_le_bytes());
        out.extend_from_slice(&coord.y.to_le_bytes());
        if let DieState::Tested(outcome) = state {
            out.extend_from_slice(&outcome.hard_bin.to_le_bytes());
            out.extend_from_slice(&outcome.soft_bin.to_le_bytes());
            out.push(outcome.passed as u8);
        } else {
            out.extend_from_slice(&[0, 0, 0, 0, 0]);
        }
    }
    out
}

/// The wafer map as decoded from the GDR payload: wafer ID plus per-die
/// (coordinate, tested-outcome) entries in coordinate order.
#[derive(Debug, Clone, PartialEq)]
pub struct WaferMapSnapshot {
    pub wafer_id: String,
    pub dies: Vec<(crate::wafer::DieCoord, Option<DieTestOutcome>)>,
}

/// Decode the GDR payload written by [`encode_wafer_map_gdr`].
pub fn decode_wafer_map_gdr(gdr: &Gdr) -> Option<WaferMapSnapshot> {
    let data = &gdr.gen_data;
    let id_len = *data.first()? as usize;
    let id = String::from_utf8_lossy(&data[1..1 + id_len]).into_owned();
    let mut rest = &data[1 + id_len..];
    let mut dies = Vec::new();
    const ENTRY: usize = 4 + 4 + 2 + 2 + 1;
    while rest.len() >= ENTRY {
        let x = i32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
        let y = i32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]);
        let hard = u16::from_le_bytes([rest[8], rest[9]]);
        let soft = u16::from_le_bytes([rest[10], rest[11]]);
        let passed = rest[12] != 0;
        let tested = if hard == 0 && soft == 0 && !passed {
            None
        } else {
            Some(DieTestOutcome { passed, hard_bin: hard, soft_bin: soft })
        };
        dies.push((crate::wafer::DieCoord::new(x, y), tested));
        rest = &rest[ENTRY..];
    }
    Some(WaferMapSnapshot { wafer_id: id, dies })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bin_sort::{sort_wafer, tally as make_tally};
    use crate::sim::{FaultModel, SimulatedTester};
    use crate::wafer::DieCoord;

    fn program() -> TestProgram {
        crate::patterns::TestProgram {
            design_name: "chiplet-a1".into(),
            lot_id: "LOT-1".into(),
            part_type: "CHIPLET-A1".into(),
            sblot_id: "W01".into(),
            tests: vec![crate::patterns::TestSpec {
                test_number: 10,
                name: "IDDQ".into(),
                unit: "uA".into(),
                nominal: 1.0,
                lo_limit: Some(0.0),
                hi_limit: Some(2.0),
                fail_soft_bin: 0,
            }],
            pattern_groups: vec![crate::patterns::PatternGroup {
                name: "ATPG_SCAN".into(),
                vector_count: 256,
                test_number: 20,
                fail_soft_bin: 0,
            }],
        }
    }

    #[test]
    fn gdr_roundtrip() {
        let mut wafer = WaferDieMap::rectangular("W7", 2, 1, 100.0, 120.0);
        let tester = SimulatedTester::new(program(), FaultModel::deterministic(5, vec![]));
        let results: Vec<DieTestResult> = wafer
            .dies
            .keys()
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .map(|c| tester.test_die(c))
            .collect();
        sort_wafer(&results, &mut wafer, tester.program(), &BinTaxonomy::defaults());
        let gdr = Gdr { gen_data: encode_wafer_map_gdr(&wafer) };
        let snapshot = decode_wafer_map_gdr(&gdr).expect("decode");
        assert_eq!(snapshot.wafer_id, "W7");
        assert_eq!(snapshot.dies.len(), 2);
        assert!(snapshot.dies.iter().all(|(_, outcome)| outcome.is_some()));
    }

    #[test]
    fn full_record_stream_is_ordered_and_complete() {
        let mut wafer = WaferDieMap::rectangular("W1", 3, 3, 90.0, 90.0);
        let tester = SimulatedTester::new(
            program(),
            FaultModel::deterministic(9, vec![DieCoord::new(2, 2)]),
        );
        let results: Vec<DieTestResult> = wafer
            .dies
            .keys()
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .map(|c| tester.test_die(c))
            .collect();
        let tally = make_tally(&results, tester.program(), &BinTaxonomy::defaults());
        sort_wafer(&results, &mut wafer, tester.program(), &BinTaxonomy::defaults());
        let records = build_wafer_run_records(
            tester.program(),
            &wafer,
            &results,
            &tally,
            &BinTaxonomy::defaults(),
            &RunMetadata {
                start_t: 1_700_000_000,
                finish_t: 1_700_000_100,
                ..RunMetadata::default()
            },
        );
        assert!(matches!(records[0], Record::Far(_)));
        assert!(matches!(records[2], Record::Mir(_)));
        assert!(matches!(records.last(), Some(Record::Mrr(_))));
        // 9 dies x (1 PTR + 1 FTR) + everything else.
        let ptr_count = records.iter().filter(|r| matches!(r, Record::Ptr(_))).count();
        assert_eq!(ptr_count, 9);
        // One failing die at (2,2).
        let failed_ptrs =
            records.iter().filter(|r| matches!(r, Record::Ptr(p) if p.failed())).count();
        assert_eq!(failed_ptrs, 1);
    }
}
