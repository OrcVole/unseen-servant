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

/// Layer 3, scheme-independent: does this host/port pair name *this*
/// server? Shared by the Gemini and Titan paths — Titan rides the same
/// listener (recon: titan.md §5.4), so "is this authority ours" must be
/// one rule, not two that can drift apart.
pub fn authority_is_ours(
    host: &str,
    port: Option<u16>,
    serves_host: impl Fn(&str) -> bool,
    advertised_port: u16,
) -> bool {
    port.unwrap_or(GEMINI_DEFAULT_PORT) == advertised_port && serves_host(host)
}

/// Layer 3: is this validated target one this server actually serves?
pub fn check_authority(
    target: uri::Target,
    serves_host: impl Fn(&str) -> bool,
    advertised_port: u16,
) -> Result<uri::GeminiRequest, ForeignAuthority> {
    match target {
        uri::Target::Foreign { .. } => Err(ForeignAuthority),
        uri::Target::Gemini(req) => {
            if authority_is_ours(&req.host, req.port, serves_host, advertised_port) {
                Ok(req)
            } else {
                Err(ForeignAuthority)
            }
        }
    }
}

/// Scheme peek for same-listener dispatch: do these framed request bytes
/// begin with `titan:`?
///
/// Deliberately the *only* thing decided before parsing — the framing
/// layer has already bounded the bytes, and each scheme then gets its own
/// layer-2 parser ([`uri::validate_uri`] or [`titan::parse`]). Comparing
/// the literal scheme prefix here, rather than routing on a full parse,
/// keeps the two parsers independent: neither has to understand the
/// other's grammar to decide whether the request is its business.
///
/// Case-insensitive per RFC 3986 (schemes are case-insensitive), and it
/// requires the colon, so a path or host merely *starting with* "titan"
/// is not mistaken for the scheme.
pub fn is_titan_request(framed_uri: &[u8]) -> bool {
    framed_uri
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"titan:"))
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
    fn titan_scheme_is_detected_case_insensitively() {
        for line in [
            &b"titan://example.org/p;size=1"[..],
            b"TITAN://example.org/p;size=1",
            b"TiTaN://example.org/p;size=1",
        ] {
            assert!(is_titan_request(line), "{:?}", str::from_utf8(line));
        }
    }

    #[test]
    fn non_titan_schemes_are_not_dispatched_to_titan() {
        for line in [
            &b"gemini://example.org/"[..],
            b"http://example.org/",
            // A host or path that merely starts with the letters "titan"
            // is not the titan scheme — the colon is required.
            b"gemini://titan.example.org/",
            b"titanic://example.org/",
            b"titan",
            b"",
        ] {
            assert!(!is_titan_request(line), "{:?}", str::from_utf8(line));
        }
    }

    #[test]
    fn authority_predicate_is_shared_by_both_schemes() {
        // The same rule the Gemini path uses must accept/refuse a Titan
        // authority identically — one rule, no drift (recon titan.md §5.4).
        assert!(authority_is_ours("example.org", None, ours, 1965));
        assert!(authority_is_ours("example.org", Some(1965), ours, 1965));
        assert!(!authority_is_ours("example.org", Some(1966), ours, 1965));
        assert!(!authority_is_ours("other.example", None, ours, 1965));
    }

    #[test]
    fn nonstandard_advertised_port_requires_explicit_port() {
        // On a remapped port, a URL without a port means the client thinks
        // we are on 1965 — that authority is not ours.
        assert!(check_authority(target("gemini://example.org:11965/"), ours, 11965).is_ok());
        assert!(check_authority(target("gemini://example.org/"), ours, 11965).is_err());
    }
}
