//! The Gemini wire protocol, spec v0.24.1 (frozen upstream since 2024-08-28;
//! see `docs/recon/protocol.md` for the dated evidence and every ambiguity
//! ruling this implementation follows).
//!
//! # Layering
//!
//! Request handling is three deliberate layers, so each rule is testable and
//! fuzzable in isolation, and so a future maintainer can see exactly where a
//! given rejection comes from:
//!
//! 1. **Framing** ([`request`], phase C0/C1): byte-level. Finds the CRLF
//!    terminator, enforces the 1024-byte URI budget, rejects bare LF and stray
//!    CR. Knows nothing about URIs.
//! 2. **URI validation** (C1): parses the framed bytes as an RFC 3986
//!    absolute URI; rejects userinfo, fragments, non-ASCII bytes, foreign
//!    schemes. Produces a typed request.
//! 3. **Authority checks** (C1): is this a hostname/port this server serves?
//!    (Status 53 lives here, not in parsing.)
//!
//! Every rejection across all three layers maps to Gemini status 59 or 53 per
//! the table in `docs/recon/protocol.md` §"Implementation guidance".

pub mod request;
