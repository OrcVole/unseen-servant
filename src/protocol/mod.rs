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
pub mod response;
pub mod titan;
pub mod uri;

/// The port clients assume when a `gemini://` URI names none.
pub const GEMINI_DEFAULT_PORT: u16 = 1965;

/// Layer 3 rejection: the request was *well-formed* but names a scheme,
/// host, or port that belongs to someone else. Answered with status 53
/// (proxy request refused) — usv is not a proxy (ADR 0005 territory: we
/// never fetch on a client's behalf).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignAuthority;

/// Layer 3: is this validated target one this server actually serves?
pub fn check_authority(
    target: uri::Target,
    serves_host: impl Fn(&str) -> bool,
    advertised_port: u16,
) -> Result<uri::GeminiRequest, ForeignAuthority> {
    match target {
        uri::Target::Foreign { .. } => Err(ForeignAuthority),
        uri::Target::Gemini(req) => {
            let effective_port = req.port.unwrap_or(GEMINI_DEFAULT_PORT);
            if effective_port == advertised_port && serves_host(&req.host) {
                Ok(req)
            } else {
                Err(ForeignAuthority)
            }
        }
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;

    fn target(uri: &str) -> uri::Target {
        uri::validate_uri(uri.as_bytes()).expect("well-formed test URI")
    }

    fn ours(name: &str) -> bool {
        name == "example.org"
    }

    #[test]
    fn our_host_default_port_is_accepted() {
        assert!(check_authority(target("gemini://example.org/"), ours, 1965).is_ok());
    }

    #[test]
    fn explicit_default_port_is_accepted() {
        // gemini-diagnostics URLIncludePort: explicit :1965 must work.
        assert!(check_authority(target("gemini://example.org:1965/"), ours, 1965).is_ok());
    }

    #[test]
    fn wrong_port_is_refused() {
        assert!(check_authority(target("gemini://example.org:1966/"), ours, 1965).is_err());
    }

    #[test]
    fn wrong_host_is_refused() {
        assert!(check_authority(target("gemini://other.example/"), ours, 1965).is_err());
    }

    #[test]
    fn foreign_scheme_is_refused() {
        assert!(check_authority(target("http://example.org/"), ours, 1965).is_err());
    }

    #[test]
    fn nonstandard_advertised_port_requires_explicit_port() {
        // On a remapped port, a URL without a port means the client thinks
        // we are on 1965 — that authority is not ours.
        assert!(check_authority(target("gemini://example.org:11965/"), ours, 11965).is_ok());
        assert!(check_authority(target("gemini://example.org/"), ours, 11965).is_err());
    }
}
