//! HSMS (SEMI E37/E37.1, single selected session) message framing and session
//! state handling.
//!
//! Wire format: a 4-byte big-endian total length, then a 10-byte header, then
//! (for data messages only) a SECS-II body.
//!
//! Header layout (SEMI E37, Figure 1):
//! bytes 1-2 session ID, byte 3 W-bit + stream, byte 4 function, byte 5 PType
//! (0 = SECS-II), byte 6 SType, bytes 7-10 system bytes.

use crate::secs::error::{Result, SecsError};
use crate::secs::item::SecsItem;
use std::io::{Read, Write};

/// HSMS SType values (SEMI E37, Table 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SType {
    /// SECS-II data message.
    Data = 0,
    SelectReq = 1,
    SelectRsp = 2,
    DeselectReq = 3,
    DeselectRsp = 4,
    LinktestReq = 5,
    LinktestRsp = 6,
    RejectReq = 7,
    SeparateReq = 9,
}

impl SType {
    fn from_u8(v: u8) -> Option<SType> {
        Some(match v {
            0 => SType::Data,
            1 => SType::SelectReq,
            2 => SType::SelectRsp,
            3 => SType::DeselectReq,
            4 => SType::DeselectRsp,
            5 => SType::LinktestReq,
            6 => SType::LinktestRsp,
            7 => SType::RejectReq,
            9 => SType::SeparateReq,
            _ => return None,
        })
    }
}

/// Select.rsp select-status codes (SEMI E37.1, Section 5.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SelectStatus {
    /// Select accepted — session is now Selected.
    Selected = 0,
    /// Select rejected — connection already selected.
    AlreadySelected = 1,
    /// Select rejected by the remote side.
    NotSelected = 2,
}

/// Reject.req reason codes (SEMI E37, Table 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RejectReason {
    NotSelected = 1,
    InsufficientResources = 2,
    UnrecognizedParameter = 3,
    IllegalStatus = 4,
    TimerExpired = 5,
    Congestion = 6,
}

/// Connection state per SEMI E37.1 Figure 3, reduced to the states the
/// single-session transport moves through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HsmsState {
    /// TCP connection not established.
    NotConnected,
    /// TCP established, no Select exchanged yet (NOT SELECTED).
    ConnectedNotSelected,
    /// Select completed (SELECTED) — SECS-II data may flow.
    Selected,
}

/// An incoming HSMS message.
#[derive(Debug, Clone, PartialEq)]
pub enum HsmsMessage {
    Data {
        session_id: u16,
        stream: u8,
        function: u8,
        /// W-bit: primary expects a reply.
        w_bit: bool,
        system_bytes: u32,
        body: Option<SecsItem>,
    },
    SelectReq {
        system_bytes: u32,
    },
    SelectRsp {
        system_bytes: u32,
        status: SelectStatus,
    },
    LinktestReq {
        system_bytes: u32,
    },
    LinktestRsp {
        system_bytes: u32,
    },
    Separate {
        system_bytes: u32,
    },
    Reject {
        system_bytes: u32,
        reason: u8,
        /// Stream/function of the message being rejected (raw header bytes 3-4).
        rejected_s: u8,
        rejected_f: u8,
    },
}

impl HsmsMessage {
    /// Build a SECS-II data message. `stream`/`function` are the SxFy pair;
    /// `w_bit` marks a primary that expects a reply.
    pub fn data(
        session_id: u16,
        stream: u8,
        function: u8,
        w_bit: bool,
        system_bytes: u32,
        body: Option<SecsItem>,
    ) -> HsmsMessage {
        HsmsMessage::Data { session_id, stream, function, w_bit, system_bytes, body }
    }

    /// The system bytes echoed back in replies and used to match transactions.
    pub fn system_bytes(&self) -> u32 {
        match self {
            HsmsMessage::Data { system_bytes, .. }
            | HsmsMessage::SelectReq { system_bytes }
            | HsmsMessage::SelectRsp { system_bytes, .. }
            | HsmsMessage::LinktestReq { system_bytes }
            | HsmsMessage::LinktestRsp { system_bytes }
            | HsmsMessage::Separate { system_bytes }
            | HsmsMessage::Reject { system_bytes, .. } => *system_bytes,
        }
    }

    /// If this is a data message, its (stream, function, w-bit, body) tuple.
    pub fn as_data(&self) -> Option<(u8, u8, bool, &Option<SecsItem>)> {
        match self {
            HsmsMessage::Data { stream, function, w_bit, body, .. } => {
                Some((*stream, *function, *w_bit, body))
            }
            _ => None,
        }
    }

    /// Encode to the wire: 4-byte length prefix + 10-byte header + body.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut header = [0u8; 10];
        let mut body: Vec<u8> = Vec::new();
        match self {
            HsmsMessage::Data { session_id, stream, function, w_bit, system_bytes, body: item } => {
                header[0..2].copy_from_slice(&session_id.to_be_bytes());
                let mut b2 = *stream & 0x7F;
                if *w_bit {
                    b2 |= 0x80;
                }
                header[2] = b2;
                header[3] = function & 0x7F;
                header[4] = 0; // PType: SECS-II
                header[5] = SType::Data as u8;
                header[6..10].copy_from_slice(&system_bytes.to_be_bytes());
                if let Some(item) = item {
                    item.encode(&mut body);
                }
            }
            HsmsMessage::SelectReq { system_bytes } => {
                header[5] = SType::SelectReq as u8;
                header[6..10].copy_from_slice(&system_bytes.to_be_bytes());
            }
            HsmsMessage::SelectRsp { system_bytes, status } => {
                header[5] = SType::SelectRsp as u8;
                header[6..10].copy_from_slice(&system_bytes.to_be_bytes());
                // SEMI E37: the Select.rsp status lives in header byte 10,
                // which overlaps the low byte of the system bytes — the
                // status is authoritative on the wire.
                header[9] = *status as u8;
            }
            HsmsMessage::LinktestReq { system_bytes } => {
                header[5] = SType::LinktestReq as u8;
                header[6..10].copy_from_slice(&system_bytes.to_be_bytes());
            }
            HsmsMessage::LinktestRsp { system_bytes } => {
                header[5] = SType::LinktestRsp as u8;
                header[6..10].copy_from_slice(&system_bytes.to_be_bytes());
            }
            HsmsMessage::Separate { system_bytes } => {
                header[5] = SType::SeparateReq as u8;
                header[6..10].copy_from_slice(&system_bytes.to_be_bytes());
            }
            HsmsMessage::Reject { system_bytes, reason, rejected_s, rejected_f } => {
                header[5] = SType::RejectReq as u8;
                header[6..10].copy_from_slice(&system_bytes.to_be_bytes());
                header[7] = *rejected_s;
                header[8] = *rejected_f;
                header[9] = *reason;
            }
        }
        let total_len = (10 + body.len()) as u32;
        out.extend_from_slice(&total_len.to_be_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(&body);
    }

    /// Encode to a fresh `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }

    /// Decode one message from the front of `data`. Returns the message and
    /// how many bytes it consumed.
    pub fn from_bytes(data: &[u8]) -> Result<(HsmsMessage, usize)> {
        if data.len() < 4 {
            return Err(SecsError::UnexpectedEof { wanted: 4, had: data.len() });
        }
        let total_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if total_len < 10 {
            return Err(SecsError::BadHeaderSize(total_len));
        }
        let needed = 4 + total_len;
        if data.len() < needed {
            return Err(SecsError::UnexpectedEof { wanted: needed, had: data.len() });
        }
        let header = &data[4..14];
        let stype = header[5];
        if stype == SType::Data as u8 && header[4] != 0 {
            return Err(SecsError::UnsupportedPType(header[4]));
        }
        let system_bytes = u32::from_be_bytes([header[6], header[7], header[8], header[9]]);
        let body_bytes = &data[14..needed];
        let message = match SType::from_u8(stype) {
            Some(SType::Data) => {
                let w_bit = header[2] & 0x80 != 0;
                let stream = header[2] & 0x7F;
                let function = header[3] & 0x7F;
                let body = if body_bytes.is_empty() {
                    None
                } else {
                    let (item, used) = SecsItem::from_bytes(body_bytes)?;
                    if used != body_bytes.len() {
                        return Err(SecsError::TrailingBytesAfterItem {
                            count: body_bytes.len() - used,
                        });
                    }
                    Some(item)
                };
                HsmsMessage::Data {
                    session_id: u16::from_be_bytes([header[0], header[1]]),
                    stream,
                    function,
                    w_bit,
                    system_bytes,
                    body,
                }
            }
            Some(SType::SelectReq) => HsmsMessage::SelectReq { system_bytes },
            Some(SType::SelectRsp) => HsmsMessage::SelectRsp {
                system_bytes,
                status: match header[9] {
                    0 => SelectStatus::Selected,
                    1 => SelectStatus::AlreadySelected,
                    _ => SelectStatus::NotSelected,
                },
            },
            Some(SType::LinktestReq) => HsmsMessage::LinktestReq { system_bytes },
            Some(SType::LinktestRsp) => HsmsMessage::LinktestRsp { system_bytes },
            Some(SType::SeparateReq) => HsmsMessage::Separate { system_bytes },
            Some(SType::RejectReq) => HsmsMessage::Reject {
                system_bytes,
                reason: header[9],
                rejected_s: header[7],
                rejected_f: header[8],
            },
            // Deselect req/rsp and unknown STypes are carried as raw reject
            // frames for now; the ATE single-session flow never sends them.
            _ => HsmsMessage::Reject {
                system_bytes,
                reason: stype,
                rejected_s: header[7],
                rejected_f: header[8],
            },
        };
        Ok((message, needed))
    }
}

/// Read exactly `wanted` bytes from `reader`, failing on early EOF.
pub fn read_exact_counted<R: Read>(reader: &mut R, wanted: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; wanted];
    let mut filled = 0;
    while filled < wanted {
        let n = reader.read(&mut buf[filled..])?;
        if n == 0 {
            return Err(SecsError::UnexpectedEof { wanted, had: filled });
        }
        filled += n;
    }
    Ok(buf)
}

/// Read one HSMS message from a byte stream (blocking).
pub fn read_message<R: Read>(reader: &mut R) -> Result<HsmsMessage> {
    let len_bytes = read_exact_counted(reader, 4)?;
    let total_len =
        u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
    let rest = read_exact_counted(reader, total_len)?;
    let mut frame = len_bytes;
    frame.extend_from_slice(&rest);
    let (msg, _) = HsmsMessage::from_bytes(&frame)?;
    Ok(msg)
}

/// Write one HSMS message to a byte stream.
pub fn write_message<W: Write>(writer: &mut W, message: &HsmsMessage) -> Result<()> {
    let bytes = message.to_bytes();
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

/// A Selected-state HSMS session over any byte stream. Handles the
/// select/linktest control exchanges so callers only see SECS-II data
/// messages; non-data frames arriving while selected are answered per
/// SEMI E37 (linktest requests get responses, anything else is rejected).
pub struct HsmsSession<S> {
    stream: S,
    state: HsmsState,
    session_id: u16,
    next_system_bytes: u32,
}

impl<S: Read + Write> HsmsSession<S> {
    /// Wrap an established byte stream. The session starts in
    /// `ConnectedNotSelected`; call [`select`](Self::select) before data flow.
    pub fn new(stream: S, session_id: u16) -> Self {
        HsmsSession {
            stream,
            state: HsmsState::ConnectedNotSelected,
            session_id,
            next_system_bytes: 1,
        }
    }

    pub fn state(&self) -> HsmsState {
        self.state
    }

    fn next_system_bytes(&mut self) -> u32 {
        let sb = self.next_system_bytes;
        self.next_system_bytes = self.next_system_bytes.wrapping_add(1);
        sb
    }

    /// Perform the Select transaction (SEMI E37.1 Section 5.3). Whoever calls
    /// this acts as the selecting side; the peer answers. The Select.rsp
    /// status byte overlaps the echoed system bytes (see
    /// [`HsmsMessage::encode`]), so the first Select.rsp is taken as the
    /// answer — in a single-selected session nothing else can race it.
    pub fn select(&mut self) -> Result<()> {
        let sb = self.next_system_bytes();
        write_message(&mut self.stream, &HsmsMessage::SelectReq { system_bytes: sb })?;
        loop {
            let msg = read_message(&mut self.stream)?;
            match msg {
                HsmsMessage::SelectRsp { status, .. } => {
                    if status == SelectStatus::Selected {
                        self.state = HsmsState::Selected;
                        return Ok(());
                    }
                    return Err(SecsError::Gem(format!("select rejected, status {status:?}")));
                }
                // Tolerate interleaved control traffic during the handshake.
                HsmsMessage::LinktestReq { system_bytes } => {
                    write_message(&mut self.stream, &HsmsMessage::LinktestRsp { system_bytes })?;
                }
                other => {
                    return Err(SecsError::Gem(format!(
                        "unexpected {other:?} while awaiting Select.rsp"
                    )));
                }
            }
        }
    }

    /// Equipment side of the Select transaction: wait for Select.req and
    /// answer Selected.
    pub fn accept_select(&mut self) -> Result<()> {
        loop {
            let msg = read_message(&mut self.stream)?;
            match msg {
                HsmsMessage::SelectReq { system_bytes } => {
                    write_message(
                        &mut self.stream,
                        &HsmsMessage::SelectRsp { system_bytes, status: SelectStatus::Selected },
                    )?;
                    self.state = HsmsState::Selected;
                    return Ok(());
                }
                HsmsMessage::LinktestReq { system_bytes } => {
                    write_message(&mut self.stream, &HsmsMessage::LinktestRsp { system_bytes })?;
                }
                other => {
                    return Err(SecsError::Gem(format!(
                        "unexpected {other:?} while awaiting Select.req"
                    )));
                }
            }
        }
    }

    /// Send a SECS-II primary and wait for its secondary, matching on system
    /// bytes. Control frames that arrive meanwhile are handled or rejected.
    pub fn transact(
        &mut self,
        stream: u8,
        function: u8,
        body: Option<SecsItem>,
    ) -> Result<(u8, Option<SecsItem>)> {
        if self.state != HsmsState::Selected {
            return Err(SecsError::NotSelected { state: self.state });
        }
        let sb = self.next_system_bytes();
        write_message(
            &mut self.stream,
            &HsmsMessage::data(self.session_id, stream, function, true, sb, body),
        )?;
        let reply = self.await_reply(sb, function + 1)?;
        match reply {
            HsmsMessage::Data { function, body, .. } => Ok((function, body)),
            _ => unreachable!("await_reply returns only Data"),
        }
    }

    fn await_reply(&mut self, system_bytes: u32, want_function: u8) -> Result<HsmsMessage> {
        loop {
            let msg = read_message(&mut self.stream)?;
            match msg {
                HsmsMessage::Data { system_bytes: sb, function, body, .. }
                    if sb == system_bytes =>
                {
                    return Ok(HsmsMessage::Data {
                        session_id: self.session_id,
                        stream: 0,
                        function,
                        w_bit: false,
                        system_bytes: sb,
                        body,
                    });
                }
                HsmsMessage::LinktestReq { system_bytes: sb } => {
                    write_message(
                        &mut self.stream,
                        &HsmsMessage::LinktestRsp { system_bytes: sb },
                    )?;
                }
                HsmsMessage::SelectReq { system_bytes: sb } => {
                    write_message(
                        &mut self.stream,
                        &HsmsMessage::SelectRsp {
                            system_bytes: sb,
                            status: SelectStatus::AlreadySelected,
                        },
                    )?;
                }
                HsmsMessage::Separate { .. } => {
                    self.state = HsmsState::ConnectedNotSelected;
                    return Err(SecsError::Gem("peer sent Separate; session deselected".into()));
                }
                other => {
                    return Err(SecsError::UnexpectedStreamFunction {
                        stream: 0,
                        function: want_function,
                        wanted: format!("reply to system bytes {system_bytes}, got {other:?}"),
                    });
                }
            }
        }
    }

    /// Answer one inbound primary transaction: read a data message, reply
    /// with `make_reply` computed from the primary, and return the primary's
    /// (stream, function, body).
    pub fn serve_one<F>(&mut self, make_reply: F) -> Result<(u8, u8, Option<SecsItem>)>
    where
        F: FnOnce(u8, u8, Option<SecsItem>) -> Option<SecsItem>,
    {
        if self.state != HsmsState::Selected {
            return Err(SecsError::NotSelected { state: self.state });
        }
        loop {
            let msg = read_message(&mut self.stream)?;
            match msg {
                HsmsMessage::Data { session_id, stream, function, w_bit, system_bytes, body } => {
                    let reply =
                        if w_bit { make_reply(stream, function, body.clone()) } else { None };
                    if let Some(reply_body) = reply {
                        write_message(
                            &mut self.stream,
                            &HsmsMessage::data(
                                session_id,
                                stream,
                                function + 1,
                                false,
                                system_bytes,
                                Some(reply_body),
                            ),
                        )?;
                    }
                    return Ok((stream, function, body));
                }
                HsmsMessage::LinktestReq { system_bytes } => {
                    write_message(&mut self.stream, &HsmsMessage::LinktestRsp { system_bytes })?;
                }
                HsmsMessage::SelectReq { system_bytes } => {
                    write_message(
                        &mut self.stream,
                        &HsmsMessage::SelectRsp {
                            system_bytes,
                            status: SelectStatus::AlreadySelected,
                        },
                    )?;
                }
                HsmsMessage::Separate { .. } => {
                    self.state = HsmsState::ConnectedNotSelected;
                    return Err(SecsError::Gem("peer sent Separate; session deselected".into()));
                }
                other => {
                    write_message(
                        &mut self.stream,
                        &HsmsMessage::Reject {
                            system_bytes: other.system_bytes(),
                            reason: RejectReason::IllegalStatus as u8,
                            rejected_s: 0,
                            rejected_f: 0,
                        },
                    )?;
                }
            }
        }
    }

    /// Unwrap the underlying stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_message_roundtrip() {
        let msg =
            HsmsMessage::data(0x1234, 6, 11, true, 77, Some(SecsItem::List(vec![SecsItem::u4(5)])));
        let bytes = msg.to_bytes();
        assert_eq!(
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize,
            bytes.len() - 4
        );
        let (decoded, used) = HsmsMessage::from_bytes(&bytes).expect("decode");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, msg);
    }

    #[test]
    fn select_roundtrip() {
        let msg = HsmsMessage::SelectRsp { system_bytes: 9, status: SelectStatus::Selected };
        let (decoded, used) = HsmsMessage::from_bytes(&msg.to_bytes()).expect("decode");
        assert_eq!(used, 14);
        match decoded {
            HsmsMessage::SelectRsp { status, .. } => assert_eq!(status, SelectStatus::Selected),
            other => panic!("expected SelectRsp, got {other:?}"),
        }
    }

    #[test]
    fn header_layout_matches_semi_e37() {
        // S6F11 W-bit, session 1, system bytes 0xDEADBEEF.
        let msg = HsmsMessage::data(1, 6, 11, true, 0xDEAD_BEEF, None);
        let bytes = msg.to_bytes();
        assert_eq!(&bytes[4..14], &[0x00, 0x01, 0x86, 0x0B, 0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn truncated_frame_reports_eof() {
        let bytes = HsmsMessage::SelectReq { system_bytes: 1 }.to_bytes();
        let err = HsmsMessage::from_bytes(&bytes[..7]).unwrap_err();
        assert!(matches!(err, SecsError::UnexpectedEof { .. }));
    }
}
