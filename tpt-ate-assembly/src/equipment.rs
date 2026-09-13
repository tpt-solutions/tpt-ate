//! Simulated assembly equipment: the equipment side of the GEM flow —
//! accepts Select, answers the handshake and host commands (S2F41 → S2F42
//! via the shared `EquipmentGem`), executes one `PLACE_*` command per plan
//! entry with a deterministic placement jitter, and reports each completed
//! placement as an S6F11 event report.
//!
//! Deterministic like the test-equipment simulator: jitter per command is
//! seeded from the run seed plus the command index.

use crate::backend::{AssemblyTechnique, PlacementCommand};
use std::io::{Read, Write};
use tpt_ate_comm::rng::Rng;
use tpt_ate_comm::secs::error::{Result, SecsError};
use tpt_ate_comm::secs::gem::{parse_s2f41, EquipmentGem};
use tpt_ate_comm::secs::hsms::HsmsSession;
use tpt_ate_comm::secs::item::SecsItem;

/// CEID: a placement completed; the report carries the site and its
/// as-placed position (V-list: SITE A, X U4, Y U4 — positional).
pub const CEID_PLACEMENT_DONE: u32 = 201;
/// CEID: the whole assembly run completed.
pub const CEID_RUN_COMPLETE: u32 = 202;

/// The equipment's placement jitter in nm (uniform half-window per axis).
pub const PLACEMENT_JITTER_NM: f32 = 15.0;

/// The simulated assembly machine for one technique.
pub struct AssemblyEquipment {
    technique: AssemblyTechnique,
    model_name: String,
    seed: u64,
    jitter_nm: f32,
}

impl AssemblyEquipment {
    pub fn new(technique: AssemblyTechnique, seed: u64) -> AssemblyEquipment {
        let suffix = match technique {
            AssemblyTechnique::WireBond => "WB",
            AssemblyTechnique::FlipChip => "FC",
            AssemblyTechnique::ChipletPickPlace => "PP",
        };
        AssemblyEquipment {
            technique,
            model_name: format!("TPT-SIM-{suffix}"),
            seed,
            jitter_nm: PLACEMENT_JITTER_NM,
        }
    }

    pub fn technique(&self) -> AssemblyTechnique {
        self.technique
    }

    /// Equipment side of one assembly session over an HSMS connection.
    /// Executes exactly `commands.len()` host commands in arrival order
    /// (the host drives ordering), then acknowledges the run.
    pub fn run<S: Read + Write>(
        &self,
        stream: S,
        session_id: u16,
        commands: &[PlacementCommand],
    ) -> Result<()> {
        let mut session = HsmsSession::new(stream, session_id);
        session.accept_select()?;
        let mut gem =
            EquipmentGem::new(session, self.model_name.clone(), env!("CARGO_PKG_VERSION"));

        // Handshake: answer whatever the host asks (S1F1 and/or S1F13).
        let mut served = 0;
        while served < 2 {
            gem.serve_one()?;
            served += 1;
        }

        for (index, command) in commands.iter().enumerate() {
            // Expect the host's S2F41 for this command (S2F42 already sent
            // by serve_one).
            let (stream, function, body) = gem.serve_one()?;
            if stream != 2 || function != 41 {
                return Err(SecsError::UnexpectedStreamFunction {
                    stream,
                    function,
                    wanted: format!("S2F41 for site {}", command.site_id),
                });
            }
            let (rcmd, _params) = parse_s2f41(
                body.as_ref().ok_or(SecsError::MissingDataBody { stream: 2, function: 41 })?,
            )?;
            if rcmd != self.technique.rcmd() {
                return Err(SecsError::Gem(format!(
                    "equipment {} got host command {rcmd}",
                    self.technique.rcmd()
                )));
            }

            // Deterministic placement: jitter within the process window.
            let mut rng = Rng::from_parts(&[
                self.seed,
                index as u64,
                command.target.x as u64,
                command.target.y as u64,
            ]);
            let placed_x = command.target.x + rng.uniform(-self.jitter_nm, self.jitter_nm) as i64;
            let placed_y = command.target.y + rng.uniform(-self.jitter_nm, self.jitter_nm) as i64;

            gem.send_event_report(
                1,
                CEID_PLACEMENT_DONE,
                &[(
                    1,
                    vec![
                        SecsItem::a(command.site_id.clone()),
                        SecsItem::u4(placed_x.max(0) as u32),
                        SecsItem::u4(placed_y.max(0) as u32),
                    ],
                )],
            )?;
        }
        Ok(())
    }
}
