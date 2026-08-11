//! Request-line **framing**: layer 1 of 3 (see [`crate::protocol`]).
//!
//! Spec basis (v0.24.1, quoted in `docs/internal/recon/protocol.md`): a request is
//! `absolute-URI CRLF`, and "the URI MUST NOT exceed 1024 bytes, and a server
//! MUST reject requests where the URI exceeds this limit". The limit is on the
//! URI itself, excluding the terminator, so the largest conforming request is
//! [`MAX_REQUEST_BYTES`] (1026) bytes on the wire. The ABNF terminator is CRLF
//! (STD 68); a bare LF is not a conforming request and is rejected — the
//! strict reading, which also closes request-smuggling-adjacent ambiguity
//! (recon: "Contested — trailing CRLF strictness").
//!
//! This layer is byte-level only. It deliberately passes through NUL, other
//! control bytes, and non-ASCII bytes inside the URI slice: rejecting those is
//! layer 2's job (URI validation, status 59), and keeping the layers honest is
//! what lets each be fuzzed for its own contract.

/// Maximum size of the request URI in bytes, excluding the CRLF terminator.
pub const MAX_URI_BYTES: usize = 1024;

/// Maximum bytes a conforming request occupies on the wire: URI plus CRLF.
///
/// The connection reader (C1) reads at most this many bytes before giving up;
/// [`frame_request_line`] therefore judges a complete buffer, never a stream.
pub const MAX_REQUEST_BYTES: usize = MAX_URI_BYTES + 2;

/// Why the framing layer rejected a buffer. Every variant is answered on the
/// wire with status `59` (bad request) and connection close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramingError {
    /// No CRLF terminator within the first [`MAX_REQUEST_BYTES`] bytes.
    ///
    /// For a complete read this means the peer sent an oversize or
    /// unterminated request; the reader has already stopped at the budget, so
    /// "wait for more" is never the right interpretation here.
    MissingCrlf,
    /// A LF appeared that was not preceded by CR (bare-LF termination or an
    /// embedded newline). The spec's ABNF requires CRLF; we do not accept the
    /// lenient reading.
    BareLf,
    /// A CR appeared that was not immediately followed by LF.
    StrayCr,
    /// The terminator was found but the URI before it was empty.
    EmptyUri,
    /// The terminator was found but the URI before it exceeds
    /// [`MAX_URI_BYTES`].
    UriTooLong,
}

impl core::fmt::Display for FramingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            FramingError::MissingCrlf => "request line not terminated by CRLF within budget",
            FramingError::BareLf => "bare LF in request line (CRLF required)",
            FramingError::StrayCr => "CR not followed by LF in request line",
            FramingError::EmptyUri => "empty request line",
            FramingError::UriTooLong => "URI exceeds 1024 bytes",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for FramingError {}

/// Extract the URI bytes from a complete request buffer.
///
/// `buf` is everything the connection reader collected, at most
/// [`MAX_REQUEST_BYTES`] bytes of which are examined (bytes beyond the budget
/// cannot change the verdict: a conforming terminator can only live inside
/// it). On success the returned slice is the URI exactly as sent — framing
/// guarantees only: non-empty, at most [`MAX_URI_BYTES`] long, and free of CR
/// and LF. All further validation is layer 2.
///
/// Trailing bytes after the CRLF are ignored here: the client has nothing
/// more to say in this protocol, and the connection task closes without
/// reading them (recon: "Ambiguity policy").
pub fn frame_request_line(buf: &[u8]) -> Result<&[u8], FramingError> {
    let window = &buf[..buf.len().min(MAX_REQUEST_BYTES)];
    let mut i = 0;
    while i < window.len() {
        match window[i] {
            b'\r' => {
                // A CR is only legal as the first half of the terminator, and
                // the LF that completes it must be visible. `buf` (not the
                // clipped window) decides whether a following byte exists at
                // all: a CR at the window edge with more bytes beyond it is a
                // CR followed by something that is not LF, or an over-budget
                // terminator — StrayCr either way, never a wait-for-more.
                return match buf.get(i + 1) {
                    Some(&b'\n') => {
                        let uri = &window[..i];
                        if uri.is_empty() {
                            Err(FramingError::EmptyUri)
                        } else if uri.len() > MAX_URI_BYTES {
                            Err(FramingError::UriTooLong)
                        } else {
                            Ok(uri)
                        }
                    }
                    _ => Err(FramingError::StrayCr),
                };
            }
            b'\n' => return Err(FramingError::BareLf),
            _ => i += 1,
        }
    }
    Err(FramingError::MissingCrlf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_minimal_request() {
        assert_eq!(
            frame_request_line(b"gemini://a/\r\n"),
            Ok(&b"gemini://a/"[..])
        );
    }

    #[test]
    fn uri_of_exactly_1024_bytes_is_accepted() {
        let mut buf = vec![b'a'; MAX_URI_BYTES];
        buf.extend_from_slice(b"\r\n");
        let framed = frame_request_line(&buf).expect("1024-byte URI is conforming");
        assert_eq!(framed.len(), MAX_URI_BYTES);
    }

    #[test]
    fn uri_of_1025_bytes_is_rejected() {
        let mut buf = vec![b'a'; MAX_URI_BYTES + 1];
        buf.extend_from_slice(b"\r\n");
        assert_eq!(frame_request_line(&buf), Err(FramingError::UriTooLong));
    }

    #[test]
    fn unterminated_oversize_garbage_is_missing_crlf() {
        let buf = vec![b'a'; 4096];
        assert_eq!(frame_request_line(&buf), Err(FramingError::MissingCrlf));
    }

    #[test]
    fn bare_lf_is_rejected() {
        assert_eq!(
            frame_request_line(b"gemini://a/\n"),
            Err(FramingError::BareLf)
        );
    }

    #[test]
    fn embedded_bare_lf_is_rejected_before_terminator_is_found() {
        assert_eq!(
            frame_request_line(b"gem\nini://a/\r\n"),
            Err(FramingError::BareLf)
        );
    }

    #[test]
    fn cr_without_lf_is_rejected() {
        assert_eq!(
            frame_request_line(b"gemini://a\rb\r\n"),
            Err(FramingError::StrayCr)
        );
    }

    #[test]
    fn lone_trailing_cr_is_stray_not_wait_for_more() {
        // Complete-buffer semantics: the reader stopped, so a dangling CR is a
        // malformed request, not a partial one.
        assert_eq!(
            frame_request_line(b"gemini://a/\r"),
            Err(FramingError::StrayCr)
        );
    }

    #[test]
    fn empty_line_is_rejected() {
        assert_eq!(frame_request_line(b"\r\n"), Err(FramingError::EmptyUri));
    }

    #[test]
    fn empty_buffer_is_missing_crlf() {
        assert_eq!(frame_request_line(b""), Err(FramingError::MissingCrlf));
    }

    #[test]
    fn bytes_after_the_terminator_are_ignored() {
        assert_eq!(
            frame_request_line(b"gemini://a/\r\ntrailing junk"),
            Ok(&b"gemini://a/"[..])
        );
    }

    #[test]
    fn nul_and_high_bytes_pass_framing() {
        // Layer separation: framing hands these through; URI validation
        // (layer 2, C1) is where they die with status 59.
        assert_eq!(frame_request_line(b"\x00\xff\r\n"), Ok(&b"\x00\xff"[..]));
    }

    #[test]
    fn cr_at_window_edge_with_overflow_beyond_is_rejected() {
        // CR at index 1025 (the last in-window byte), LF beyond the window:
        // the URI would be 1025 bytes, so no reading of it is conforming.
        let mut buf = vec![b'a'; MAX_URI_BYTES + 1];
        buf.extend_from_slice(b"\r\n"); // CR at 1025, LF at 1026
        assert_eq!(frame_request_line(&buf), Err(FramingError::UriTooLong));
    }
}
