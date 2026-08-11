//! Titan request-line parsing (ADR 0006; C4) — the `titan://` scheme's
//! layer-2 validation, the upload analogue of [`super::uri`].
//!
//! Titan is Gemini-shaped: one request line, then exactly `size` payload
//! bytes, then an ordinary Gemini response (docs/internal/recon/titan.md §1). The
//! request line is a `titan://` URL whose **path carries a `;`-separated
//! parameter block**: a mandatory `size`, an optional `mime` (default
//! `text/gemini`), and an optional `token`. Example:
//!
//! ```text
//! titan://example.org/wiki/page;token=hunter2;mime=text/plain;size=10
//! ```
//!
//! This module parses and validates the request **line only**. It is
//! deliberately blind to everything the *handler* owns:
//!
//! - It never reads the payload, so it cannot know if the peer delivers
//!   `size` bytes — it only records the declared length.
//! - It never sees a client certificate (auth is 60/61/62, decided at the
//!   handler with the cert zones).
//! - It never consults config, so it does **not** enforce the size *cap*
//!   (declared-`size` vs a per-zone maximum is a handler decision). `size`
//!   is parsed to a `u64`; whether that `u64` is acceptable is decided
//!   later, before the body is read (recon §5.3).
//!
//! Every failure here is a malformed request line, answered on the wire
//! with status **59** (recon §1.3). Host and path grammar are reused from
//! [`super::uri`] rather than reimplemented: forking security-critical
//! authority parsing between the two schemes is exactly how a divergence
//! an attacker can exploit gets introduced. Fuzzed by
//! `fuzz/fuzz_targets/parse_titan.rs`.

use super::uri::{self, UriError};

/// The payload MIME assumed when a request omits `mime` (recon §1.2).
pub const DEFAULT_MIME: &str = "text/gemini";

/// Why the Titan request line was rejected. Every variant is answered on
/// the wire with status `59` and connection close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitanError {
    /// A failure in the shared URI grammar (forbidden byte, missing
    /// scheme/authority, userinfo, fragment, bad host/port/path). Carries
    /// [`super::uri`]'s own diagnosis so the log line is precise.
    Uri(UriError),
    /// The scheme was present and well-formed but is not `titan`. (The
    /// dispatcher only calls this parser for `titan://`, but the parser
    /// checks anyway so it is correct in isolation and under fuzzing.)
    NotTitan,
    /// No `size=` parameter. `size` is the one mandatory parameter — a
    /// Titan server cannot know when the payload ends without it.
    MissingSize,
    /// `size` is empty, non-decimal, or overflows a `u64`.
    BadSize,
    /// A parameter segment is not `key=value`, or a value carries
    /// malformed percent-encoding or decodes to non-UTF-8 bytes.
    BadParameter,
    /// The same recognised parameter key appeared more than once — an
    /// ambiguity we refuse rather than silently pick a winner for.
    DuplicateParameter,
}

impl core::fmt::Display for TitanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TitanError::Uri(e) => write!(f, "{e}"),
            TitanError::NotTitan => f.write_str("request scheme is not titan"),
            TitanError::MissingSize => {
                f.write_str("titan request is missing the mandatory size parameter")
            }
            TitanError::BadSize => {
                f.write_str("titan size parameter is not a non-negative integer")
            }
            TitanError::BadParameter => {
                f.write_str("titan parameter is malformed (expected key=value)")
            }
            TitanError::DuplicateParameter => f.write_str("titan request repeats a parameter key"),
        }
    }
}

impl core::error::Error for TitanError {}

/// A parsed, validated `titan://` request line. The payload is *not* part
/// of this — reading `size` bytes off the connection is the handler's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitanRequest {
    /// Hostname, lowercased (same rules as [`super::uri::GeminiRequest`]).
    pub host: String,
    /// Explicit port, if named; `None` means the default (1965).
    pub port: Option<u16>,
    /// Resource path, **percent-encoding intact** and **without** the
    /// `;`-parameter block. Decoding and traversal defence are the upload
    /// handler's territory, exactly as the static-file handler owns them
    /// for reads (C2). May be empty.
    pub path: String,
    /// A normal `?query` may follow the parameter block (recon §1.2); kept
    /// as sent, without the `?`.
    pub query: Option<String>,
    /// Declared payload length in bytes. Mandatory. **Not** yet checked
    /// against any configured cap.
    pub size: u64,
    /// Payload MIME, percent-decoded; [`DEFAULT_MIME`] when absent.
    pub mime: String,
    /// Optional authorization token, percent-decoded. Treated as a secret:
    /// the caller must never write it to a log or an error META (recon
    /// §5.2).
    pub token: Option<String>,
}

/// Parse framed request-line bytes as a Titan request. Input is the same
/// framed slice a Gemini request would get (non-empty, ≤1024 bytes, no
/// CR/LF — layer 1's guarantees).
pub fn parse(line: &[u8]) -> Result<TitanRequest, TitanError> {
    // Same printable-ASCII gate as `uri::validate_uri`: the request line
    // is URI-based, so percent-encoding is the only path for any other
    // byte. This also lets the rest of the parser treat the input as
    // ASCII `str` safely.
    if !line.iter().all(|&b| (0x21..=0x7e).contains(&b)) {
        return Err(TitanError::Uri(UriError::ForbiddenByte));
    }
    let s = str::from_utf8(line).map_err(|_| TitanError::Uri(UriError::ForbiddenByte))?;

    let colon = s
        .find(':')
        .ok_or(TitanError::Uri(UriError::MissingScheme))?;
    let (scheme, after_scheme) = s.split_at(colon);
    if !is_scheme(scheme) {
        return Err(TitanError::Uri(UriError::MissingScheme));
    }
    if !scheme.eq_ignore_ascii_case("titan") {
        return Err(TitanError::NotTitan);
    }
    let rest = after_scheme[1..]
        .strip_prefix("//")
        .ok_or(TitanError::Uri(UriError::MissingAuthority))?;

    // Fragments are illegal for every scheme (a client MUST NOT send one).
    if rest.contains('#') {
        return Err(TitanError::Uri(UriError::Fragment));
    }

    // The authority ends at the first '/', '?', or ';'. Including ';' means
    // a path-less `titan://host;size=…` still parses (authority = "host",
    // path = "") rather than folding the parameter block into the host and
    // failing with a misleading bad-host error.
    let authority_end = rest.find(['/', '?', ';']).unwrap_or(rest.len());
    let (authority, remainder) = rest.split_at(authority_end);
    if authority.contains('@') {
        return Err(TitanError::Uri(UriError::Userinfo));
    }
    let (host, port) = uri::split_host_port(authority).map_err(TitanError::Uri)?;

    // Peel the query off the end first, then the parameter block off the
    // path. A literal ';' or '?' inside the resource path must be
    // percent-encoded by the client (documented); the first unencoded one
    // of each is structural.
    let (path_and_params, query) = match remainder.split_once('?') {
        Some((pp, q)) => {
            uri::validate_query(q).map_err(TitanError::Uri)?;
            (pp, Some(q.to_string()))
        }
        None => (remainder, None),
    };
    let (path, param_block) = match path_and_params.split_once(';') {
        Some((p, params)) => (p, Some(params)),
        None => (path_and_params, None),
    };
    uri::validate_path(path).map_err(TitanError::Uri)?;

    let mut size: Option<u64> = None;
    let mut mime: Option<String> = None;
    let mut token: Option<String> = None;
    if let Some(block) = param_block {
        for segment in block.split(';') {
            let (key, value) = segment.split_once('=').ok_or(TitanError::BadParameter)?;
            let value = pct_decode_utf8(value)?;
            if key.eq_ignore_ascii_case("size") {
                set_once(&mut size, parse_size(&value)?)?;
            } else if key.eq_ignore_ascii_case("mime") {
                set_once(&mut mime, value)?;
            } else if key.eq_ignore_ascii_case("token") {
                set_once(&mut token, value)?;
            }
            // Unknown keys are tolerated for interop (recon §1.2) — but,
            // having matched none of the above, they still had to be a
            // well-formed `key=value`, checked at split_once above.
        }
    }

    Ok(TitanRequest {
        host,
        port,
        path: path.to_string(),
        query,
        size: size.ok_or(TitanError::MissingSize)?,
        mime: mime.unwrap_or_else(|| DEFAULT_MIME.to_string()),
        token,
    })
}

/// Store `value` into `slot`, or fail if it was already set — the "a key
/// appears twice" guard, shared by all three recognised parameters.
fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), TitanError> {
    if slot.is_some() {
        return Err(TitanError::DuplicateParameter);
    }
    *slot = Some(value);
    Ok(())
}

/// RFC 3986 scheme grammar: `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`.
fn is_scheme(scheme: &str) -> bool {
    let mut bytes = scheme.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

/// Parse a percent-decoded `size` value: a non-negative decimal `u64`.
/// Empty, non-decimal, or overflowing values are [`TitanError::BadSize`].
fn parse_size(s: &str) -> Result<u64, TitanError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TitanError::BadSize);
    }
    s.parse::<u64>().map_err(|_| TitanError::BadSize)
}

/// Percent-decode a parameter value into a UTF-8 `String`. Malformed
/// escapes (`%` not followed by two hex digits) and byte sequences that
/// are not valid UTF-8 are [`TitanError::BadParameter`]. A local copy
/// rather than a shared one, so the protocol layer takes no dependency on
/// the handler layer (which has its own decoder for a different purpose).
fn pct_decode_utf8(s: &str) -> Result<String, TitanError> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = bytes.get(i + 1).copied().ok_or(TitanError::BadParameter)?;
            let lo = bytes.get(i + 2).copied().ok_or(TitanError::BadParameter)?;
            let hi = (hi as char).to_digit(16).ok_or(TitanError::BadParameter)?;
            let lo = (lo as char).to_digit(16).ok_or(TitanError::BadParameter)?;
            out.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| TitanError::BadParameter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(line: &str) -> TitanRequest {
        match parse(line.as_bytes()) {
            Ok(r) => r,
            Err(e) => panic!("expected Ok for {line:?}, got {e:?}"),
        }
    }
    fn err(line: &str) -> TitanError {
        parse(line.as_bytes()).expect_err(&format!("expected Err for {line:?}"))
    }

    #[test]
    fn canonical_request_parses() {
        let r = ok("titan://example.org/wiki/page;token=hunter2;mime=text/plain;size=10");
        assert_eq!(r.host, "example.org");
        assert_eq!(r.port, None);
        assert_eq!(r.path, "/wiki/page");
        assert_eq!(r.size, 10);
        assert_eq!(r.mime, "text/plain");
        assert_eq!(r.token.as_deref(), Some("hunter2"));
        assert_eq!(r.query, None);
    }

    #[test]
    fn size_is_the_only_mandatory_parameter() {
        let r = ok("titan://example.org/p;size=42");
        assert_eq!(r.size, 42);
        assert_eq!(r.mime, DEFAULT_MIME, "mime defaults to text/gemini");
        assert_eq!(r.token, None);
    }

    #[test]
    fn missing_size_is_rejected() {
        assert_eq!(
            err("titan://example.org/p;mime=text/plain"),
            TitanError::MissingSize
        );
        assert_eq!(err("titan://example.org/p"), TitanError::MissingSize);
    }

    #[test]
    fn parameters_parse_in_any_order() {
        // The spec's own examples vary the order (recon §1.2).
        let a = ok("titan://h/p;size=1;mime=text/plain;token=t");
        let b = ok("titan://h/p;token=t;mime=text/plain;size=1");
        let c = ok("titan://h/p;mime=text/plain;size=1;token=t");
        for r in [a, b, c] {
            assert_eq!(r.size, 1);
            assert_eq!(r.mime, "text/plain");
            assert_eq!(r.token.as_deref(), Some("t"));
        }
    }

    #[test]
    fn size_zero_is_valid_the_delete_operation() {
        // size=0 is how Titan expresses deletion (recon §1.4); the parser
        // accepts it — whether deletion is *permitted* is the handler's
        // per-zone decision.
        assert_eq!(ok("titan://h/p;size=0").size, 0);
    }

    #[test]
    fn bad_size_values_are_rejected() {
        for line in [
            "titan://h/p;size=",
            "titan://h/p;size=-1",
            "titan://h/p;size=1.5",
            "titan://h/p;size=abc",
            "titan://h/p;size=99999999999999999999999999",
        ] {
            assert_eq!(err(line), TitanError::BadSize, "{line:?}");
        }
    }

    #[test]
    fn large_but_in_range_size_parses() {
        // No cap is applied here — that is the handler's job. A 4 GiB
        // declaration parses fine; the handler rejects it against config.
        assert_eq!(ok("titan://h/p;size=4294967296").size, 4_294_967_296);
    }

    #[test]
    fn a_normal_query_may_follow_the_parameter_block() {
        let r = ok("titan://h/p;size=3?q=1");
        assert_eq!(r.path, "/p");
        assert_eq!(r.size, 3);
        assert_eq!(r.query.as_deref(), Some("q=1"));
    }

    #[test]
    fn parameter_values_are_percent_decoded() {
        // A token containing the structural ';' must arrive percent-encoded
        // (%3B); it decodes back to the literal (recon §1.2).
        let r = ok("titan://h/p;size=1;token=a%3Bb%20c");
        assert_eq!(r.token.as_deref(), Some("a;b c"));
    }

    #[test]
    fn malformed_percent_encoding_in_a_value_is_rejected() {
        for line in [
            "titan://h/p;size=1;token=%zz",
            "titan://h/p;size=1;token=%2",
        ] {
            assert_eq!(err(line), TitanError::BadParameter, "{line:?}");
        }
    }

    #[test]
    fn a_segment_without_equals_is_malformed() {
        assert_eq!(err("titan://h/p;size=1;justtext"), TitanError::BadParameter);
        // A trailing ';' leaves an empty final segment — also malformed.
        assert_eq!(err("titan://h/p;size=1;"), TitanError::BadParameter);
    }

    #[test]
    fn duplicate_recognised_keys_are_rejected() {
        assert_eq!(
            err("titan://h/p;size=1;size=2"),
            TitanError::DuplicateParameter
        );
        assert_eq!(
            err("titan://h/p;size=1;mime=a/b;mime=c/d"),
            TitanError::DuplicateParameter
        );
    }

    #[test]
    fn unknown_keys_are_tolerated_for_interop() {
        // Unknown-key tolerance is the safer interop choice (recon §1.2):
        // a future/vendor parameter must not fail an otherwise valid write.
        let r = ok("titan://h/p;size=1;future=whatever");
        assert_eq!(r.size, 1);
    }

    #[test]
    fn keys_are_case_insensitive() {
        let r = ok("titan://h/p;SIZE=1;MIME=text/plain;TOKEN=t");
        assert_eq!(r.size, 1);
        assert_eq!(r.mime, "text/plain");
        assert_eq!(r.token.as_deref(), Some("t"));
    }

    #[test]
    fn path_may_be_empty_when_there_is_no_path_segment() {
        let r = ok("titan://h;size=1");
        assert_eq!(r.host, "h");
        assert_eq!(r.path, "");
        assert_eq!(r.size, 1);
    }

    #[test]
    fn a_literal_semicolon_in_the_path_must_be_encoded() {
        // Unencoded, the first ';' starts the parameter block, so "/a;b"
        // means path "/a" + a bogus parameter "b" → malformed.
        assert_eq!(err("titan://h/a;b;size=1"), TitanError::BadParameter);
        // Percent-encoded, it stays in the path.
        let r = ok("titan://h/a%3Bb;size=1");
        assert_eq!(r.path, "/a%3Bb");
    }

    #[test]
    fn path_percent_encoding_is_kept_raw_for_the_handler() {
        // Traversal defence is the handler's job (C2 model); the parser
        // must not decode the path, or it would hide %2e%2e from it.
        let r = ok("titan://h/%2e%2e/up;size=1");
        assert_eq!(r.path, "/%2e%2e/up");
    }

    #[test]
    fn non_titan_scheme_is_flagged_distinctly() {
        assert_eq!(err("gemini://h/p"), TitanError::NotTitan);
        assert_eq!(err("http://h/p"), TitanError::NotTitan);
    }

    #[test]
    fn shared_uri_failures_surface_as_uri_variant() {
        assert_eq!(
            err("titan://user@h/p;size=1"),
            TitanError::Uri(UriError::Userinfo)
        );
        assert_eq!(
            err("titan://h/p#frag;size=1"),
            TitanError::Uri(UriError::Fragment)
        );
        assert_eq!(
            err("titan:h/p;size=1"),
            TitanError::Uri(UriError::MissingAuthority)
        );
        assert!(matches!(
            err("titan://[bad]/p;size=1"),
            TitanError::Uri(UriError::BadHost)
        ));
        // Raw non-ASCII (an IRI) is a forbidden byte, as for Gemini.
        assert_eq!(
            parse("titan://h/café;size=1".as_bytes()),
            Err(TitanError::Uri(UriError::ForbiddenByte))
        );
    }

    #[test]
    fn host_is_lowercased_and_port_parses() {
        let r = ok("titan://Example.ORG:1965/p;size=1");
        assert_eq!(r.host, "example.org");
        assert_eq!(r.port, Some(1965));
    }

    #[test]
    fn a_token_value_is_never_required_to_be_present() {
        // An empty token value is still a value (the operator may compare
        // it); it is distinct from an absent token.
        let r = ok("titan://h/p;size=1;token=");
        assert_eq!(r.token.as_deref(), Some(""));
    }

    /// Fuzz entry point (mirrors `fuzz/fuzz_targets/parse_titan.rs`): parse
    /// must never panic on any byte sequence.
    #[test]
    fn fuzz_smoke_corpus_never_panics() {
        for line in [
            "",
            "titan:",
            "titan://",
            "titan:///;size=1",
            "titan://h",
            "titan://h/;size=",
            "titan://h/p;;;size=1",
            "titan://h/p;=;size=1",
            "titan://h/p;size=1;size=1;size=1",
            "titan://h/p;token=%",
            "titan://h/p;size=%30",
            ";;;;;;;",
            "titan://h/p;size=1?;;",
        ] {
            let _ = parse(line.as_bytes());
        }
    }
}
