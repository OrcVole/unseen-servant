//! Certificate zones: path-scoped client-certificate gating (Molly Brown's
//! `authorized_keys`-style model, docs/recon/prior-art.md §3; ADR 0005).
//!
//! A zone is a path prefix plus an optional SHA-256 fingerprint allowlist.
//! The **longest matching prefix wins** (most-specific zone applies), per
//! the recon guidance's "host + port + path-prefix" scoping. No CGI means
//! no certificate details are ever exported anywhere beyond this check
//! (ADR 0005) — the caller sees only a pass/block decision.

use crate::handler::{ClientCertInfo, HandlerResponse};
use crate::protocol::response::{Header, Status};

/// One certificate zone.
#[derive(Debug, Clone)]
pub struct Zone {
    /// Path prefix this zone gates (e.g. `/private/`).
    pub path_prefix: String,
    /// SHA-256 fingerprints (lowercase hex) authorized for this zone. An
    /// empty list means "any currently-valid client certificate" — the
    /// zone requires *a* cert but doesn't restrict *which* one, matching
    /// Molly Brown's "require cert" mode without an allowlist.
    pub allowed_fingerprints: Vec<String>,
}

/// Check `path` against `zones`. Returns `Some(response)` when the request
/// must be blocked (60/61/62); `None` means either no zone applies or the
/// presented certificate is authorized — the caller proceeds to normal
/// dispatch (redirect/static).
pub fn check(zones: &[Zone], path: &str, cert: Option<&ClientCertInfo>) -> Option<HandlerResponse> {
    let zone = zones
        .iter()
        .filter(|z| path.starts_with(z.path_prefix.as_str()))
        .max_by_key(|z| z.path_prefix.len())?;

    match cert {
        None => Some(blocked(
            Status::ClientCertRequired,
            "this area requires a client certificate",
        )),
        Some(c) if !c.currently_valid => Some(blocked(
            Status::CertNotValid,
            "your certificate is expired or not yet valid",
        )),
        Some(c) => {
            let authorized = zone.allowed_fingerprints.is_empty()
                || zone
                    .allowed_fingerprints
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&c.fingerprint_sha256));
            if authorized {
                None
            } else {
                Some(blocked(
                    Status::CertNotAuthorized,
                    "your certificate is not authorized for this area",
                ))
            }
        }
    }
}

fn blocked(status: Status, message: &str) -> HandlerResponse {
    // Status classes 60/61/62 have optional META; a human-readable message
    // is always worth including (recon guidance §5: "Always include a
    // human-readable META with 6x").
    let header = Header::new(status, Some(message))
        .unwrap_or_else(|_| crate::protocol::response::stock::unavailable());
    HandlerResponse::header_only(header)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn valid_cert(fp: &str) -> ClientCertInfo {
        ClientCertInfo {
            fingerprint_sha256: fp.to_string(),
            currently_valid: true,
        }
    }

    #[test]
    fn no_zone_matches_falls_through() {
        let zones = vec![Zone {
            path_prefix: "/private/".into(),
            allowed_fingerprints: vec![],
        }];
        assert!(check(&zones, "/public/page.gmi", None).is_none());
    }

    #[test]
    fn missing_cert_is_60() {
        let zones = vec![Zone {
            path_prefix: "/private/".into(),
            allowed_fingerprints: vec![],
        }];
        let resp = check(&zones, "/private/x", None).expect("blocked");
        assert_eq!(resp.header.status(), Status::ClientCertRequired);
    }

    #[test]
    fn expired_cert_is_62() {
        let zones = vec![Zone {
            path_prefix: "/private/".into(),
            allowed_fingerprints: vec![],
        }];
        let mut cert = valid_cert("aa");
        cert.currently_valid = false;
        let resp = check(&zones, "/private/x", Some(&cert)).expect("blocked");
        assert_eq!(resp.header.status(), Status::CertNotValid);
    }

    #[test]
    fn valid_cert_not_on_allowlist_is_61() {
        let zones = vec![Zone {
            path_prefix: "/private/".into(),
            allowed_fingerprints: vec!["aabbcc".into()],
        }];
        let resp = check(&zones, "/private/x", Some(&valid_cert("ddeeff"))).expect("blocked");
        assert_eq!(resp.header.status(), Status::CertNotAuthorized);
    }

    #[test]
    fn valid_cert_on_allowlist_passes() {
        let zones = vec![Zone {
            path_prefix: "/private/".into(),
            allowed_fingerprints: vec!["aabbcc".into()],
        }];
        assert!(check(&zones, "/private/x", Some(&valid_cert("AABBCC"))).is_none());
    }

    #[test]
    fn empty_allowlist_accepts_any_valid_cert() {
        let zones = vec![Zone {
            path_prefix: "/private/".into(),
            allowed_fingerprints: vec![],
        }];
        assert!(check(&zones, "/private/x", Some(&valid_cert("anything"))).is_none());
    }

    #[test]
    fn most_specific_zone_wins() {
        let zones = vec![
            Zone {
                path_prefix: "/a/".into(),
                allowed_fingerprints: vec![],
            },
            Zone {
                path_prefix: "/a/b/".into(),
                allowed_fingerprints: vec!["specific".into()],
            },
        ];
        // /a/b/x matches both; the longer prefix (/a/b/) must win, so an
        // unauthorized fingerprint gets 61, not a pass-through from /a/.
        let resp = check(&zones, "/a/b/x", Some(&valid_cert("other"))).expect("blocked");
        assert_eq!(resp.header.status(), Status::CertNotAuthorized);
    }
}
