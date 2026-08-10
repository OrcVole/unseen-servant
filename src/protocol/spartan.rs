//! Spartan wire layer, pure and fuzzed.
//!
//! Spartan is Gemini with the cryptography removed: same document
//! format, no TLS, no certificates, no input class. The request is one
//! line of three space-separated fields —
//!
//! ```text
//! host SP path-absolute SP content-length CRLF
//! ```
//!
//! — optionally followed by `content-length` bytes of data.
//!
//! **usv rejects every upload (ADR 0012 §5).** Spartan folds "input" and
//! "upload" into the same mechanism: `spartan://host/path?text` is sent
//! as a request whose data block is the decoded query, so any non-zero
//! content-length is a write attempt. They are unauthenticated by
//! construction — the protocol has no certificates and no TLS, so the
//! strongest available control is a shared secret in a URL. usv already
//! does authenticated writes properly over Titan (ADR 0006), and
//! accepting unauthenticated ones on a second door would undo that.
//!
//! Rejecting on the *declared* length, before reading a byte of body,
//! also disposes of the obvious resource-exhaustion move: a peer that
//! announces a four-gigabyte upload gets a refusal and a closed socket
//! rather than a server that starts making room for it.
//!
//! Unlike gopher and Nex, the request names the host, so Spartan
//! supports virtual hosting — which falls out of usv's existing
//! per-host model rather than needing anything new.

/// Longest request line accepted. The spec suggests bounding it; this
/// matches the other listeners.
pub const MAX_REQUEST_BYTES: usize = 4096;

/// A parsed Spartan request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The host being asked for. Carries no port; IDNs arrive as
    /// punycode.
    pub host: String,
    /// The absolute path, always beginning with `/`.
    pub path: String,
    /// Bytes of data the client says follow. **Non-zero is an upload**,
    /// which usv refuses without reading them.
    pub content_length: u64,
}

/// Why a request line was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestError {
    /// No terminator within [`MAX_REQUEST_BYTES`].
    TooLong,
    /// No terminator yet; read more.
    Incomplete,
    /// Not three space-separated fields.
    Malformed,
    /// The path did not start with `/`. The spec requires
    /// path-absolute, and a relative path here would be ambiguous
    /// exactly where ambiguity turns into traversal.
    PathNotAbsolute,
    /// `content-length` was not a plain non-negative decimal. A signed
    /// or hex or padded value is a parser-disagreement trick, not a
    /// length.
    BadContentLength,
    /// A control byte or NUL anywhere in the line.
    ControlByte,
    /// Not valid UTF-8.
    NotUtf8,
}

/// Parse one Spartan request line.
///
/// Strict about CRLF, unlike the gopher and finger parsers: Spartan is a
/// 2021 protocol with a finished spec and a handful of maintained
/// clients, so there is no decades-old client population to be lenient
/// for, and strictness is free.
pub fn parse(raw: &[u8]) -> Result<(Request, usize), RequestError> {
    let Some(nl) = raw.iter().position(|&b| b == b'\n') else {
        return if raw.len() >= MAX_REQUEST_BYTES {
            Err(RequestError::TooLong)
        } else {
            Err(RequestError::Incomplete)
        };
    };
    if nl > MAX_REQUEST_BYTES {
        return Err(RequestError::TooLong);
    }
    let line = &raw[..nl];
    let line = line.strip_suffix(b"\r").ok_or(RequestError::Malformed)?;

    for &b in line {
        if b < 0x20 || b == 0x7f {
            return Err(RequestError::ControlByte);
        }
    }
    let text = std::str::from_utf8(line).map_err(|_| RequestError::NotUtf8)?;

    // Exactly three fields. `splitn` would silently tolerate a fourth by
    // folding it into the last, which is how a length field ends up
    // containing something that is not a length.
    let mut parts = text.split(' ');
    let (Some(host), Some(path), Some(len), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(RequestError::Malformed);
    };
    if host.is_empty() {
        return Err(RequestError::Malformed);
    }
    if !path.starts_with('/') {
        return Err(RequestError::PathNotAbsolute);
    }
    // Plain decimal only: no sign, no leading '+', no whitespace. Rust's
    // `parse` already rejects most of that, but `+5` parses, so it is
    // excluded explicitly.
    if len.is_empty() || !len.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RequestError::BadContentLength);
    }
    let content_length: u64 = len.parse().map_err(|_| RequestError::BadContentLength)?;

    Ok((
        Request {
            host: host.to_ascii_lowercase(),
            path: path.to_string(),
            content_length,
        },
        nl + 1,
    ))
}

/// A Spartan response header. The body, if any, follows immediately.
pub fn success(mime: &str) -> String {
    format!("2 {mime}\r\n")
}

/// `3` — redirect. Same-host, path-absolute only; the spec allows
/// nothing else, which is why the render side resolves anything more
/// complicated at generation time.
pub fn redirect(path: &str) -> String {
    format!("3 {path}\r\n")
}

/// `4` — client error.
pub fn client_error(msg: &str) -> String {
    format!("4 {msg}\r\n")
}

/// `5` — server error.
pub fn server_error(msg: &str) -> String {
    format!("5 {msg}\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_fetch_parses() {
        let (req, n) = parse(b"example.org /index.gmi 0\r\n").expect("valid");
        assert_eq!(req.host, "example.org");
        assert_eq!(req.path, "/index.gmi");
        assert_eq!(req.content_length, 0);
        assert_eq!(n, b"example.org /index.gmi 0\r\n".len());
    }

    #[test]
    fn the_host_is_lowercased_for_matching() {
        let (req, _) = parse(b"EXAMPLE.ORG / 0\r\n").expect("valid");
        assert_eq!(req.host, "example.org");
    }

    #[test]
    fn an_upload_is_parsed_so_it_can_be_refused_before_the_body() {
        // The point: usv learns it is an upload from the DECLARED length
        // and answers without reading a byte of it.
        let (req, _) = parse(b"example.org /up 12345\r\n").expect("valid");
        assert_eq!(req.content_length, 12345);
    }

    #[test]
    fn an_absurd_declared_length_still_parses_rather_than_overflowing() {
        let (req, _) = parse(b"example.org /up 18446744073709551615\r\n").expect("valid");
        assert_eq!(req.content_length, u64::MAX);
    }

    #[test]
    fn a_relative_path_is_refused() {
        assert_eq!(
            parse(b"example.org index.gmi 0\r\n"),
            Err(RequestError::PathNotAbsolute)
        );
    }

    #[test]
    fn a_fourth_field_is_malformed_not_folded_in() {
        // splitn would hide the extra field inside the length.
        assert_eq!(
            parse(b"example.org / 0 extra\r\n"),
            Err(RequestError::Malformed)
        );
    }

    #[test]
    fn a_non_decimal_length_is_refused() {
        for bad in [
            &b"example.org / +5\r\n"[..],
            &b"example.org / -1\r\n"[..],
            &b"example.org / 0x10\r\n"[..],
            &b"example.org / \r\n"[..],
            &b"example.org / five\r\n"[..],
        ] {
            assert!(
                matches!(
                    parse(bad),
                    Err(RequestError::BadContentLength) | Err(RequestError::Malformed)
                ),
                "accepted {bad:?}"
            );
        }
    }

    #[test]
    fn a_bare_lf_is_refused_here_unlike_gopher() {
        // A 2021 protocol with a finished spec: no ancient clients to be
        // lenient for, so strictness costs nothing.
        assert_eq!(parse(b"example.org / 0\n"), Err(RequestError::Malformed));
    }

    #[test]
    fn control_bytes_are_refused() {
        assert_eq!(
            parse(b"example.org /a\0b 0\r\n"),
            Err(RequestError::ControlByte)
        );
    }

    #[test]
    fn a_partial_line_asks_for_more() {
        assert_eq!(parse(b"example.org /"), Err(RequestError::Incomplete));
    }

    #[test]
    fn response_headers_are_shaped_correctly() {
        assert_eq!(success("text/gemini"), "2 text/gemini\r\n");
        assert_eq!(redirect("/moved"), "3 /moved\r\n");
        assert_eq!(client_error("nope"), "4 nope\r\n");
        assert_eq!(server_error("oops"), "5 oops\r\n");
    }
}
