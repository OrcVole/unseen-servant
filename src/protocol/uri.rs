//! URI **validation**: layer 2 of 3 (see [`crate::protocol`]).
//!
//! Input is the framed URI slice from layer 1 (non-empty, ≤1024 bytes, free
//! of CR/LF). This layer decides whether those bytes are a strict RFC 3986
//! absolute URI acceptable in a Gemini request, per the recon reject-list
//! (docs/recon/protocol.md "Implementation guidance" §1):
//!
//! - non-ASCII bytes, control bytes, and literal spaces → reject (the spec
//!   is URI-based, not IRI-based; percent-encoding is the only escape hatch)
//! - missing scheme / relative reference → reject
//! - userinfo → reject (spec: "a server MUST reject")
//! - fragment → reject (spec: "a server MUST reject such requests as well")
//! - malformed percent-encoding, host, or port → reject
//!
//! Every rejection here is answered with status 59. A **well-formed** URI in
//! a foreign scheme (`http://…`, `gopher://…`) is *not* rejected here: it
//! parses to [`Target::Foreign`] and layer 3 answers 53 (proxy request
//! refused), matching gemini-diagnostics' expectations.
//!
//! The parser is hand-rolled (ADR 0001): the reject-list is diagnostics-
//! relevant behavior we want under direct test and fuzz control, and the
//! grammar subset is small enough that a dependency would obscure more than
//! it saves. It is fuzzed by `fuzz/fuzz_targets/validate_uri.rs`.

/// Why layer 2 rejected the framed URI bytes. Every variant is answered on
/// the wire with status `59` and connection close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriError {
    /// A byte outside printable ASCII (0x21–0x7E): non-ASCII, a control
    /// byte, or a literal space. (NUL lands here too.)
    ForbiddenByte,
    /// No `scheme:` prefix — a relative reference or scheme-less URL.
    MissingScheme,
    /// The scheme was present but the characters after it do not form
    /// `//authority` — Gemini URIs are hierarchical and require an authority.
    MissingAuthority,
    /// A userinfo component (`user@host`) is present.
    Userinfo,
    /// A fragment (`#…`) is present.
    Fragment,
    /// A `%` not followed by two hex digits.
    BadPercentEncoding,
    /// The host is empty or contains a character outside the reg-name /
    /// IP-literal grammar.
    BadHost,
    /// The port is non-numeric or out of range.
    BadPort,
    /// A path or query character outside the RFC 3986 grammar.
    BadPathOrQuery,
}

impl core::fmt::Display for UriError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            UriError::ForbiddenByte => "URI contains a non-ASCII, control, or space byte",
            UriError::MissingScheme => "request URI is not absolute (no scheme)",
            UriError::MissingAuthority => "request URI has no //authority",
            UriError::Userinfo => "request URI contains userinfo, which servers must reject",
            UriError::Fragment => "request URI contains a fragment, which servers must reject",
            UriError::BadPercentEncoding => "request URI contains invalid percent-encoding",
            UriError::BadHost => "request URI host is empty or malformed",
            UriError::BadPort => "request URI port is malformed",
            UriError::BadPathOrQuery => "request URI path or query contains forbidden characters",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for UriError {}

/// A validated request target: ours to answer, or someone else's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A `gemini://` URI that passed the full strict parse.
    Gemini(GeminiRequest),
    /// A well-formed absolute URI in another scheme. Layer 3 answers 53:
    /// this server is not a proxy. The scheme is kept (lowercased) for the
    /// log line only.
    Foreign {
        /// The foreign scheme, lowercased (`http`, `gopher`, …).
        scheme: String,
    },
}

/// A parsed, validated `gemini://` request. Path and query are kept exactly
/// as sent (percent-encoding intact): decoding is the traversal-defense
/// territory of the static-file handler (C2), and a request logger must see
/// the wire form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiRequest {
    /// Hostname, lowercased. Reg-names stay in punycode exactly as sent;
    /// IP literals keep their brackets (`[::1]`) so the authority check
    /// compares like with like.
    pub host: String,
    /// Explicit port, if the URI named one. `None` means the client relied
    /// on the default (1965).
    pub port: Option<u16>,
    /// Path exactly as sent, possibly empty. The spec requires an empty path
    /// and `/` to be treated as the same resource, without redirecting.
    pub path: String,
    /// Query exactly as sent (without the `?`), if present. Treated as
    /// sensitive for logging purposes (status 10/11 input lands here).
    pub query: Option<String>,
}

/// Validate framed URI bytes into a [`Target`].
pub fn validate_uri(uri: &[u8]) -> Result<Target, UriError> {
    // One pass up front kills every forbidden byte, so the rest of the
    // parser can safely treat the input as printable ASCII `str`.
    if !uri.iter().all(|&b| (0x21..=0x7e).contains(&b)) {
        return Err(UriError::ForbiddenByte);
    }
    let s = str::from_utf8(uri).map_err(|_| UriError::ForbiddenByte)?;

    let (scheme, rest) = split_scheme(s)?;

    if !scheme.eq_ignore_ascii_case("gemini") {
        // Foreign schemes must still be structurally sane enough to log; a
        // fragment or bad byte anywhere is still a parse failure (59), but
        // we deliberately do not police foreign-scheme authority grammar we
        // will never serve. Fragments are rejected for every scheme: the
        // client MUST NOT send them, full stop.
        if rest.contains('#') {
            return Err(UriError::Fragment);
        }
        return Ok(Target::Foreign {
            scheme: scheme.to_ascii_lowercase(),
        });
    }

    let rest = rest.strip_prefix("//").ok_or(UriError::MissingAuthority)?;

    // Authority ends at the first '/', '?', or '#'; the terminator (if any)
    // stays attached to the remainder.
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, remainder) = rest.split_at(authority_end);

    if authority.contains('@') {
        return Err(UriError::Userinfo);
    }
    let (host, port) = split_host_port(authority)?;

    let (path, after_path) = match remainder.find(['?', '#']) {
        Some(i) => remainder.split_at(i),
        None => (remainder, ""),
    };
    validate_path(path)?;

    let query = match after_path.strip_prefix('?') {
        Some(q) => match q.find('#') {
            Some(_) => return Err(UriError::Fragment),
            None => {
                validate_query(q)?;
                Some(q.to_string())
            }
        },
        None => match after_path.strip_prefix('#') {
            Some(_) => return Err(UriError::Fragment),
            None => None,
        },
    };

    Ok(Target::Gemini(GeminiRequest {
        host,
        port,
        path: path.to_string(),
        query,
    }))
}

/// Split `scheme:` per RFC 3986: `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`.
fn split_scheme(s: &str) -> Result<(&str, &str), UriError> {
    let colon = s.find(':').ok_or(UriError::MissingScheme)?;
    let (scheme, rest) = s.split_at(colon);
    let mut bytes = scheme.bytes();
    let first_is_alpha = bytes.next().is_some_and(|b| b.is_ascii_alphabetic());
    let rest_ok = bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'));
    if !first_is_alpha || !rest_ok {
        // "example.com:1965/x" parses as scheme "example.com" — chars are
        // legal scheme chars, so that IS a (foreign) scheme per RFC 3986.
        // Only genuinely illegal scheme bytes land here, and the right
        // diagnosis for those is a scheme-less (relative) reference.
        return Err(UriError::MissingScheme);
    }
    Ok((scheme, &rest[1..]))
}

/// Split an authority (already checked for userinfo) into host and port.
///
/// `pub(crate)` because the Titan parser ([`super::titan`]) reuses the
/// exact same host/port grammar — forking security-critical authority
/// parsing between the two schemes would be a way to introduce a
/// divergence an attacker could exploit against one but not the other.
pub(crate) fn split_host_port(authority: &str) -> Result<(String, Option<u16>), UriError> {
    if authority.is_empty() {
        return Err(UriError::BadHost);
    }
    let (host, port_str) = if let Some(rest) = authority.strip_prefix('[') {
        // IP-literal: brackets must close, contents must parse as IPv6.
        // (IPvFuture is rejected: nothing speaks it, strictness wins.)
        let close = rest.find(']').ok_or(UriError::BadHost)?;
        let (ip, after) = rest.split_at(close);
        if ip.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(UriError::BadHost);
        }
        let after = &after[1..]; // drop ']'
        let port = match after.strip_prefix(':') {
            Some(p) => p,
            None if after.is_empty() => "",
            None => return Err(UriError::BadHost),
        };
        (&authority[..close + 2], port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h, p),
            None => (authority, ""),
        }
    };

    if host.is_empty() {
        return Err(UriError::BadHost);
    }
    if !host.starts_with('[') {
        // reg-name = *( unreserved / pct-encoded / sub-delims ); we insist
        // on the decoded-form-free subset (no pct-encoding in hostnames —
        // DNS names are ASCII already and encoding one is only ever evasion).
        if !host.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'!' | b'$')
        }) {
            return Err(UriError::BadHost);
        }
    }

    // RFC 3986 allows an empty port ("host:"); it means the scheme default.
    let port = if port_str.is_empty() {
        None
    } else {
        if !port_str.bytes().all(|b| b.is_ascii_digit()) {
            return Err(UriError::BadPort);
        }
        Some(port_str.parse::<u16>().map_err(|_| UriError::BadPort)?)
    };

    Ok((host.to_ascii_lowercase(), port))
}

/// `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`.
fn is_pchar_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'.' | b'_' | b'~' // unreserved
            | b'!' | b'$' | b'&' | b'\'' | b'(' | b')'
            | b'*' | b'+' | b',' | b';' | b'=' // sub-delims
            | b':' | b'@'
        )
}

/// Validate percent-encoding and character set of a path (`*( "/" pchar )`).
/// `pub(crate)`: shared with the Titan parser ([`super::titan`]).
pub(crate) fn validate_path(path: &str) -> Result<(), UriError> {
    validate_pct_and(path, |b| is_pchar_byte(b) || b == b'/').map_err(|e| match e {
        UriError::BadPercentEncoding => UriError::BadPercentEncoding,
        _ => UriError::BadPathOrQuery,
    })
}

/// Validate a query (`*( pchar / "/" / "?" )`).
/// `pub(crate)`: shared with the Titan parser ([`super::titan`]).
pub(crate) fn validate_query(query: &str) -> Result<(), UriError> {
    validate_pct_and(query, |b| is_pchar_byte(b) || b == b'/' || b == b'?').map_err(|e| match e {
        UriError::BadPercentEncoding => UriError::BadPercentEncoding,
        _ => UriError::BadPathOrQuery,
    })
}

fn validate_pct_and(s: &str, allowed: impl Fn(u8) -> bool) -> Result<(), UriError> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let ok = bytes.len() >= i + 3
                    && bytes[i + 1].is_ascii_hexdigit()
                    && bytes[i + 2].is_ascii_hexdigit();
                if !ok {
                    return Err(UriError::BadPercentEncoding);
                }
                i += 3;
            }
            b if allowed(b) => i += 1,
            _ => return Err(UriError::BadPathOrQuery),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gem(uri: &str) -> GeminiRequest {
        match validate_uri(uri.as_bytes()) {
            Ok(Target::Gemini(r)) => r,
            other => panic!("expected Gemini target for {uri:?}, got {other:?}"),
        }
    }

    #[test]
    fn minimal_request_parses() {
        let r = gem("gemini://example.org/");
        assert_eq!(r.host, "example.org");
        assert_eq!(r.port, None);
        assert_eq!(r.path, "/");
        assert_eq!(r.query, None);
    }

    #[test]
    fn empty_path_is_preserved_not_rewritten() {
        assert_eq!(gem("gemini://example.org").path, "");
    }

    #[test]
    fn explicit_port_and_query_parse() {
        let r = gem("gemini://example.org:1965/search?hello%20there");
        assert_eq!(r.port, Some(1965));
        assert_eq!(r.query.as_deref(), Some("hello%20there"));
    }

    #[test]
    fn host_is_lowercased_scheme_case_insensitive() {
        let r = gem("GEMINI://Example.ORG/");
        assert_eq!(r.host, "example.org");
    }

    #[test]
    fn ipv6_literal_hosts_parse() {
        let r = gem("gemini://[::1]:1965/");
        assert_eq!(r.host, "[::1]");
        assert_eq!(r.port, Some(1965));
    }

    #[test]
    fn foreign_schemes_are_typed_not_rejected() {
        for uri in [
            "http://example.org/",
            "https://example.org/",
            "gopher://example.org/",
        ] {
            match validate_uri(uri.as_bytes()) {
                Ok(Target::Foreign { scheme }) => {
                    assert!(uri.starts_with(&scheme));
                }
                other => panic!("expected Foreign for {uri:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn scheme_less_urls_are_rejected() {
        for uri in ["//example.org/", "/path/only", "example org"] {
            assert!(
                validate_uri(uri.as_bytes()).is_err(),
                "{uri:?} should be rejected"
            );
        }
        // "example.com/x" has no colon at all → missing scheme.
        assert_eq!(validate_uri(b"example.com/x"), Err(UriError::MissingScheme));
    }

    #[test]
    fn userinfo_is_rejected() {
        assert_eq!(
            validate_uri(b"gemini://user@example.org/"),
            Err(UriError::Userinfo)
        );
        assert_eq!(
            validate_uri(b"gemini://user:pass@example.org/"),
            Err(UriError::Userinfo)
        );
    }

    #[test]
    fn fragments_are_rejected_everywhere() {
        assert_eq!(
            validate_uri(b"gemini://example.org/page#frag"),
            Err(UriError::Fragment)
        );
        assert_eq!(
            validate_uri(b"gemini://example.org/?q#frag"),
            Err(UriError::Fragment)
        );
        assert_eq!(
            validate_uri(b"http://example.org/#frag"),
            Err(UriError::Fragment)
        );
    }

    #[test]
    fn forbidden_bytes_are_rejected() {
        for uri in [
            &b"gemini://example.org/\x00"[..],
            b"gemini://example.org/caf\xc3\xa9", // raw UTF-8: IRIs not accepted
            b"gemini://example.org/a b",         // literal space
            b"gemini://example.org/\x07",        // control byte
        ] {
            assert_eq!(validate_uri(uri), Err(UriError::ForbiddenByte), "{uri:?}");
        }
    }

    #[test]
    fn bad_percent_encoding_is_rejected() {
        for uri in [
            "gemini://example.org/%",
            "gemini://example.org/%2",
            "gemini://e.org/%zz",
        ] {
            assert_eq!(
                validate_uri(uri.as_bytes()),
                Err(UriError::BadPercentEncoding),
                "{uri:?}"
            );
        }
    }

    #[test]
    fn valid_percent_encoding_is_kept_raw() {
        let r = gem("gemini://example.org/%2e%2e/up");
        assert_eq!(r.path, "/%2e%2e/up", "no decoding at this layer");
    }

    #[test]
    fn bad_hosts_are_rejected() {
        for uri in [
            "gemini:///nohost",
            "gemini://:1965/",
            "gemini://[::1/", // unclosed bracket
            "gemini://[nonsense]/",
            "gemini://ex%41mple.org/", // pct-encoding in hostname is evasion
        ] {
            assert!(
                matches!(validate_uri(uri.as_bytes()), Err(UriError::BadHost)),
                "{uri:?}"
            );
        }
    }

    #[test]
    fn bad_ports_are_rejected() {
        for uri in ["gemini://e.org:abc/", "gemini://e.org:99999/"] {
            assert!(
                matches!(validate_uri(uri.as_bytes()), Err(UriError::BadPort)),
                "{uri:?}"
            );
        }
    }

    #[test]
    fn empty_port_means_default() {
        assert_eq!(gem("gemini://example.org:/").port, None);
    }
}
