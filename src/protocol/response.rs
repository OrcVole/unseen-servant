//! Response **emission**: the exact `XX SP META CRLF` rules.
//!
//! Spec basis (v0.24.1, docs/recon/protocol.md "Responses"): META is
//! mandatory for 1x (prompt), 2x (MIME type), and 3x (redirect target) —
//! the old empty-META default died in 0.24.0. META is optional for 4x/5x/6x,
//! and when omitted the SP separator is omitted too (`"50" CRLF`). Headers
//! are UTF-8, must not begin with a BOM, and META is capped at 1024 bytes.
//! Servers MUST NOT send undefined status codes, so the [`Status`] enum is
//! the complete list — there is no escape hatch for inventing one.

/// Every status code defined by Gemini v0.24.1. The enum is exhaustive on
/// purpose: "Servers MUST NOT send status codes that are not defined."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    /// 10 — input expected; META is the prompt.
    Input = 10,
    /// 11 — sensitive input (echo off); pair with client certs, never
    /// passwords alone (contested-issue #17 guidance).
    SensitiveInput = 11,
    /// 20 — success; META is the MIME type; body follows.
    Success = 20,
    /// 30 — temporary redirect; META is the target URI-reference.
    RedirectTemporary = 30,
    /// 31 — permanent redirect.
    RedirectPermanent = 31,
    /// 40 — temporary failure.
    TemporaryFailure = 40,
    /// 41 — server unavailable.
    ServerUnavailable = 41,
    /// 42 — CGI error (defined by the spec; usv never runs CGI, ADR 0005).
    CgiError = 42,
    /// 43 — proxy error (defined by the spec; usv never proxies).
    ProxyError = 43,
    /// 44 — slow down; META is an optional human message, never a number
    /// (the wait-seconds semantics died with v0.16.1).
    SlowDown = 44,
    /// 50 — permanent failure.
    PermanentFailure = 50,
    /// 51 — not found.
    NotFound = 51,
    /// 52 — gone.
    Gone = 52,
    /// 53 — proxy request refused: the authority is not one this server
    /// serves (foreign scheme, host, or port).
    ProxyRequestRefused = 53,
    /// 59 — bad request: framing or URI validation failed.
    BadRequest = 59,
    /// 60 — client certificate required.
    ClientCertRequired = 60,
    /// 61 — certificate not authorized for this resource.
    CertNotAuthorized = 61,
    /// 62 — certificate not valid (expired, not yet valid, malformed).
    CertNotValid = 62,
}

impl Status {
    /// Whether META is mandatory (1x/2x/3x), or optional (4x/5x/6x).
    pub fn requires_meta(self) -> bool {
        (self as u8) < 40
    }
}

/// Why a response header could not be constructed. These are **programmer
/// errors** surfaced as `Result` rather than panics: a server must never be
/// taken down by a handler building a bad header, and `unwrap_used` is a
/// lint for a reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    /// META omitted for a status class where it is mandatory.
    MetaRequired,
    /// META exceeds 1024 bytes.
    MetaTooLong,
    /// META contains a control character, starts with a BOM, or contains
    /// CR/LF (which would let response content forge headers).
    MetaForbiddenChars,
}

impl core::fmt::Display for HeaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            HeaderError::MetaRequired => "META is mandatory for status classes 1x/2x/3x",
            HeaderError::MetaTooLong => "META exceeds 1024 bytes",
            HeaderError::MetaForbiddenChars => "META contains control characters or a leading BOM",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for HeaderError {}

/// Maximum META length in bytes, per the spec.
pub const MAX_META_BYTES: usize = 1024;

/// A validated response header, ready for the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    status: Status,
    meta: Option<String>,
}

impl Header {
    /// Build a header, enforcing the META rules for the status class.
    pub fn new(status: Status, meta: Option<&str>) -> Result<Header, HeaderError> {
        match &meta {
            None => {
                if status.requires_meta() {
                    return Err(HeaderError::MetaRequired);
                }
            }
            Some(m) => {
                if m.len() > MAX_META_BYTES {
                    return Err(HeaderError::MetaTooLong);
                }
                if m.starts_with('\u{feff}') {
                    return Err(HeaderError::MetaForbiddenChars);
                }
                // The response ABNF allows SP plus non-control characters;
                // CR/LF especially would let META forge a second header.
                if m.chars().any(|c| c.is_control()) {
                    return Err(HeaderError::MetaForbiddenChars);
                }
            }
        }
        Ok(Header {
            status,
            meta: meta.map(str::to_owned),
        })
    }

    /// The status this header carries.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Wire form: `XX CRLF` or `XX SP META CRLF` — exactly one SP, no
    /// trailing whitespace, CRLF terminator.
    pub fn to_wire(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + 1 + self.meta.as_ref().map_or(0, String::len) + 2);
        out.extend_from_slice(format!("{}", self.status as u8).as_bytes());
        if let Some(meta) = &self.meta {
            out.push(b' ');
            out.extend_from_slice(meta.as_bytes());
        }
        out.extend_from_slice(b"\r\n");
        out
    }
}

/// The stock rejection headers the connection path needs. Centralized so
/// every rejection site emits an identical, spec-checked header — and so the
/// infallibility of these particular constructions is proven right here by
/// tests rather than `unwrap`ed at nine call sites.
pub mod stock {
    use super::{Header, Status};

    fn built(status: Status, meta: &str) -> Header {
        // Infallible by construction: static META, short, no controls.
        // Header::new is total over these inputs; the tests pin it.
        Header::new(status, Some(meta)).unwrap_or(Header { status, meta: None })
    }

    /// 59 with the framing/validation reason as META.
    pub fn bad_request(reason: &dyn core::fmt::Display) -> Header {
        let msg = format!("bad request: {reason}");
        match Header::new(Status::BadRequest, Some(&msg)) {
            Ok(h) => h,
            // A Display impl smuggling controls cannot forge a header; fall
            // back to the reasonless form.
            Err(_) => built(Status::BadRequest, "bad request"),
        }
    }

    /// 53 — this server does not proxy and does not serve that authority.
    pub fn proxy_refused() -> Header {
        built(
            Status::ProxyRequestRefused,
            "this server does not proxy requests for other hosts or schemes",
        )
    }

    /// 51 — not found.
    pub fn not_found() -> Header {
        built(Status::NotFound, "not found")
    }

    /// 41 — the server is shutting down or overloaded.
    pub fn unavailable() -> Header {
        built(Status::ServerUnavailable, "server unavailable")
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    #[test]
    fn success_header_has_exactly_one_space_and_crlf() {
        let h = Header::new(Status::Success, Some("text/gemini; charset=utf-8")).expect("valid");
        assert_eq!(h.to_wire(), b"20 text/gemini; charset=utf-8\r\n");
    }

    #[test]
    fn optional_meta_omits_the_space_too() {
        let h = Header::new(Status::PermanentFailure, None).expect("valid");
        assert_eq!(h.to_wire(), b"50\r\n");
    }

    #[test]
    fn failure_with_message_keeps_the_space() {
        let h = Header::new(Status::NotFound, Some("not found")).expect("valid");
        assert_eq!(h.to_wire(), b"51 not found\r\n");
    }

    #[test]
    fn meta_is_mandatory_for_1x_2x_3x() {
        for status in [Status::Input, Status::Success, Status::RedirectTemporary] {
            assert_eq!(
                Header::new(status, None).unwrap_err(),
                HeaderError::MetaRequired
            );
        }
    }

    #[test]
    fn oversize_meta_is_rejected() {
        let long = "x".repeat(MAX_META_BYTES + 1);
        assert_eq!(
            Header::new(Status::Success, Some(&long)).unwrap_err(),
            HeaderError::MetaTooLong
        );
        let exact = "x".repeat(MAX_META_BYTES);
        assert!(Header::new(Status::Success, Some(&exact)).is_ok());
    }

    #[test]
    fn control_chars_and_bom_are_rejected() {
        for meta in ["a\r\nb", "a\nb", "tab\tok?", "\u{feff}text/gemini"] {
            assert_eq!(
                Header::new(Status::Success, Some(meta)).unwrap_err(),
                HeaderError::MetaForbiddenChars,
                "{meta:?}"
            );
        }
    }

    #[test]
    fn unicode_meta_is_allowed() {
        let h = Header::new(Status::NotFound, Some("página no encontrada")).expect("valid");
        assert!(h.to_wire().starts_with(b"51 p"));
    }

    #[test]
    fn stock_headers_are_infallible_and_conforming() {
        assert_eq!(stock::proxy_refused().status(), Status::ProxyRequestRefused);
        assert_eq!(stock::not_found().to_wire(), b"51 not found\r\n");
        let h = stock::bad_request(&"URI exceeds 1024 bytes");
        assert_eq!(h.to_wire(), b"59 bad request: URI exceeds 1024 bytes\r\n");
    }
}
