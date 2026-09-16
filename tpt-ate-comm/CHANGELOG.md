# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-17

### Added

- SECS-II (SEMI E5) data-item model with full wire encoder/decoder: `L`, `B`, `BOOLEAN`,
  `A`, `J`, `I1`–`I8`, `F4`/`F8`, `U1`–`U8`, including 1–3 byte length encodings and
  SML-style `Display`.
- HSMS-SS (SEMI E37/E37.1) message framing — 4-byte length prefix + 10-byte header — with
  `Data`, `SelectReq/Rsp`, `LinktestReq/Rsp`, `Separate`, and `Reject` messages; documented
  wire-format test pinning the E37 header layout.
- `HsmsSession` session state machine (NOT CONNECTED → NOT SELECTED → SELECTED) over any
  `Read + Write` stream: `select`/`accept_select` handshake, `transact` with system-byte
  reply matching, and control-frame handling while selected.
- GEM (SEMI E30) transaction catalog: S1F1/F2 identify, S1F13/F14 establish communications,
  S2F41/F42 host commands with HCACK, S6F11/F12 event reports (parse + build), and the
  ATE event vocabulary (CEID/VID/RPTID modules).
- `EquipmentGem` and `HostGem` session wrappers answering the standard handshake on the
  equipment side and driving it on the host side.
- In-memory blocking duplex transport (`duplex`) with TCP-like read/write/EOF semantics.
- Seeded xorshift64* RNG (`rng`) for deterministic equipment simulators.
- This crate is the extraction of the SECS/GEM layer originally built inline in
  `tpt-ate-test` (RFC-003 §5's locked decision, executed once `tpt-ate-assembly` made the
  duplication concrete).

[0.1.0]: https://github.com/tpt-solutions/tpt-ate/releases/tag/tpt-ate-comm-v0.1.0
