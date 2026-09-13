//! The STDF V4 record set this crate reads and writes, per the STDF
//! Specification V4 (Teradyne-published industry standard).
//!
//! Scope: the core production-flow record types — FAR, ATR, MIR, MRR, PCR,
//! HBR, SBR, PMR, WIR, WRR, WCR, PTR, FTR, TSR, GDR, DTR, BPS, EPS. Records
//! this crate does not model are preserved verbatim as
//! [`Record::Unknown`] (type/subtype + raw body), so a read→write pass is
//! byte-lossless even for files containing unsupported records. Fields whose
//! presence depends on flag bits (e.g. PTR's optional limit fields) follow
//! the V4 conditional-presence rules; any bytes left after a record's known
//! fields are kept as a tail and re-emitted on write.

use crate::stdf::codec::{write, Endian, Reader};
use crate::stdf::error::Result;

/// TEST_FLG bit meanings that are stable across the field (STDF V4, PTR).
pub mod test_flg {
    /// Test failed.
    pub const TEST_FAILED: u8 = 0x01;
    /// Data unreliable.
    pub const DATA_UNRELIABLE: u8 = 0x04;
    /// `RESULT` carries a pass/fail verdict rather than a measurement.
    pub const PASS_FAIL_FIELD: u8 = 0x08;
    /// Test not executed.
    pub const NOT_EXECUTED: u8 = 0x40;
}

/// PARM_FLG bit meanings (STDF V4, PTR).
pub mod parm_flg {
    /// `OPT_FLAG` and the optional scaling/limit fields are present.
    pub const OPT_FLAG_PRESENT: u8 = 0x10;
}

/// OPT_FLAG bit meanings (STDF V4, PTR).
pub mod opt_flg {
    /// `RES_SCAL` is not present.
    pub const NO_RES_SCAL: u8 = 0x01;
    /// `LLM_SCAL` is not present.
    pub const NO_LLM_SCAL: u8 = 0x02;
    /// `HLM_SCAL` is not present.
    pub const NO_HLM_SCAL: u8 = 0x04;
    /// `LO_LIMIT` is not present.
    pub const NO_LO_LIMIT: u8 = 0x08;
    /// `HI_LIMIT` is not present.
    pub const NO_HI_LIMIT: u8 = 0x10;
}

/// Final Assembly and Test flow identification (functional sources: the
/// operator/system that produced the file).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Far {
    pub cpu_type: u8,
    pub stdf_ver: u8,
}

/// Audit Trail Record.
#[derive(Debug, Clone, PartialEq)]
pub struct Atr {
    pub mod_timn: u32,
    pub cmd_line: String,
}

/// Master Information Record: identifies the lot, part, tester, and test job.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mir {
    pub setup_t: u32,
    pub start_t: u32,
    pub stat_bin: u8,
    pub mode_cod: u8,
    pub rtn_cod: u8,
    pub prot_cod: u8,
    pub burn_day: u16,
    pub cmt_cnt: u8,
    pub lot_id: String,
    pub part_typ: String,
    pub node_nam: String,
    pub tstr_typ: String,
    pub job_nam: String,
    pub job_rev: String,
    pub sblot_id: String,
    pub oper_nam: String,
    pub exec_typ: String,
    pub exec_ver: String,
    pub test_cod: String,
    pub tst_temp: String,
    pub user_txt: String,
    pub aux_file: String,
    pub pkg_typ: String,
    pub family_id: String,
    pub date_cod: String,
    pub facil_id: String,
    pub floor_id: String,
    pub proc_id: String,
    pub oper_frq: String,
    pub spec_nam: String,
    pub spec_ver: String,
    pub flow_id: String,
    pub setup_id: String,
    pub dsgn_rev: String,
    pub eng_id: String,
    pub rom_cod: String,
    pub serl_num: String,
    pub supr_nam: String,
}

/// Master Results Record: lot-level counters, written last.
#[derive(Debug, Clone, PartialEq)]
pub struct Mrr {
    pub finish_t: u32,
    pub disp_cod: u8,
    pub usr_cod: u8,
    pub exc_cod: u8,
    pub usr_desc: String,
    pub exc_desc: String,
}

/// Part Results Record: per-site cumulative part counters.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcr {
    pub head_num: u8,
    pub site_num: u8,
    pub part_cnt: u32,
    pub rtst_cnt: u32,
    pub abrt_cnt: u32,
    pub good_cnt: u32,
    pub func_cnt: u32,
}

/// Hardware Bin Record.
#[derive(Debug, Clone, PartialEq)]
pub struct Hbr {
    pub head_num: u8,
    pub site_num: u8,
    pub hbin_num: u16,
    pub hbin_cnt: u32,
    /// 'P' or 'F'.
    pub hbin_pf: u8,
    pub hbin_nam: String,
}

/// Software Bin Record.
#[derive(Debug, Clone, PartialEq)]
pub struct Sbr {
    pub head_num: u8,
    pub site_num: u8,
    pub sbin_num: u16,
    pub sbin_cnt: u32,
    /// 'P' or 'F'.
    pub sbin_pf: u8,
    pub sbin_nam: String,
}

/// Pin Map Record.
#[derive(Debug, Clone, PartialEq)]
pub struct Pmr {
    pub pin_idx: u16,
    pub chan_typ: u16,
    pub chan_nam: String,
    pub phy_nam: String,
    pub log_nam: String,
    pub head_num: Option<u8>,
    pub site_num: Option<u8>,
}

/// Wafer Information Record: start of wafer testing.
#[derive(Debug, Clone, PartialEq)]
pub struct Wir {
    pub head_num: u8,
    pub site_grp: u8,
    pub start_t: u32,
    pub wafer_id: String,
}

/// Wafer Results Record: end of wafer testing.
#[derive(Debug, Clone, PartialEq)]
pub struct Wrr {
    pub head_num: u8,
    pub site_grp: u8,
    pub finish_t: u32,
    pub part_cnt: u32,
    pub rtst_cnt: u32,
    pub abrt_cnt: u32,
    pub good_cnt: u32,
    pub func_cnt: u32,
    pub wafer_id: String,
    pub fabwf_id: String,
    pub frame_id: String,
    pub mask_id: String,
    pub usr_desc: String,
    pub exc_desc: String,
}

/// Wafer Configuration Record: wafer geometry and die coordinate system.
#[derive(Debug, Clone, PartialEq)]
pub struct Wcr {
    pub wafr_siz: f32,
    pub die_ht: f32,
    pub die_wid: f32,
    /// Units for the three fields above (0 = inches, 1 = cm, 2 = mm, 3 = mil).
    pub wf_units: u8,
    /// WF_FLAT bit field: bits 0-1 = flat location (0=bottom, 1=top, 2=left,
    /// 3=right), bit 2 = flat located.
    pub wf_flat: u8,
    pub center_x: i16,
    pub center_y: i16,
    /// Die X axis direction ('L', 'R', or '?').
    pub pos_x: u8,
    /// Die Y axis direction ('T', 'B', or '?').
    pub pos_y: u8,
}

/// Parametric Test Record: one analog/numeric test result.
#[derive(Debug, Clone, PartialEq)]
pub struct Ptr {
    pub test_num: u32,
    pub head_num: u8,
    pub site_num: u8,
    pub test_flg: u8,
    pub parm_flg: u8,
    pub result: f32,
    pub test_txt: String,
    pub alarm_id: String,
    pub opt_flg: Option<u8>,
    pub res_scal: Option<i8>,
    pub llm_scal: Option<i8>,
    pub hlm_scal: Option<i8>,
    pub lo_limit: Option<f32>,
    pub hi_limit: Option<f32>,
    pub units: String,
}

impl Ptr {
    /// Build a numeric measurement PTR with the standard optional fields
    /// present (scaling on limits, both limits carried).
    // Argument list mirrors the STDF PTR field order; bundling would hide
    // the record mapping.
    #[allow(clippy::too_many_arguments)]
    pub fn measured(
        test_num: u32,
        head_num: u8,
        site_num: u8,
        result: f32,
        name: &str,
        units: &str,
        lo: f32,
        hi: f32,
    ) -> Ptr {
        Ptr {
            test_num,
            head_num,
            site_num,
            test_flg: 0,
            parm_flg: parm_flg::OPT_FLAG_PRESENT,
            result,
            test_txt: name.to_string(),
            alarm_id: String::new(),
            opt_flg: Some(0),
            res_scal: Some(0),
            llm_scal: Some(0),
            hlm_scal: Some(0),
            lo_limit: Some(lo),
            hi_limit: Some(hi),
            units: units.to_string(),
        }
    }

    /// Build a pass/fail-verdict PTR (`RESULT` is the verdict, not a value).
    pub fn verdict(test_num: u32, head_num: u8, site_num: u8, passed: bool) -> Ptr {
        Ptr {
            test_flg: test_flg::PASS_FAIL_FIELD | if passed { 0 } else { test_flg::TEST_FAILED },
            ..Ptr::measured(
                test_num,
                head_num,
                site_num,
                if passed { 1.0 } else { 0.0 },
                "",
                "",
                0.0,
                1.0,
            )
        }
    }

    /// Did this test fail? (Bit meaningful only when `PASS_FAIL_FIELD` set.)
    pub fn failed(&self) -> bool {
        self.test_flg & test_flg::TEST_FAILED != 0
    }
}

/// Functional Test Record: one pattern-group execution result.
#[derive(Debug, Clone, PartialEq)]
pub struct Ftr {
    pub test_num: u32,
    pub head_num: u8,
    pub site_num: u8,
    pub test_flg: u8,
    pub opt_flg: u8,
    pub cycl_cnt: u32,
    pub rel_vpos: u32,
    pub rept_cnt: u32,
    pub num_fail: u32,
    pub x_fail_adj: i16,
    pub y_fail_adj: i16,
    pub vect_offset: i16,
    pub rtn_icq: u16,
    pub rtn_ilim: u16,
    pub pgm_icq: u16,
    pub pgm_ilim: u16,
    pub rtn_nam: Vec<String>,
    pub pgm_nam: Vec<String>,
    pub alarm_id: Option<String>,
    pub opt_flg_2: Option<u8>,
    pub program: Option<String>,
    pub test_label: Option<String>,
}

impl Ftr {
    /// Build a minimal functional result for a pattern group.
    pub fn pattern(
        test_num: u32,
        head_num: u8,
        site_num: u8,
        passed: bool,
        pattern_name: &str,
    ) -> Ftr {
        Ftr {
            test_num,
            head_num,
            site_num,
            test_flg: test_flg::PASS_FAIL_FIELD | if passed { 0 } else { test_flg::TEST_FAILED },
            opt_flg: 0,
            cycl_cnt: 0,
            rel_vpos: 0,
            rept_cnt: 0,
            num_fail: if passed { 0 } else { 1 },
            x_fail_adj: 0,
            y_fail_adj: 0,
            vect_offset: 0,
            rtn_icq: 0,
            rtn_ilim: 0,
            pgm_icq: 0,
            pgm_ilim: 0,
            rtn_nam: Vec::new(),
            pgm_nam: vec![pattern_name.to_string()],
            alarm_id: None,
            opt_flg_2: None,
            program: None,
            test_label: None,
        }
    }

    pub fn failed(&self) -> bool {
        self.test_flg & test_flg::TEST_FAILED != 0
    }
}

/// Test Synopsis Record: per-test aggregate counters.
#[derive(Debug, Clone, PartialEq)]
pub struct Tsr {
    pub head_num: u8,
    pub site_num: u8,
    /// 'P' parametric, 'F' functional, 'M' multiple-mode.
    pub test_typ: u8,
    pub test_num: u32,
    pub exec_cnt: u32,
    pub fail_cnt: u32,
    pub alarm_cnt: u32,
    pub test_nam: String,
    pub seq_name: String,
    pub test_lbl: String,
}

/// Generic Data Record: opaque N-byte data blob.
#[derive(Debug, Clone, PartialEq)]
pub struct Gdr {
    pub gen_data: Vec<u8>,
}

/// Datalog Text Record: free-form text.
#[derive(Debug, Clone, PartialEq)]
pub struct Dtr {
    pub text_dat: String,
}

/// Begin Program Section marker (no data fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bps;

/// End Program Section marker (no data fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Eps;

/// One parsed STDF record. Unsupported record types survive as `Unknown`.
// MIR carries 30 spec-mandated string fields, dwarfing the empty markers;
// records are transient values, so boxing MIR would only add churn.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    Far(Far),
    Atr(Atr),
    Mir(Mir),
    Mrr(Mrr),
    Pcr(Pcr),
    Hbr(Hbr),
    Sbr(Sbr),
    Pmr(Pmr),
    Wir(Wir),
    Wrr(Wrr),
    Wcr(Wcr),
    Ptr(Ptr),
    Ftr(Ftr),
    Tsr(Tsr),
    Gdr(Gdr),
    Dtr(Dtr),
    Bps(Bps),
    Eps(Eps),
    /// A record this crate does not model, preserved verbatim.
    Unknown {
        typ: u8,
        sub: u8,
        data: Vec<u8>,
    },
}

impl Record {
    /// The (REC_TYP, REC_SUB) pair for this record.
    pub fn type_subtype(&self) -> (u8, u8) {
        match self {
            Record::Far(_) => (0, 10),
            Record::Atr(_) => (0, 20),
            Record::Mir(_) => (1, 10),
            Record::Mrr(_) => (1, 20),
            Record::Pcr(_) => (1, 30),
            Record::Hbr(_) => (1, 40),
            Record::Sbr(_) => (1, 50),
            Record::Pmr(_) => (5, 10),
            Record::Wir(_) => (2, 10),
            Record::Wrr(_) => (2, 20),
            Record::Wcr(_) => (2, 30),
            Record::Ptr(_) => (15, 10),
            Record::Ftr(_) => (15, 20),
            Record::Tsr(_) => (5, 30),
            Record::Gdr(_) => (20, 10),
            Record::Dtr(_) => (20, 30),
            Record::Bps(_) => (20, 15),
            Record::Eps(_) => (20, 20),
            Record::Unknown { typ, sub, .. } => (*typ, *sub),
        }
    }

    /// Encode: REC_LEN + REC_TYP + REC_SUB + body (including any preserved
    /// tail for partially-modeled records).
    pub fn encode(&self, out: &mut Vec<u8>) {
        let (typ, sub) = self.type_subtype();
        let mut body = Vec::new();
        match self {
            Record::Far(r) => {
                write::u1(&mut body, r.cpu_type);
                write::u1(&mut body, r.stdf_ver);
            }
            Record::Atr(r) => {
                write::u4(&mut body, r.mod_timn);
                write::cn(&mut body, &r.cmd_line);
            }
            Record::Mir(r) => {
                write::u4(&mut body, r.setup_t);
                write::u4(&mut body, r.start_t);
                write::u1(&mut body, r.stat_bin);
                write::u1(&mut body, r.mode_cod);
                write::u1(&mut body, r.rtn_cod);
                write::u1(&mut body, r.prot_cod);
                write::u2(&mut body, r.burn_day);
                write::u1(&mut body, r.cmt_cnt);
                for field in [
                    &r.lot_id,
                    &r.part_typ,
                    &r.node_nam,
                    &r.tstr_typ,
                    &r.job_nam,
                    &r.job_rev,
                    &r.sblot_id,
                    &r.oper_nam,
                    &r.exec_typ,
                    &r.exec_ver,
                    &r.test_cod,
                    &r.tst_temp,
                    &r.user_txt,
                    &r.aux_file,
                    &r.pkg_typ,
                    &r.family_id,
                    &r.date_cod,
                    &r.facil_id,
                    &r.floor_id,
                    &r.proc_id,
                    &r.oper_frq,
                    &r.spec_nam,
                    &r.spec_ver,
                    &r.flow_id,
                    &r.setup_id,
                    &r.dsgn_rev,
                    &r.eng_id,
                    &r.rom_cod,
                    &r.serl_num,
                    &r.supr_nam,
                ] {
                    write::cn(&mut body, field);
                }
            }
            Record::Mrr(r) => {
                write::u4(&mut body, r.finish_t);
                write::u1(&mut body, r.disp_cod);
                write::u1(&mut body, r.usr_cod);
                write::u1(&mut body, r.exc_cod);
                write::cn(&mut body, &r.usr_desc);
                write::cn(&mut body, &r.exc_desc);
            }
            Record::Pcr(r) => {
                write::u1(&mut body, r.head_num);
                write::u1(&mut body, r.site_num);
                write::u4(&mut body, r.part_cnt);
                write::u4(&mut body, r.rtst_cnt);
                write::u4(&mut body, r.abrt_cnt);
                write::u4(&mut body, r.good_cnt);
                write::u4(&mut body, r.func_cnt);
            }
            Record::Hbr(r) => {
                write::u1(&mut body, r.head_num);
                write::u1(&mut body, r.site_num);
                write::u2(&mut body, r.hbin_num);
                write::u4(&mut body, r.hbin_cnt);
                write::u1(&mut body, r.hbin_pf);
                write::cn(&mut body, &r.hbin_nam);
            }
            Record::Sbr(r) => {
                write::u1(&mut body, r.head_num);
                write::u1(&mut body, r.site_num);
                write::u2(&mut body, r.sbin_num);
                write::u4(&mut body, r.sbin_cnt);
                write::u1(&mut body, r.sbin_pf);
                write::cn(&mut body, &r.sbin_nam);
            }
            Record::Pmr(r) => {
                write::u2(&mut body, r.pin_idx);
                write::u2(&mut body, r.chan_typ);
                write::cn(&mut body, &r.chan_nam);
                write::cn(&mut body, &r.phy_nam);
                write::cn(&mut body, &r.log_nam);
                if let (Some(head), Some(site)) = (r.head_num, r.site_num) {
                    write::u1(&mut body, head);
                    write::u1(&mut body, site);
                }
            }
            Record::Wir(r) => {
                write::u1(&mut body, r.head_num);
                write::u1(&mut body, r.site_grp);
                write::u4(&mut body, r.start_t);
                write::cn(&mut body, &r.wafer_id);
            }
            Record::Wrr(r) => {
                write::u1(&mut body, r.head_num);
                write::u1(&mut body, r.site_grp);
                write::u4(&mut body, r.finish_t);
                write::u4(&mut body, r.part_cnt);
                write::u4(&mut body, r.rtst_cnt);
                write::u4(&mut body, r.abrt_cnt);
                write::u4(&mut body, r.good_cnt);
                write::u4(&mut body, r.func_cnt);
                write::cn(&mut body, &r.wafer_id);
                write::cn(&mut body, &r.fabwf_id);
                write::cn(&mut body, &r.frame_id);
                write::cn(&mut body, &r.mask_id);
                write::cn(&mut body, &r.usr_desc);
                write::cn(&mut body, &r.exc_desc);
            }
            Record::Wcr(r) => {
                write::r4(&mut body, r.wafr_siz);
                write::r4(&mut body, r.die_ht);
                write::r4(&mut body, r.die_wid);
                write::u1(&mut body, r.wf_units);
                write::u1(&mut body, r.wf_flat);
                write::i2(&mut body, r.center_x);
                write::i2(&mut body, r.center_y);
                write::u1(&mut body, r.pos_x);
                write::u1(&mut body, r.pos_y);
            }
            Record::Ptr(r) => encode_ptr(r, &mut body),
            Record::Ftr(r) => encode_ftr(r, &mut body),
            Record::Tsr(r) => {
                write::u1(&mut body, r.head_num);
                write::u1(&mut body, r.site_num);
                write::u1(&mut body, r.test_typ);
                write::u4(&mut body, r.test_num);
                write::u4(&mut body, r.exec_cnt);
                write::u4(&mut body, r.fail_cnt);
                write::u4(&mut body, r.alarm_cnt);
                write::cn(&mut body, &r.test_nam);
                write::cn(&mut body, &r.seq_name);
                write::cn(&mut body, &r.test_lbl);
            }
            Record::Gdr(r) => write::k_bytes(&mut body, &r.gen_data),
            Record::Dtr(r) => write::cn(&mut body, &r.text_dat),
            Record::Bps(_) | Record::Eps(_) => {}
            Record::Unknown { data, .. } => body.extend_from_slice(data),
        }
        write::u2(out, body.len() as u16);
        write::u1(out, typ);
        write::u1(out, sub);
        out.extend_from_slice(&body);
    }

    /// Encode to a fresh `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }

    /// Parse one record body given its (typ, sub). `endian` applies to the
    /// fixed-width numeric fields. Unknown types round-trip as
    /// [`Record::Unknown`]; known types with unparsed trailing bytes keep
    /// those bytes in the record's tail.
    pub fn parse(typ: u8, sub: u8, data: &[u8], endian: Endian) -> Result<Record> {
        let mut r = Reader::new(data, endian);
        let record = match (typ, sub) {
            (0, 10) => Record::Far(Far { cpu_type: r.u1()?, stdf_ver: r.u1()? }),
            (0, 20) => Record::Atr(Atr { mod_timn: r.u4()?, cmd_line: r.cn()? }),
            (1, 10) => Record::Mir(Mir {
                setup_t: r.u4()?,
                start_t: r.u4()?,
                stat_bin: r.u1()?,
                mode_cod: r.u1()?,
                rtn_cod: r.u1()?,
                prot_cod: r.u1()?,
                burn_day: r.u2()?,
                cmt_cnt: r.u1()?,
                lot_id: r.cn()?,
                part_typ: r.cn()?,
                node_nam: r.cn()?,
                tstr_typ: r.cn()?,
                job_nam: r.cn()?,
                job_rev: r.cn()?,
                sblot_id: r.cn()?,
                oper_nam: r.cn()?,
                exec_typ: r.cn()?,
                exec_ver: r.cn()?,
                test_cod: r.cn()?,
                tst_temp: r.cn()?,
                user_txt: r.cn()?,
                aux_file: r.cn()?,
                pkg_typ: r.cn()?,
                family_id: r.cn()?,
                date_cod: r.cn()?,
                facil_id: r.cn()?,
                floor_id: r.cn()?,
                proc_id: r.cn()?,
                oper_frq: r.cn()?,
                spec_nam: r.cn()?,
                spec_ver: r.cn()?,
                flow_id: r.cn()?,
                setup_id: r.cn()?,
                dsgn_rev: r.cn()?,
                eng_id: r.cn()?,
                rom_cod: r.cn()?,
                serl_num: r.cn()?,
                supr_nam: r.cn()?,
            }),
            (1, 20) => Record::Mrr(Mrr {
                finish_t: r.u4()?,
                disp_cod: r.u1()?,
                usr_cod: r.u1()?,
                exc_cod: r.u1()?,
                usr_desc: r.cn()?,
                exc_desc: r.cn()?,
            }),
            (1, 30) => Record::Pcr(Pcr {
                head_num: r.u1()?,
                site_num: r.u1()?,
                part_cnt: r.u4()?,
                rtst_cnt: r.u4()?,
                abrt_cnt: r.u4()?,
                good_cnt: r.u4()?,
                func_cnt: r.u4()?,
            }),
            (1, 40) => Record::Hbr(Hbr {
                head_num: r.u1()?,
                site_num: r.u1()?,
                hbin_num: r.u2()?,
                hbin_cnt: r.u4()?,
                hbin_pf: r.u1()?,
                hbin_nam: r.cn()?,
            }),
            (1, 50) => Record::Sbr(Sbr {
                head_num: r.u1()?,
                site_num: r.u1()?,
                sbin_num: r.u2()?,
                sbin_cnt: r.u4()?,
                sbin_pf: r.u1()?,
                sbin_nam: r.cn()?,
            }),
            (5, 10) => {
                let pin_idx = r.u2()?;
                let chan_typ = r.u2()?;
                let chan_nam = r.cn()?;
                let phy_nam = r.cn()?;
                let log_nam = r.cn()?;
                let (head_num, site_num) =
                    if !r.is_empty() { (Some(r.u1()?), Some(r.u1()?)) } else { (None, None) };
                Record::Pmr(Pmr {
                    pin_idx,
                    chan_typ,
                    chan_nam,
                    phy_nam,
                    log_nam,
                    head_num,
                    site_num,
                })
            }
            (2, 10) => Record::Wir(Wir {
                head_num: r.u1()?,
                site_grp: r.u1()?,
                start_t: r.u4()?,
                wafer_id: r.cn()?,
            }),
            (2, 20) => {
                let head_num = r.u1()?;
                let site_grp = r.u1()?;
                let finish_t = r.u4()?;
                let part_cnt = r.u4()?;
                let rtst_cnt = r.u4()?;
                let abrt_cnt = r.u4()?;
                let good_cnt = r.u4()?;
                let func_cnt = r.u4()?;
                let wafer_id = r.cn()?;
                let fabwf_id = if !r.is_empty() { r.cn()? } else { String::new() };
                let frame_id = if !r.is_empty() { r.cn()? } else { String::new() };
                let mask_id = if !r.is_empty() { r.cn()? } else { String::new() };
                let usr_desc = if !r.is_empty() { r.cn()? } else { String::new() };
                let exc_desc = if !r.is_empty() { r.cn()? } else { String::new() };
                Record::Wrr(Wrr {
                    head_num,
                    site_grp,
                    finish_t,
                    part_cnt,
                    rtst_cnt,
                    abrt_cnt,
                    good_cnt,
                    func_cnt,
                    wafer_id,
                    fabwf_id,
                    frame_id,
                    mask_id,
                    usr_desc,
                    exc_desc,
                })
            }
            (2, 30) => Record::Wcr(Wcr {
                wafr_siz: r.r4()?,
                die_ht: r.r4()?,
                die_wid: r.r4()?,
                wf_units: r.u1()?,
                wf_flat: r.u1()?,
                center_x: r.i2()?,
                center_y: r.i2()?,
                pos_x: r.u1()?,
                pos_y: r.u1()?,
            }),
            (15, 10) => {
                let test_num = r.u4()?;
                let head_num = r.u1()?;
                let site_num = r.u1()?;
                let test_flg = r.u1()?;
                let parm_flg = r.u1()?;
                let result = r.r4()?;
                let test_txt = r.cn()?;
                let alarm_id = r.cn()?;
                let opt_flg = if parm_flg & parm_flg::OPT_FLAG_PRESENT != 0 && !r.is_empty() {
                    Some(r.u1()?)
                } else {
                    None
                };
                let flags = opt_flg.unwrap_or(0);
                let res_scal =
                    if opt_flg.is_some() && flags & opt_flg::NO_RES_SCAL == 0 && !r.is_empty() {
                        Some(r.i1()?)
                    } else {
                        None
                    };
                let llm_scal =
                    if opt_flg.is_some() && flags & opt_flg::NO_LLM_SCAL == 0 && !r.is_empty() {
                        Some(r.i1()?)
                    } else {
                        None
                    };
                let hlm_scal =
                    if opt_flg.is_some() && flags & opt_flg::NO_HLM_SCAL == 0 && !r.is_empty() {
                        Some(r.i1()?)
                    } else {
                        None
                    };
                let lo_limit =
                    if opt_flg.is_some() && flags & opt_flg::NO_LO_LIMIT == 0 && !r.is_empty() {
                        Some(r.r4()?)
                    } else {
                        None
                    };
                let hi_limit =
                    if opt_flg.is_some() && flags & opt_flg::NO_HI_LIMIT == 0 && !r.is_empty() {
                        Some(r.r4()?)
                    } else {
                        None
                    };
                let units = if !r.is_empty() { r.cn()? } else { String::new() };
                Record::Ptr(Ptr {
                    test_num,
                    head_num,
                    site_num,
                    test_flg,
                    parm_flg,
                    result,
                    test_txt,
                    alarm_id,
                    opt_flg,
                    res_scal,
                    llm_scal,
                    hlm_scal,
                    lo_limit,
                    hi_limit,
                    units,
                })
            }
            (15, 20) => {
                let test_num = r.u4()?;
                let head_num = r.u1()?;
                let site_num = r.u1()?;
                let test_flg = r.u1()?;
                let opt_flg = r.u1()?;
                let cycl_cnt = r.u4()?;
                let rel_vpos = r.u4()?;
                let rept_cnt = r.u4()?;
                let num_fail = r.u4()?;
                let x_fail_adj = r.i2()?;
                let y_fail_adj = r.i2()?;
                let vect_offset = r.i2()?;
                let rtn_icq = r.u2()?;
                let rtn_ilim = r.u2()?;
                let pgm_icq = r.u2()?;
                let pgm_ilim = r.u2()?;
                let rtn_nam = r.sn()?;
                let pgm_nam = r.sn()?;
                let (alarm_id, opt_flg_2, program, test_label) = if opt_flg & 0x01 != 0
                    && !r.is_empty()
                {
                    let alarm_id = r.cn()?;
                    let opt_flg_2 = if !r.is_empty() { r.u1()? } else { 0 };
                    let program =
                        if opt_flg_2 & 0x02 == 0 && !r.is_empty() { Some(r.nn()?) } else { None };
                    let test_label =
                        if opt_flg_2 & 0x04 == 0 && !r.is_empty() { Some(r.nn()?) } else { None };
                    (Some(alarm_id), Some(opt_flg_2), program, test_label)
                } else {
                    (None, None, None, None)
                };
                Record::Ftr(Ftr {
                    test_num,
                    head_num,
                    site_num,
                    test_flg,
                    opt_flg,
                    cycl_cnt,
                    rel_vpos,
                    rept_cnt,
                    num_fail,
                    x_fail_adj,
                    y_fail_adj,
                    vect_offset,
                    rtn_icq,
                    rtn_ilim,
                    pgm_icq,
                    pgm_ilim,
                    rtn_nam,
                    pgm_nam,
                    alarm_id,
                    opt_flg_2,
                    program,
                    test_label,
                })
            }
            (5, 30) => {
                let head_num = r.u1()?;
                let site_num = r.u1()?;
                let test_typ = r.u1()?;
                let test_num = r.u4()?;
                let exec_cnt = r.u4()?;
                let fail_cnt = r.u4()?;
                let alarm_cnt = r.u4()?;
                let test_nam = if !r.is_empty() { r.cn()? } else { String::new() };
                let seq_name = if !r.is_empty() { r.cn()? } else { String::new() };
                let test_lbl = if !r.is_empty() { r.cn()? } else { String::new() };
                Record::Tsr(Tsr {
                    head_num,
                    site_num,
                    test_typ,
                    test_num,
                    exec_cnt,
                    fail_cnt,
                    alarm_cnt,
                    test_nam,
                    seq_name,
                    test_lbl,
                })
            }
            (20, 10) => Record::Gdr(Gdr { gen_data: r.k_bytes()? }),
            (20, 30) => Record::Dtr(Dtr { text_dat: r.cn()? }),
            (20, 15) => Record::Bps(Bps),
            (20, 20) => Record::Eps(Eps),
            _ => return Ok(Record::Unknown { typ, sub, data: data.to_vec() }),
        };
        Ok(record)
    }
}

/// `Nn`: two-byte length, then that many ASCII characters (FTR PROGRAM /
/// TEST_LABEL fields).
impl<'a> Reader<'a> {
    pub fn nn(&mut self) -> Result<String> {
        let len = self.u2()? as usize;
        let bytes = self.take(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

fn encode_ptr(r: &Ptr, body: &mut Vec<u8>) {
    write::u4(body, r.test_num);
    write::u1(body, r.head_num);
    write::u1(body, r.site_num);
    write::u1(body, r.test_flg);
    write::u1(body, r.parm_flg);
    write::r4(body, r.result);
    write::cn(body, &r.test_txt);
    write::cn(body, &r.alarm_id);
    if let Some(opt_flg) = r.opt_flg {
        write::u1(body, opt_flg);
        let flags = r.opt_flg.unwrap_or(0);
        if flags & opt_flg::NO_RES_SCAL == 0 && r.res_scal.is_some() {
            write::i1(body, r.res_scal.unwrap_or(0));
        }
        if flags & opt_flg::NO_LLM_SCAL == 0 && r.llm_scal.is_some() {
            write::i1(body, r.llm_scal.unwrap_or(0));
        }
        if flags & opt_flg::NO_HLM_SCAL == 0 && r.hlm_scal.is_some() {
            write::i1(body, r.hlm_scal.unwrap_or(0));
        }
        if flags & opt_flg::NO_LO_LIMIT == 0 && r.lo_limit.is_some() {
            write::r4(body, r.lo_limit.unwrap_or(0.0));
        }
        if flags & opt_flg::NO_HI_LIMIT == 0 && r.hi_limit.is_some() {
            write::r4(body, r.hi_limit.unwrap_or(0.0));
        }
    }
    write::cn(body, &r.units);
}

fn encode_ftr(r: &Ftr, body: &mut Vec<u8>) {
    write::u4(body, r.test_num);
    write::u1(body, r.head_num);
    write::u1(body, r.site_num);
    write::u1(body, r.test_flg);
    write::u1(body, r.opt_flg);
    write::u4(body, r.cycl_cnt);
    write::u4(body, r.rel_vpos);
    write::u4(body, r.rept_cnt);
    write::u4(body, r.num_fail);
    write::i2(body, r.x_fail_adj);
    write::i2(body, r.y_fail_adj);
    write::i2(body, r.vect_offset);
    write::u2(body, r.rtn_icq);
    write::u2(body, r.rtn_ilim);
    write::u2(body, r.pgm_icq);
    write::u2(body, r.pgm_ilim);
    write::sn(body, &r.rtn_nam);
    write::sn(body, &r.pgm_nam);
    if let Some(alarm_id) = &r.alarm_id {
        write::cn(body, alarm_id);
        write::u1(body, r.opt_flg_2.unwrap_or(0));
        if r.opt_flg_2.is_none_or(|f| f & 0x02 == 0) {
            if let Some(program) = &r.program {
                let bytes = program.as_bytes();
                write::u2(body, bytes.len() as u16);
                body.extend_from_slice(bytes);
            }
        }
        if r.opt_flg_2.is_none_or(|f| f & 0x04 == 0) {
            if let Some(label) = &r.test_label {
                let bytes = label.as_bytes();
                write::u2(body, bytes.len() as u16);
                body.extend_from_slice(bytes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(record: Record) {
        let bytes = record.to_bytes();
        // Pull (typ, sub, body) back out of the encoded bytes.
        let len = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
        let typ = bytes[2];
        let sub = bytes[3];
        assert_eq!(bytes.len(), 4 + len, "REC_LEN must cover the body exactly");
        let parsed = Record::parse(typ, sub, &bytes[4..], Endian::Little).expect("parse");
        assert_eq!(parsed, record, "record must round-trip through bytes");
    }

    #[test]
    fn far_roundtrip() {
        roundtrip(Record::Far(Far { cpu_type: crate::stdf::codec::cpu_type::X86, stdf_ver: 4 }));
    }

    #[test]
    fn mir_roundtrip_with_populated_fields() {
        let mut mir = Mir { setup_t: 1_700_000_000, start_t: 1_700_000_100, ..Mir::default() };
        mir.lot_id = "LOT-42".into();
        mir.part_typ = "CHIPLET-A1".into();
        mir.tstr_typ = "SIM-ATE-1".into();
        roundtrip(Record::Mir(mir));
    }

    #[test]
    fn ptr_measured_roundtrip() {
        roundtrip(Record::Ptr(Ptr::measured(100, 1, 3, 3.3, "VDD_CORE", "V", 3.0, 3.6)));
    }

    #[test]
    fn ptr_verdict_roundtrip() {
        roundtrip(Record::Ptr(Ptr::verdict(200, 1, 3, false)));
    }

    #[test]
    fn ptr_parse_respects_opt_flag_presence_bit() {
        // PARM_FLG without bit 4: no OPT_FLAG, no optional fields at all.
        let mut body = Vec::new();
        write::u4(&mut body, 5);
        write::u1(&mut body, 1);
        write::u1(&mut body, 2);
        write::u1(&mut body, 0);
        write::u1(&mut body, 0); // PARM_FLG: no OPT_FLAG_PRESENT
        write::r4(&mut body, 1.0);
        write::cn(&mut body, "T");
        write::cn(&mut body, "");
        write::cn(&mut body, "V");
        let rec = Record::parse(15, 10, &body, Endian::Little).expect("parse");
        match rec {
            Record::Ptr(p) => {
                assert!(p.opt_flg.is_none());
                assert!(p.lo_limit.is_none() && p.hi_limit.is_none());
            }
            other => panic!("expected PTR, got {other:?}"),
        }
    }

    #[test]
    fn ftr_roundtrip() {
        roundtrip(Record::Ftr(Ftr::pattern(300, 1, 2, true, "ATPG_SCAN")));
    }

    #[test]
    fn ftr_roundtrip_with_optional_fields() {
        let mut ftr = Ftr::pattern(301, 1, 2, false, "ATPG_MBIST");
        ftr.opt_flg = 0x01; // optional field block present
        ftr.alarm_id = Some("ALM1".into());
        ftr.opt_flg_2 = Some(0);
        ftr.program = Some("PROG-1".into());
        ftr.test_label = Some("LBL-1".into());
        roundtrip(Record::Ftr(ftr));
    }

    #[test]
    fn wafer_records_roundtrip() {
        roundtrip(Record::Wir(Wir {
            head_num: 1,
            site_grp: 0,
            start_t: 1_700_000_200,
            wafer_id: "W01".into(),
        }));
        roundtrip(Record::Wcr(Wcr {
            wafr_siz: 150.0,
            die_ht: 5200.0,
            die_wid: 4100.0,
            wf_units: 3, // mil
            wf_flat: 0b100,
            center_x: 0,
            center_y: 0,
            pos_x: b'R',
            pos_y: b'T',
        }));
        roundtrip(Record::Wrr(Wrr {
            head_num: 1,
            site_grp: 0,
            finish_t: 1_700_000_900,
            part_cnt: 441,
            rtst_cnt: 0,
            abrt_cnt: 1,
            good_cnt: 400,
            func_cnt: 0,
            wafer_id: "W01".into(),
            fabwf_id: String::new(),
            frame_id: String::new(),
            mask_id: String::new(),
            usr_desc: String::new(),
            exc_desc: String::new(),
        }));
    }

    #[test]
    fn bin_records_roundtrip() {
        roundtrip(Record::Hbr(Hbr {
            head_num: 1,
            site_num: 255,
            hbin_num: 1,
            hbin_cnt: 400,
            hbin_pf: b'P',
            hbin_nam: "PASS".into(),
        }));
        roundtrip(Record::Sbr(Sbr {
            head_num: 1,
            site_num: 255,
            sbin_num: 12,
            sbin_cnt: 7,
            sbin_pf: b'F',
            sbin_nam: "VDD_MIN_SHORT".into(),
        }));
        roundtrip(Record::Pcr(Pcr {
            head_num: 1,
            site_num: 255,
            part_cnt: 441,
            rtst_cnt: 0,
            abrt_cnt: 1,
            good_cnt: 400,
            func_cnt: 0,
        }));
        roundtrip(Record::Mrr(Mrr {
            finish_t: 1_700_000_999,
            disp_cod: b' ',
            usr_cod: 0,
            exc_cod: 0,
            usr_desc: String::new(),
            exc_desc: String::new(),
        }));
    }

    #[test]
    fn unknown_record_preserved_verbatim() {
        // Type 99/sub 5 is not modeled; body must survive byte-for-byte.
        let data = vec![0x02, 0x00, 99, 5, 0xDE, 0xAD];
        let parsed = Record::parse(99, 5, &[0xDE, 0xAD], Endian::Little).expect("parse");
        assert_eq!(parsed, Record::Unknown { typ: 99, sub: 5, data: vec![0xDE, 0xAD] });
        assert_eq!(parsed.to_bytes(), data);
    }

    #[test]
    fn gdr_dtr_bps_eps_roundtrip() {
        roundtrip(Record::Gdr(Gdr { gen_data: vec![1, 2, 3, 4] }));
        roundtrip(Record::Dtr(Dtr { text_dat: "note".into() }));
        roundtrip(Record::Bps(Bps));
        roundtrip(Record::Eps(Eps));
    }
}
