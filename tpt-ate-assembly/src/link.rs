//! Host-side link to assembly equipment over GEM: connect (Select + S1F1 +
//! S1F13 handshake), issue placement host commands (S2F41 → S2F42), and
//! collect the placement-complete event reports (S6F11 → S6F12).

use crate::backend::{AssemblyError, PlacementCommand};
use crate::layout::PointNm;
use std::io::{Read, Write};
use tpt_ate_comm::secs::error::{Result as SecsResult, SecsError};
use tpt_ate_comm::secs::gem::{parse_s6f11, s2f41_host_command, HostGem, HCACK_OK};
use tpt_ate_comm::secs::hsms::HsmsSession;
use tpt_ate_comm::secs::item::SecsItem;
use tpt_ate_comm::secs::transport::DuplexHalf;

/// The concrete stream the simulated backends are typed over (TCP works the
/// same way in production — any `Read + Write` does).
pub type SimStream = DuplexHalf;

/// Host side of an assembly-equipment GEM session.
pub struct AssemblyHost<S: Read + Write> {
    gem: HostGem<S>,
}

impl<S: Read + Write> AssemblyHost<S> {
    /// Select the session and run the standard handshake
    /// (S1F1 → S1F2 identify, S1F13 → S1F14 establish communications).
    pub fn connect(stream: S, session_id: u16) -> SecsResult<AssemblyHost<S>> {
        let mut session = HsmsSession::new(stream, session_id);
        session.select()?;
        let mut gem = HostGem::new(session);
        let (model, _rev) = gem.online_data()?;
        if model.is_empty() {
            return Err(SecsError::Gem("equipment reported empty model number".into()));
        }
        if !gem.establish_communications()? {
            return Err(SecsError::Gem("equipment refused to establish communications".into()));
        }
        Ok(AssemblyHost { gem })
    }

    /// Issue one placement host command and wait for the matching
    /// placement-complete event. Returns the as-placed position reported by
    /// the equipment.
    pub fn place(
        &mut self,
        rcmd: &str,
        command: &PlacementCommand,
    ) -> std::result::Result<PointNm, AssemblyError> {
        let body = s2f41_host_command(
            rcmd,
            &[
                ("SITE".to_string(), SecsItem::a(command.site_id.clone())),
                ("X".to_string(), SecsItem::u4(command.target.x.max(0) as u32)),
                ("Y".to_string(), SecsItem::u4(command.target.y.max(0) as u32)),
            ],
        );
        let (function, reply) =
            self.gem.transact(2, 41, Some(body)).map_err(AssemblyError::Comm)?;
        if function != 42 {
            return Err(AssemblyError::Comm(SecsError::UnexpectedStreamFunction {
                stream: 2,
                function,
                wanted: "S2F42 acknowledge".into(),
            }));
        }
        let hcack = match reply {
            Some(SecsItem::Binary(b)) => b.first().copied().unwrap_or(1),
            _ => 1,
        };
        if hcack != HCACK_OK {
            return Err(AssemblyError::CommandRejected { site_id: command.site_id.clone(), hcack });
        }
        self.await_placement_event(&command.site_id)
    }

    /// Wait for the S6F11 placement-done event for `site_id`, answering
    /// every event report with S6F12 DACKN 0.
    fn await_placement_event(
        &mut self,
        site_id: &str,
    ) -> std::result::Result<PointNm, AssemblyError> {
        loop {
            let (stream, function, body) =
                self.gem.serve_event_reports().map_err(AssemblyError::Comm)?;
            if stream != 6 || function != 11 {
                continue; // answered, but not an event report
            }
            let body = body.ok_or(AssemblyError::Comm(SecsError::MissingDataBody {
                stream: 6,
                function: 11,
            }))?;
            let (_dataid, ceid_received, reports) =
                parse_s6f11(&body).map_err(AssemblyError::Comm)?;
            if ceid_received != crate::equipment::CEID_PLACEMENT_DONE {
                continue;
            }
            for (_rptid, variables) in &reports {
                if variables.len() != 3 {
                    return Err(AssemblyError::MalformedEvent {
                        site_id: site_id.to_string(),
                        name: format!("V-list of {}", variables.len()),
                    });
                }
                let reported_site = variables[0]
                    .as_ascii()
                    .ok_or(AssemblyError::MalformedEvent {
                        site_id: site_id.to_string(),
                        name: "SITE".into(),
                    })?
                    .to_string();
                if reported_site != site_id {
                    continue;
                }
                let x = variables[1].as_u4().ok_or(AssemblyError::MalformedEvent {
                    site_id: site_id.to_string(),
                    name: "X".into(),
                })? as i64;
                let y = variables[2].as_u4().ok_or(AssemblyError::MalformedEvent {
                    site_id: site_id.to_string(),
                    name: "Y".into(),
                })? as i64;
                return Ok(PointNm::new(x, y));
            }
        }
    }
}
