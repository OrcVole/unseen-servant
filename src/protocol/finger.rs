//! Finger wire layer (RFC 1288), pure and fuzzed.
//!
//! The smallest protocol usv speaks: the client writes a line, the
//! server writes text, the connection closes. There is no status, no
//! content type, and no framing — the close *is* the end of the
//! response.
//!
//! RFC 1288's grammar, in full:
//!
//! ```text
//! {Q1} ::= [{W}|{W}{S}{U}]{C}
//! {Q2} ::= [{W}{S}][{U}]{H}{C}
//! {U}  ::= username
//! {H}  ::= @hostname | @hostname{H}
//! {W}  ::= /W          (the "verbose" flag)
//! {C}  ::= CRLF
//! ```
//!
//! **{Q2} — the forwarding query — is refused, always.** A request of
//! the form `user@host` asks *this* server to go and finger a third
//! party on the client's behalf. RFC 1288 §3.2.1 is unusually blunt
//! about it ("this is a bad idea"), because it turns every finger server
//! into an open relay for probing hosts that would not answer the
//! attacker directly, and hides the origin while doing it. usv is not a
//! proxy on any protocol (ADR 0005) and this is the same refusal, on the
//! one protocol where the spec itself invites the mistake.
//!
//! usv has no user accounts, so the username is parsed but does not
//! select anything: every query gets the same capsule profile. That is
//! deliberate — a finger server that answers differently per name is a
//! user-enumeration oracle, and this one has nothing to enumerate.

/// Longest request accepted before the peer is refused. RFC 1288 sets no
/// limit; this matches the other listeners so one peer cannot make the
/// server buffer without bound.
pub const MAX_REQUEST_BYTES: usize = 1024;

/// A parsed finger query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The username asked about, if any. Empty means "the host itself",
    /// which is the only thing usv actually answers with.
    pub user: String,
    /// Whether `/W` (verbose) was requested. Recorded for completeness;
    /// usv's profile is short enough that it does not vary.
    pub verbose: bool,
}

/// Why a query was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestError {
    /// No terminator within [`MAX_REQUEST_BYTES`].
    TooLong,
    /// No terminator yet; the caller should read more (see the gopher
    /// parser for why this is distinct from [`Self::TooLong`]).
    Incomplete,
    /// A `user@host` forwarding query. Refused by policy, not by
    /// inability — see the module docs.
    ForwardingRefused,
    /// A control byte or NUL in the query.
    ControlByte,
    /// Not valid UTF-8.
    NotUtf8,
}

/// Parse one finger query.
///
/// Accepts CRLF or a bare LF, for the same interoperability reasons as
/// the gopher parser: clients are ancient and inconsistent, and nothing
/// here is security-relevant to the framing.
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
    let mut line = &raw[..nl];
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }
    for &b in line {
        if b < 0x20 || b == 0x7f {
            return Err(RequestError::ControlByte);
        }
    }
    let text = std::str::from_utf8(line).map_err(|_| RequestError::NotUtf8)?;

    // The verbose flag, if present, comes first and is followed by
    // whitespace before any username.
    let (verbose, rest) = match text.strip_prefix("/W") {
        Some(r) => (true, r.trim_start()),
        None => (false, text.trim()),
    };

    // Refuse forwarding before anything else looks at the name: the '@'
    // is the whole signal, wherever it appears.
    if rest.contains('@') {
        return Err(RequestError::ForwardingRefused);
    }

    Ok((
        Request {
            user: rest.trim().to_string(),
            verbose,
        },
        nl + 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_query_asks_about_the_host() {
        let (req, n) = parse(b"\r\n").expect("valid");
        assert_eq!(req.user, "");
        assert!(!req.verbose);
        assert_eq!(n, 2);
    }

    #[test]
    fn a_username_is_parsed_but_selects_nothing_later() {
        let (req, _) = parse(b"someone\r\n").expect("valid");
        assert_eq!(req.user, "someone");
    }

    #[test]
    fn the_verbose_flag_is_recognised() {
        let (req, _) = parse(b"/W someone\r\n").expect("valid");
        assert!(req.verbose);
        assert_eq!(req.user, "someone");
        let (req, _) = parse(b"/W\r\n").expect("valid");
        assert!(req.verbose);
        assert_eq!(req.user, "");
    }

    #[test]
    fn a_bare_lf_is_accepted() {
        assert!(parse(b"x\n").is_ok());
    }

    #[test]
    fn forwarding_queries_are_refused() {
        // RFC 1288 {Q2}. Answering these makes the server an open relay
        // for probing hosts that would not answer the client directly.
        for q in [
            &b"someone@example.org\r\n"[..],
            &b"@example.org\r\n"[..],
            &b"/W someone@a@b\r\n"[..],
            &b"a@b@c\r\n"[..],
        ] {
            assert_eq!(parse(q), Err(RequestError::ForwardingRefused), "{q:?}");
        }
    }

    #[test]
    fn control_bytes_are_refused() {
        assert_eq!(parse(b"a\0b\r\n"), Err(RequestError::ControlByte));
    }

    #[test]
    fn a_partial_line_asks_for_more() {
        assert_eq!(parse(b"abc"), Err(RequestError::Incomplete));
    }

    #[test]
    fn an_unterminated_flood_is_refused() {
        let raw = vec![b'a'; MAX_REQUEST_BYTES + 1];
        assert_eq!(parse(&raw), Err(RequestError::TooLong));
    }
}
