//! The request → response trait and its v1 implementations (ADR 0005):
//! static files, redirects, certificate zones. **Internal only** — not a
//! public extension API (windmark already exists for that); no stability
//! promise, and no other module may execute tree-resident content
//! (ADR 0005: CGI refused permanently).
//!
//! The trait exists for the shape ADR 0005 specifies (parsed request in,
//! `(status, meta, body)` out) but v1 has a small, fixed handler set, so
//! [`Router`] dispatches by owning typed fields rather than a `Vec<Box<dyn
//! Handler>>` — async fn in traits isn't object-safe without extra
//! machinery, and a fixed set doesn't need dynamic dispatch to get the
//! "new handler, not a restructure" property the ADR asks for.

pub mod admin;
pub mod cert_zone;
pub mod finger;
pub mod gopher;
pub mod mime;
pub mod nex;
pub mod redirect;
pub mod spartan;
pub mod static_file;
pub mod titan;

use crate::protocol::response::Header;

/// A handler's decided response, before wire emission. Body is either
/// nothing, an in-memory buffer (small generated content), or an open file
/// streamed via `tokio::io::copy` — no custom chunking abstraction needed,
/// since Gemini's body is just "bytes until connection close".
pub enum Body {
    /// No body (any 1x/3x/4x/5x/6x response).
    None,
    /// A fully-buffered body (small generated content: redirect targets
    /// carry no body, but future handlers might).
    Bytes(Vec<u8>),
    /// An open file to stream. The caller (`server.rs`) copies it directly
    /// into the TLS stream after writing the header.
    File(tokio::fs::File),
}

/// A complete handler outcome: header plus body.
pub struct HandlerResponse {
    /// The response header (status + META).
    pub header: Header,
    /// The response body.
    pub body: Body,
}

impl HandlerResponse {
    /// A response with no body — the common case for every non-2x status.
    pub fn header_only(header: Header) -> HandlerResponse {
        HandlerResponse {
            header,
            body: Body::None,
        }
    }
}

/// A validated client certificate as request handling sees it: the
/// SHA-256 fingerprint (identity for allowlists) and validity, computed
/// once at handshake time from the TLS layer's captured chain. This is
/// the *only* client-certificate information handlers receive — no raw
/// key material, no subject fields, matching ADR 0002's module-boundary
/// rule (identity/TLS modules alone touch key types) and ADR 0005's "no
/// certificate details are ever exported into environments or templates"
/// (nothing here is CGI-shaped, but the same restraint applies).
#[derive(Debug, Clone)]
pub struct ClientCertInfo {
    /// SHA-256 fingerprint of the leaf certificate's DER encoding, lowercase
    /// hex — the identity Molly Brown's authorized_keys model and gemini
    /// client-cert culture both key on.
    pub fingerprint_sha256: String,
    /// Whether the certificate's validity window covers now, per the X.509
    /// notBefore/notAfter fields. `false` maps to status 62.
    pub currently_valid: bool,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;
    use crate::protocol::response::Status;

    #[test]
    fn header_only_has_no_body() {
        let r = HandlerResponse::header_only(
            crate::protocol::response::Header::new(Status::NotFound, Some("not found")).unwrap(),
        );
        assert!(matches!(r.body, Body::None));
    }
}
