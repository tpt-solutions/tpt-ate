//! GEM (SEMI E30) semantics on top of SECS-II: the well-known stream/function
//! transactions the ATE flow uses, plus the communication/control state models.
//!
//! Stream/function numbering follows the published SEMI standard headers
//! (E5 Section 6 / E30 Annex): S1 status and communications, S2 equipment
//! control, S5 alarms, S6 data collection, S9 error responses.

use crate::secs::error::{Result, SecsError};
use crate::secs::hsms::HsmsSession;
use crate::secs::item::SecsItem;
use std::io::{Read, Write};

/// Event IDs used by the ATE simulator's data collection (S6F11). Real
/// equipment configures these per test plan; the simulator uses one fixed
/// vocabulary documented here.
pub mod ceid {
    /// A die finished testing; attached report carries die location and bins.
    pub const DIE_TESTED: u32 = 101;
    /// All dies on the wafer processed.
    pub const WAFER_DONE: u32 = 102;
}

/// Report variable IDs (VIDs) carried in the DIE_TESTED event report.
pub mod vid {
    /// Die X coordinate on the wafer map (U2).
    pub const DIE_X: u32 = 1000;
    /// Die Y coordinate on the wafer map (U2).
    pub const DIE_Y: u32 = 1001;
    /// Assigned hardware bin (U2).
    pub const HARD_BIN: u32 = 1002;
    /// Assigned software bin (U2).
    pub const SOFT_BIN: u32 = 1003;
    /// Die passed (BOOLEAN).
    pub const DIE_PASS: u32 = 1004;
}

/// Report ID (RPTID) used for the DIE_TESTED event report.
pub const DIE_TESTED_RPTID: u32 = 1;

/// One parsed report entry from an S6F11: `(RPTID, V-items)`.
pub type ReportEntry = (u32, Vec<SecsItem>);

/// Build S1F1 "Are You There" (host asks equipment to identify itself).
pub fn s1f1_are_you_there() -> SecsItem {
    SecsItem::List(vec![])
}

/// Build S1F2 "On Line Data": equipment identifies itself with MDLN/SOFTREV.
pub fn s1f2_online_data(model: &str, soft_rev: &str) -> SecsItem {
    SecsItem::List(vec![SecsItem::a(model), SecsItem::a(soft_rev)])
}

/// Build S1F13 "Establish Communications Request" (empty list body).
pub fn s1f13_establish_communications_request() -> SecsItem {
    SecsItem::List(vec![])
}

/// Build S1F14 "Establish Communications Request Acknowledge".
pub fn s1f14_establish_communications_ack(accepted: bool) -> SecsItem {
    SecsItem::List(vec![
        SecsItem::Boolean(vec![accepted]),
        SecsItem::a(if accepted { "Established" } else { "Refused" }),
    ])
}

/// Build S6F11 "Event Report Send": `L,3 [DATAID U4, CEID U4, L,n [L,2 [RPTID
/// U4, V-list]]]`.
pub fn s6f11_event_report(dataid: u32, ceid: u32, reports: &[(u32, Vec<SecsItem>)]) -> SecsItem {
    let report_items: Vec<SecsItem> = reports
        .iter()
        .map(|(rptid, variables)| {
            SecsItem::List(vec![SecsItem::u4(*rptid), SecsItem::List(variables.clone())])
        })
        .collect();
    SecsItem::List(vec![SecsItem::u4(dataid), SecsItem::u4(ceid), SecsItem::List(report_items)])
}

/// Build S6F12 "Event Report Acknowledge" (`B` DACKN, 0 = accepted).
pub fn s6f12_acknowledge(dackn: u8) -> SecsItem {
    SecsItem::Binary(vec![dackn])
}

/// Parse an S6F11 body into `(dataid, ceid, reports)` where each report is
/// `(rptid, variables)`.
pub fn parse_s6f11(body: &SecsItem) -> Result<(u32, u32, Vec<ReportEntry>)> {
    let items = body.as_list().ok_or_else(|| SecsError::Gem("S6F11 body is not a list".into()))?;
    if items.len() != 3 {
        return Err(SecsError::Gem(format!("S6F11 body has {} items, expected 3", items.len())));
    }
    let dataid = items[0].as_u4().ok_or_else(|| SecsError::Gem("S6F11 DATAID is not U4".into()))?;
    let ceid = items[1].as_u4().ok_or_else(|| SecsError::Gem("S6F11 CEID is not U4".into()))?;
    let mut reports = Vec::new();
    for report in items[2]
        .as_list()
        .ok_or_else(|| SecsError::Gem("S6F11 report list is not a list".into()))?
    {
        let pair = report
            .as_list()
            .ok_or_else(|| SecsError::Gem("S6F11 report entry is not a list".into()))?;
        if pair.len() != 2 {
            return Err(SecsError::Gem(format!(
                "S6F11 report entry has {} items, expected 2",
                pair.len()
            )));
        }
        let rptid =
            pair[0].as_u4().ok_or_else(|| SecsError::Gem("S6F11 RPTID is not U4".into()))?;
        let variables = pair[1]
            .as_list()
            .ok_or_else(|| SecsError::Gem("S6F11 V-list is not a list".into()))?
            .to_vec();
        reports.push((rptid, variables));
    }
    Ok((dataid, ceid, reports))
}

/// The equipment-side GEM session used by the simulator. Wraps an HSMS
/// session and answers the standard handshake transactions.
pub struct EquipmentGem<S: Read + Write> {
    session: HsmsSession<S>,
    model_name: String,
    soft_rev: String,
}

impl<S: Read + Write> EquipmentGem<S> {
    pub fn new(
        session: HsmsSession<S>,
        model_name: impl Into<String>,
        soft_rev: impl Into<String>,
    ) -> Self {
        EquipmentGem { session, model_name: model_name.into(), soft_rev: soft_rev.into() }
    }

    /// Serve one inbound transaction, answering the handshake messages the
    /// equipment must respond to. Returns the (stream, function, body) of
    /// primaries it does not answer itself (currently: none — everything the
    /// host sends in this flow is answered).
    pub fn serve_one(&mut self) -> Result<Option<(u8, u8, Option<SecsItem>)>> {
        let (stream, function, body) =
            self.session.serve_one(|_stream, function, _body| match function {
                1 => Some(s1f2_online_data(&self.model_name, &self.soft_rev)), // S1F1 → S1F2
                13 => Some(s1f14_establish_communications_ack(true)),          // S1F13 → S1F14
                _ => None,
            })?;
        Ok(Some((stream, function, body)))
    }

    /// Send S6F11 and consume the S6F12 acknowledge.
    pub fn send_event_report(
        &mut self,
        dataid: u32,
        ceid: u32,
        reports: &[(u32, Vec<SecsItem>)],
    ) -> Result<()> {
        let body = s6f11_event_report(dataid, ceid, reports);
        let (function, reply) = self.session.transact(6, 11, Some(body))?;
        if function != 12 {
            return Err(SecsError::UnexpectedStreamFunction {
                stream: 6,
                function,
                wanted: "S6F12 acknowledge".into(),
            });
        }
        let dackn = match reply {
            Some(SecsItem::Binary(b)) => b.first().copied(),
            _ => None,
        };
        if let Some(code) = dackn {
            if code != 0 {
                return Err(SecsError::Gem(format!("host rejected event report, DACKN={code}")));
            }
        }
        Ok(())
    }
}

/// The host-side GEM session: performs the Establish Communications
/// handshake against an equipment and exposes transactions.
pub struct HostGem<S: Read + Write> {
    session: HsmsSession<S>,
    next_dataid: u32,
}

impl<S: Read + Write> HostGem<S> {
    pub fn new(session: HsmsSession<S>) -> Self {
        HostGem { session, next_dataid: 1 }
    }

    /// S1F1 → S1F2: ask the equipment to identify itself.
    pub fn online_data(&mut self) -> Result<(String, String)> {
        let (_function, reply) = self.session.transact(1, 1, Some(s1f1_are_you_there()))?;
        parse_online_data(reply.as_ref())
    }

    /// S1F13 → S1F14: establish communications.
    pub fn establish_communications(&mut self) -> Result<bool> {
        let (_function, reply) =
            self.session.transact(1, 13, Some(s1f13_establish_communications_request()))?;
        let items = reply
            .as_ref()
            .and_then(|r| r.as_list().map(|l| l.to_vec()))
            .ok_or_else(|| SecsError::Gem("S1F14 body is not a list".into()))?;
        let accepted = items
            .first()
            .and_then(|i| match i {
                SecsItem::Boolean(v) => v.first().copied(),
                _ => None,
            })
            .ok_or_else(|| SecsError::Gem("S1F14 COMMACK missing".into()))?;
        Ok(accepted)
    }

    /// Send S6F11 and wait for the S6F12 acknowledge; returns DACKN.
    pub fn send_event_report(&mut self, ceid: u32, reports: &[(u32, Vec<SecsItem>)]) -> Result<u8> {
        let dataid = self.next_dataid;
        self.next_dataid += 1;
        let body = s6f11_event_report(dataid, ceid, reports);
        let (function, reply) = self.session.transact(6, 11, Some(body))?;
        if function != 12 {
            return Err(SecsError::UnexpectedStreamFunction {
                stream: 6,
                function,
                wanted: "S6F12 acknowledge".into(),
            });
        }
        let dackn = match reply {
            Some(SecsItem::Binary(b)) => b.first().copied().unwrap_or(1),
            _ => 1,
        };
        Ok(dackn)
    }

    /// Receive one S6F11 (as the receiving side of an event report), answering
    /// S6F12 with DACKN 0. Returns the parsed report.
    pub fn receive_event_report(&mut self) -> Result<(u32, u32, Vec<ReportEntry>)> {
        loop {
            let (stream, function, body) =
                self.session.serve_one(|_s, _f, _b| Some(s6f12_acknowledge(0)))?;
            if stream == 6 && function == 11 {
                let body = body.ok_or(SecsError::MissingDataBody { stream: 6, function: 11 })?;
                return parse_s6f11(&body);
            }
            // Other primaries (e.g. S1F1) are answered and skipped.
        }
    }

    /// Unwrap the underlying HSMS session.
    pub fn into_session(self) -> HsmsSession<S> {
        self.session
    }
}

fn parse_online_data(reply: Option<&SecsItem>) -> Result<(String, String)> {
    let items = reply
        .and_then(|r| r.as_list())
        .ok_or_else(|| SecsError::Gem("S1F2 body is not a list".into()))?;
    let model = items
        .first()
        .and_then(|i| i.as_ascii())
        .ok_or_else(|| SecsError::Gem("S1F2 MDLN missing".into()))?;
    let rev = items
        .get(1)
        .and_then(|i| i.as_ascii())
        .ok_or_else(|| SecsError::Gem("S1F2 SOFTREV missing".into()))?;
    Ok((model.to_string(), rev.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s6f11_roundtrip() {
        let body = s6f11_event_report(
            7,
            ceid::DIE_TESTED,
            &[(
                DIE_TESTED_RPTID,
                vec![
                    SecsItem::u2(vid::DIE_X as u16),
                    SecsItem::u2(vid::DIE_Y as u16),
                    SecsItem::Boolean(vec![true]),
                ],
            )],
        );
        let (dataid, ceid, reports) = parse_s6f11(&body).expect("parse");
        assert_eq!(dataid, 7);
        assert_eq!(ceid, ceid::DIE_TESTED);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].0, DIE_TESTED_RPTID);
        assert_eq!(reports[0].1.len(), 3);
    }

    #[test]
    fn s6f11_rejects_malformed() {
        let bad = SecsItem::List(vec![SecsItem::u4(1)]);
        let err = parse_s6f11(&bad).unwrap_err();
        assert!(matches!(err, SecsError::Gem(_)));
    }
}
