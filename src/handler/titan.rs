//! Titan upload zones: the **pre-body decision** (ADR 0006; recon
//! titan.md §5.1–5.3).
//!
//! Every rule here is applied *before* a single payload byte is read.
//! That is the whole architecture: Titan hands the server a declared size
//! and an already-completed TLS handshake, so authorization and limits can
//! be settled from the request line alone, and a refusal costs the server
//! nothing but a status line (plus the bounded courtesy drain in
//! `server::drain_bounded`).
//!
//! A zone is a writable path prefix plus the policy that governs it. Like
//! [`super::cert_zone`], the **longest matching prefix wins**. Unlike it,
//! two things are deliberately stricter, because this gate protects
//! *writes* rather than reads:
//!
//! 1. **An empty fingerprint allowlist is not "any valid certificate"** —
//!    it is a configuration error, rejected at startup ([`Zone::new`]).
//!    For a read zone, "require *a* cert" is a defensible policy; for a
//!    write zone it would mean anyone who can mint a self-signed
//!    certificate (i.e. anyone at all) may write to the capsule. Recon
//!    §5.1 makes cert-fingerprint gating mandatory, and this is where that
//!    is enforced structurally rather than left to operator care.
//! 2. **Writable paths are explicit and default to none.** No zone, no
//!    upload — a capsule that never configures Titan cannot be written to
//!    by anyone, whatever certificate they hold.
//!
//! The token, when a zone configures one, is only ever a *second* factor
//! (recon §5.2): it is checked after the certificate has already passed,
//! never instead of it. It rides in a URL, so it appears in client history
//! and any intermediary's logs; it is compared in constant time and never
//! echoed into a META or a log line.

use crate::handler::ClientCertInfo;
use crate::protocol::response::{Header, Status, stock};
use crate::protocol::titan::TitanRequest;

/// The default per-upload size cap when a zone names none: 10 MiB, the
/// GmCapsule default (recon §4.1) — the one number the ecosystem has
/// already converged on.
pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 10 * 1024 * 1024;

/// The payload types a zone accepts when it names none. Gemtext is the
/// point of the exercise; `text/plain` is what Lagrange's Text tab sends
/// (recon §3.1), so refusing it by default would break the single most
/// common client flow.
pub const DEFAULT_MIME_ALLOWLIST: &[&str] = &["text/gemini", "text/plain"];

/// One writable Titan zone.
#[derive(Debug, Clone)]
pub struct Zone {
    /// Path prefix this zone makes writable, always leading- and
    /// trailing-slashed (`/uploads/`). The trailing slash is what stops
    /// `/up` from making `/uploads` writable.
    pub path_prefix: String,
    /// SHA-256 fingerprints (lowercase hex) permitted to write here.
    /// Guaranteed non-empty by [`Zone::new`] — see the module docs.
    pub allowed_fingerprints: Vec<String>,
    /// Largest single upload accepted, in bytes.
    pub max_upload_bytes: u64,
    /// Accepted payload MIME types, compared without parameters and
    /// case-insensitively.
    pub allowed_mime: Vec<String>,
    /// Optional shared secret required *in addition to* an authorized
    /// certificate.
    pub token: Option<String>,
    /// Whether `size=0` (Titan's delete operation, recon §1.4) is honored
    /// here. Off unless the operator opts in: a client that can write
    /// should not automatically be able to destroy.
    pub allow_delete: bool,
}

/// Why a zone definition was refused at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneError {
    /// `path_prefix` does not start with `/`.
    PathNotAbsolute(String),
    /// No fingerprints were listed. See the module docs for why this is an
    /// error here but not for a read zone.
    NoFingerprints(String),
    /// `max_upload_bytes` was zero — a zone that accepts nothing is
    /// certainly a mistake, and silently accepting it would leave the
    /// operator wondering why every upload is refused.
    ZeroMaxUpload(String),
    /// An empty string appeared in the MIME allowlist.
    EmptyMime(String),
    /// A `token` was present but empty.
    EmptyToken(String),
}

impl std::fmt::Display for ZoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoneError::PathNotAbsolute(p) => write!(
                f,
                "titan_zone path_prefix {p:?} must start with \"/\" (it is a request path, \
                 not a filesystem path)"
            ),
            ZoneError::NoFingerprints(p) => write!(
                f,
                "titan_zone {p:?} lists no fingerprints. Unlike a read-gating cert_zone, \
                 a writable zone may not be left open: an empty allowlist would let anyone \
                 who can generate a self-signed certificate write to this capsule. List the \
                 SHA-256 fingerprint of every identity permitted to upload here"
            ),
            ZoneError::ZeroMaxUpload(p) => {
                write!(
                    f,
                    "titan_zone {p:?} has max_upload_bytes = 0, which would refuse every upload"
                )
            }
            ZoneError::EmptyMime(p) => {
                write!(f, "titan_zone {p:?} has an empty string in its mime list")
            }
            ZoneError::EmptyToken(p) => write!(
                f,
                "titan_zone {p:?} has an empty token. Remove the key to require no token, \
                 or set a real value"
            ),
        }
    }
}

impl std::error::Error for ZoneError {}

impl Zone {
    /// Build and validate a zone. Normalizes `path_prefix` to end with a
    /// slash (a zone is a subtree, and `/up` must not gate `/uploads`);
    /// everything else is checked, never silently corrected.
    pub fn new(
        path_prefix: &str,
        allowed_fingerprints: Vec<String>,
        max_upload_bytes: Option<u64>,
        allowed_mime: Option<Vec<String>>,
        token: Option<String>,
        allow_delete: bool,
    ) -> Result<Zone, ZoneError> {
        if !path_prefix.starts_with('/') {
            return Err(ZoneError::PathNotAbsolute(path_prefix.to_string()));
        }
        if allowed_fingerprints.is_empty() {
            return Err(ZoneError::NoFingerprints(path_prefix.to_string()));
        }
        let max_upload_bytes = max_upload_bytes.unwrap_or(DEFAULT_MAX_UPLOAD_BYTES);
        if max_upload_bytes == 0 {
            return Err(ZoneError::ZeroMaxUpload(path_prefix.to_string()));
        }
        let allowed_mime = match allowed_mime {
            Some(list) => {
                if list.iter().any(|m| m.trim().is_empty()) {
                    return Err(ZoneError::EmptyMime(path_prefix.to_string()));
                }
                list
            }
            None => DEFAULT_MIME_ALLOWLIST
                .iter()
                .map(|m| (*m).to_string())
                .collect(),
        };
        if token.as_ref().is_some_and(|t| t.is_empty()) {
            return Err(ZoneError::EmptyToken(path_prefix.to_string()));
        }
        let path_prefix = if path_prefix.ends_with('/') {
            path_prefix.to_string()
        } else {
            format!("{path_prefix}/")
        };
        Ok(Zone {
            path_prefix,
            allowed_fingerprints,
            max_upload_bytes,
            allowed_mime,
            token,
            allow_delete,
        })
    }
}

/// What to do with an upload, decided entirely from the request line.
#[derive(Debug)]
pub enum Decision<'a> {
    /// Refuse now, before reading any payload. The header is ready to
    /// write; the log line is already redacted (never contains the token).
    Refuse {
        /// The status to answer with.
        header: Header,
        /// What to log — safe to emit as-is.
        log: &'static str,
    },
    /// The request is authorized and within limits. Read exactly
    /// `request.size` bytes and apply them to the zone.
    Accept {
        /// The zone that matched, carrying the policy for the write.
        zone: &'a Zone,
        /// Whether this is a deletion (`size == 0`) rather than a write.
        delete: bool,
    },
}

/// Decide an upload from its request line alone.
///
/// Order is deliberate and disclosure-minimising: a caller that is not
/// authorized learns nothing about the zone's limits, because the
/// certificate and token gates run before the size and MIME checks. A
/// path outside every zone is answered as *not found* rather than
/// "forbidden", so probing cannot map which prefixes are writable.
pub fn decide<'a>(
    zones: &'a [Zone],
    request: &TitanRequest,
    cert: Option<&ClientCertInfo>,
) -> Decision<'a> {
    let Some(zone) = zones
        .iter()
        .filter(|z| request.path.starts_with(z.path_prefix.as_str()))
        .max_by_key(|z| z.path_prefix.len())
    else {
        return refuse(stock::not_found(), "titan: no writable zone for that path");
    };

    // --- Identity first: nothing below this point is disclosed to an
    // unauthenticated or unauthorized caller. ---
    match cert {
        None => {
            return refuse(
                gate(
                    Status::ClientCertRequired,
                    "uploading here requires a client certificate",
                ),
                "titan: no client certificate (60)",
            );
        }
        Some(c) if !c.currently_valid => {
            return refuse(
                gate(
                    Status::CertNotValid,
                    "your certificate is expired or not yet valid",
                ),
                "titan: certificate not currently valid (62)",
            );
        }
        Some(c) => {
            let authorized = zone
                .allowed_fingerprints
                .iter()
                .any(|f| f.eq_ignore_ascii_case(&c.fingerprint_sha256));
            if !authorized {
                return refuse(
                    gate(
                        Status::CertNotAuthorized,
                        "your certificate is not authorized to upload here",
                    ),
                    "titan: certificate not on the zone allowlist (61)",
                );
            }
        }
    }

    // --- Second factor, if the zone configures one. Constant-time, and
    // the value never reaches a META or a log. ---
    if let Some(expected) = &zone.token {
        let presented = request.token.as_deref().unwrap_or("");
        if !constant_time_eq(expected.as_bytes(), presented.as_bytes()) {
            return refuse(
                gate(
                    Status::CertNotAuthorized,
                    "a valid upload token is required for this area",
                ),
                "titan: token missing or incorrect (61)",
            );
        }
    }

    // --- Operation and limits. ---
    if request.size == 0 {
        if !zone.allow_delete {
            return refuse(
                gate(
                    Status::PermanentFailure,
                    "this area does not accept deletions",
                ),
                "titan: deletion attempted where allow_delete is off (50)",
            );
        }
        // A deletion carries no payload, so MIME and the size cap are
        // meaningless for it.
        return Decision::Accept { zone, delete: true };
    }

    if !mime_allowed(&zone.allowed_mime, &request.mime) {
        return refuse(
            gate(
                Status::BadRequest,
                "that content type is not accepted for this area",
            ),
            "titan: payload MIME not on the zone allowlist (59)",
        );
    }

    if request.size > zone.max_upload_bytes {
        // The cap itself is disclosed: the client has already proved it is
        // authorized here, and "too big" is useless without knowing the
        // limit (recon §5.3 — reject the declaration, never read the body).
        let msg = format!(
            "upload is larger than this area accepts (limit {} bytes)",
            zone.max_upload_bytes
        );
        return refuse(
            Header::new(Status::BadRequest, Some(&msg)).unwrap_or_else(|_| {
                gate(
                    Status::BadRequest,
                    "upload is larger than this area accepts",
                )
            }),
            "titan: declared size over the zone cap (59)",
        );
    }

    Decision::Accept {
        zone,
        delete: false,
    }
}

fn refuse<'a>(header: Header, log: &'static str) -> Decision<'a> {
    Decision::Refuse { header, log }
}

/// A 6x/5x header with a human-readable META (recon guidance §5: always
/// include one with 6x), falling back to a bare status if the message
/// could ever fail to encode.
fn gate(status: Status, message: &str) -> Header {
    Header::new(status, Some(message)).unwrap_or_else(|_| stock::unavailable())
}

/// Compare a declared payload type against a zone's allowlist: parameters
/// (`; charset=utf-8`) are ignored and comparison is case-insensitive, per
/// ordinary MIME rules — a client that helpfully adds a charset must not
/// be refused for it.
fn mime_allowed(allowlist: &[String], declared: &str) -> bool {
    let base = declared.split(';').next().unwrap_or("").trim();
    allowlist.iter().any(|allowed| {
        allowed
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case(base)
    })
}

/// Constant-time byte equality for the token comparison.
///
/// Length is compared first and therefore leaks, which is the standard
/// trade-off for this construction: token *length* is not the secret, and
/// hashing both sides to hide it would add a dependency and a failure mode
/// for no real gain here.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Why applying an authorized upload to the content tree failed. These
/// are *not* client errors — authorization and limits were already
/// settled — so each maps to a server-side status rather than a 5x/6x
/// aimed at the caller's request.
#[derive(Debug)]
pub enum ApplyError {
    /// The path could not be confined to the content tree (traversal,
    /// NUL, non-UTF-8), or named the tree itself rather than a file.
    UnusablePath,
    /// A deletion named a file that is not there.
    NotFound,
    /// The filesystem refused the write.
    Io(std::io::Error),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::UnusablePath => f.write_str("upload path is not usable"),
            ApplyError::NotFound => f.write_str("nothing to delete at that path"),
            ApplyError::Io(e) => write!(f, "filesystem error: {e}"),
        }
    }
}

/// Apply an authorized upload to the content tree.
///
/// **Atomic by construction** (recon §5.6): the payload is written to a
/// temporary file *in the destination directory*, flushed to disk, and
/// renamed into place. A rename within one directory is atomic on POSIX,
/// so a reader — including the render watcher — never observes a
/// half-written page, and a crash mid-upload leaves the previous version
/// intact rather than a truncated one.
///
/// The write lands in the **source** tree, which is the whole point: usv
/// renders from one content tree (ADR 0004), so an upload becomes visible
/// on both surfaces by the ordinary render path rather than by a second
/// mechanism that could disagree with it. The existing watcher notices the
/// change and re-renders; no separate trigger is needed, and the
/// debounce it already applies is exactly right for a burst of uploads.
///
/// Traversal defence is [`super::static_file::sanitize_request_path`] —
/// deliberately the same function that confines reads — plus a
/// canonicalized confinement check on the destination directory, which
/// catches a symlink inside the tree pointing out of it.
pub async fn apply(
    docroot: &std::path::Path,
    request_path: &str,
    body: &[u8],
    delete: bool,
) -> Result<(), ApplyError> {
    use std::path::PathBuf;

    let relative =
        super::static_file::sanitize_request_path(request_path).ok_or(ApplyError::UnusablePath)?;
    if relative.as_os_str().is_empty() {
        // "write the docroot itself" is meaningless.
        return Err(ApplyError::UnusablePath);
    }
    let target = docroot.join(&relative);
    let parent = target
        .parent()
        .ok_or(ApplyError::UnusablePath)?
        .to_path_buf();

    if delete {
        // Confine before touching anything: canonicalize the *existing*
        // parent and prove it is inside the tree.
        confine(docroot, &parent).await?;
        return match tokio::fs::remove_file(&target).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ApplyError::NotFound),
            Err(e) => Err(ApplyError::Io(e)),
        };
    }

    tokio::fs::create_dir_all(&parent)
        .await
        .map_err(ApplyError::Io)?;
    confine(docroot, &parent).await?;

    // Temp name: unique per process and per call, and prefixed with a dot
    // so a half-written upload is never itself mistaken for content by the
    // render walk if it is ever observed.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp: PathBuf = parent.join(format!(".usv-upload-{}-{seq}.tmp", std::process::id()));

    let write_then_rename = async {
        let mut file = tokio::fs::File::create(&temp).await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, body).await?;
        // Durability before the rename: a rename that beats its own data to
        // disk can surface an empty file after a crash.
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temp, &target).await
    };
    match write_then_rename.await {
        Ok(()) => Ok(()),
        Err(e) => {
            // Never leave the temp file behind to litter the content tree.
            let _ = tokio::fs::remove_file(&temp).await;
            Err(ApplyError::Io(e))
        }
    }
}

/// Prove `dir` really is inside `docroot` once symlinks are resolved.
async fn confine(docroot: &std::path::Path, dir: &std::path::Path) -> Result<(), ApplyError> {
    let (Ok(root), Ok(resolved)) = (
        tokio::fs::canonicalize(docroot).await,
        tokio::fs::canonicalize(dir).await,
    ) else {
        return Err(ApplyError::UnusablePath);
    };
    if !resolved.starts_with(&root) {
        return Err(ApplyError::UnusablePath);
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    const FP: &str = "aabbccdd";
    const OTHER_FP: &str = "11223344";

    fn zone(prefix: &str) -> Zone {
        Zone::new(prefix, vec![FP.to_string()], None, None, None, false).unwrap()
    }

    fn request(path: &str, size: u64) -> TitanRequest {
        TitanRequest {
            host: "example.org".into(),
            port: None,
            path: path.into(),
            query: None,
            size,
            mime: "text/gemini".into(),
            token: None,
        }
    }

    fn cert(fingerprint: &str, valid: bool) -> ClientCertInfo {
        ClientCertInfo {
            fingerprint_sha256: fingerprint.to_string(),
            currently_valid: valid,
        }
    }

    fn status_of(d: &Decision) -> Option<Status> {
        match d {
            Decision::Refuse { header, .. } => Some(header.status()),
            Decision::Accept { .. } => None,
        }
    }

    #[test]
    fn an_authorized_upload_within_limits_is_accepted() {
        let zones = [zone("/uploads/")];
        let d = decide(
            &zones,
            &request("/uploads/a.gmi", 10),
            Some(&cert(FP, true)),
        );
        assert!(matches!(d, Decision::Accept { delete: false, .. }));
    }

    #[test]
    fn a_writable_zone_may_not_be_left_open_to_any_certificate() {
        // The load-bearing asymmetry with cert_zone: an empty allowlist is
        // a startup error here, not "any valid cert".
        let err = Zone::new("/uploads/", vec![], None, None, None, false).unwrap_err();
        assert!(matches!(err, ZoneError::NoFingerprints(_)));
        assert!(err.to_string().contains("self-signed"), "{err}");
    }

    #[test]
    fn a_path_outside_every_zone_is_not_found_not_forbidden() {
        // Probing must not be able to map which prefixes are writable.
        let zones = [zone("/uploads/")];
        let d = decide(&zones, &request("/secret/a.gmi", 10), Some(&cert(FP, true)));
        assert_eq!(status_of(&d), Some(Status::NotFound));
    }

    #[test]
    fn no_zones_configured_means_nothing_is_writable() {
        let d = decide(&[], &request("/uploads/a.gmi", 10), Some(&cert(FP, true)));
        assert_eq!(status_of(&d), Some(Status::NotFound));
    }

    #[test]
    fn a_trailing_slash_is_normalised_so_a_prefix_cannot_overreach() {
        // "/up" must not make "/uploads" writable.
        let zones = [zone("/up")];
        assert_eq!(zones[0].path_prefix, "/up/");
        let d = decide(
            &zones,
            &request("/uploads/a.gmi", 10),
            Some(&cert(FP, true)),
        );
        assert_eq!(status_of(&d), Some(Status::NotFound));
    }

    #[test]
    fn the_longest_matching_prefix_wins() {
        let general = Zone::new("/w/", vec![FP.into()], Some(100), None, None, false).unwrap();
        let specific =
            Zone::new("/w/big/", vec![FP.into()], Some(9_000), None, None, false).unwrap();
        let zones = [general, specific];
        // 5000 bytes is over the general cap but under the specific one.
        let d = decide(
            &zones,
            &request("/w/big/x.gmi", 5_000),
            Some(&cert(FP, true)),
        );
        assert!(matches!(d, Decision::Accept { .. }), "{d:?}");
    }

    #[test]
    fn missing_expired_and_unlisted_certificates_map_to_60_62_61() {
        let zones = [zone("/uploads/")];
        let req = request("/uploads/a.gmi", 10);
        assert_eq!(
            status_of(&decide(&zones, &req, None)),
            Some(Status::ClientCertRequired)
        );
        assert_eq!(
            status_of(&decide(&zones, &req, Some(&cert(FP, false)))),
            Some(Status::CertNotValid)
        );
        assert_eq!(
            status_of(&decide(&zones, &req, Some(&cert(OTHER_FP, true)))),
            Some(Status::CertNotAuthorized)
        );
    }

    #[test]
    fn fingerprint_comparison_is_case_insensitive() {
        let zones = [Zone::new("/u/", vec!["AABBCCDD".into()], None, None, None, false).unwrap()];
        let d = decide(
            &zones,
            &request("/u/a.gmi", 5),
            Some(&cert("aabbccdd", true)),
        );
        assert!(matches!(d, Decision::Accept { .. }));
    }

    #[test]
    fn identity_is_checked_before_limits_are_disclosed() {
        // An oversize upload from an unauthorized cert must report the
        // certificate problem, never the size limit.
        let zones = [Zone::new("/u/", vec![FP.into()], Some(10), None, None, false).unwrap()];
        let d = decide(
            &zones,
            &request("/u/a.gmi", 1_000_000),
            Some(&cert(OTHER_FP, true)),
        );
        assert_eq!(status_of(&d), Some(Status::CertNotAuthorized));
        if let Decision::Refuse { header, .. } = d {
            let wire = String::from_utf8_lossy(&header.to_wire()).into_owned();
            assert!(!wire.contains("10"), "must not disclose the cap: {wire}");
        }
    }

    #[test]
    fn a_token_is_a_second_factor_never_a_replacement() {
        let zones = [Zone::new(
            "/u/",
            vec![FP.into()],
            None,
            None,
            Some("hunter2".into()),
            false,
        )
        .unwrap()];

        // Right token, wrong certificate: still refused on the certificate.
        let mut req = request("/u/a.gmi", 5);
        req.token = Some("hunter2".into());
        assert_eq!(
            status_of(&decide(&zones, &req, Some(&cert(OTHER_FP, true)))),
            Some(Status::CertNotAuthorized)
        );

        // Right certificate, wrong token: refused.
        let mut bad = request("/u/a.gmi", 5);
        bad.token = Some("wrong".into());
        assert_eq!(
            status_of(&decide(&zones, &bad, Some(&cert(FP, true)))),
            Some(Status::CertNotAuthorized)
        );

        // Right certificate, missing token: refused.
        assert_eq!(
            status_of(&decide(
                &zones,
                &request("/u/a.gmi", 5),
                Some(&cert(FP, true))
            )),
            Some(Status::CertNotAuthorized)
        );

        // Both correct: accepted.
        assert!(matches!(
            decide(&zones, &req, Some(&cert(FP, true))),
            Decision::Accept { .. }
        ));
    }

    #[test]
    fn a_rejected_token_is_never_echoed_back() {
        let zones = [Zone::new(
            "/u/",
            vec![FP.into()],
            None,
            None,
            Some("hunter2".into()),
            false,
        )
        .unwrap()];
        let mut req = request("/u/a.gmi", 5);
        req.token = Some("guessed-secret".into());
        let d = decide(&zones, &req, Some(&cert(FP, true)));
        if let Decision::Refuse { header, log } = d {
            let wire = String::from_utf8_lossy(&header.to_wire()).into_owned();
            assert!(!wire.contains("guessed-secret"), "{wire}");
            assert!(!wire.contains("hunter2"), "never leak the expected value");
            assert!(!log.contains("guessed-secret"));
        } else {
            panic!("expected refusal");
        }
    }

    #[test]
    fn an_oversize_declaration_is_refused_before_any_body_is_read() {
        let zones = [Zone::new("/u/", vec![FP.into()], Some(1_024), None, None, false).unwrap()];
        let d = decide(&zones, &request("/u/a.gmi", 1_025), Some(&cert(FP, true)));
        assert_eq!(status_of(&d), Some(Status::BadRequest));
        // Authorized callers DO get told the limit — "too big" alone is
        // unactionable.
        if let Decision::Refuse { header, .. } = d {
            let wire = String::from_utf8_lossy(&header.to_wire()).into_owned();
            assert!(wire.contains("1024"), "{wire}");
        }
    }

    #[test]
    fn exactly_the_cap_is_accepted() {
        let zones = [Zone::new("/u/", vec![FP.into()], Some(1_024), None, None, false).unwrap()];
        let d = decide(&zones, &request("/u/a.gmi", 1_024), Some(&cert(FP, true)));
        assert!(matches!(d, Decision::Accept { .. }));
    }

    #[test]
    fn mime_outside_the_allowlist_is_refused() {
        let zones = [zone("/u/")];
        let mut req = request("/u/a.gmi", 10);
        req.mime = "application/x-executable".into();
        assert_eq!(
            status_of(&decide(&zones, &req, Some(&cert(FP, true)))),
            Some(Status::BadRequest)
        );
    }

    #[test]
    fn mime_matching_ignores_parameters_and_case() {
        // Lagrange's Text tab sends text/plain; a client adding a charset
        // must not be refused for being helpful.
        let zones = [zone("/u/")];
        for declared in [
            "text/plain",
            "text/plain; charset=utf-8",
            "TEXT/PLAIN",
            "text/gemini;lang=en",
        ] {
            let mut req = request("/u/a.gmi", 10);
            req.mime = declared.into();
            assert!(
                matches!(
                    decide(&zones, &req, Some(&cert(FP, true))),
                    Decision::Accept { .. }
                ),
                "{declared} should be accepted by the default allowlist"
            );
        }
    }

    #[test]
    fn deletion_requires_an_explicit_opt_in() {
        let closed = [zone("/u/")];
        let d = decide(&closed, &request("/u/a.gmi", 0), Some(&cert(FP, true)));
        assert_eq!(status_of(&d), Some(Status::PermanentFailure));

        let open = [Zone::new("/u/", vec![FP.into()], None, None, None, true).unwrap()];
        assert!(matches!(
            decide(&open, &request("/u/a.gmi", 0), Some(&cert(FP, true))),
            Decision::Accept { delete: true, .. }
        ));
    }

    #[test]
    fn deletion_still_requires_an_authorized_certificate() {
        let open = [Zone::new("/u/", vec![FP.into()], None, None, None, true).unwrap()];
        assert_eq!(
            status_of(&decide(&open, &request("/u/a.gmi", 0), None)),
            Some(Status::ClientCertRequired)
        );
        assert_eq!(
            status_of(&decide(
                &open,
                &request("/u/a.gmi", 0),
                Some(&cert(OTHER_FP, true))
            )),
            Some(Status::CertNotAuthorized)
        );
    }

    #[test]
    fn zone_validation_rejects_the_obvious_mistakes() {
        assert!(matches!(
            Zone::new("uploads/", vec![FP.into()], None, None, None, false).unwrap_err(),
            ZoneError::PathNotAbsolute(_)
        ));
        assert!(matches!(
            Zone::new("/u/", vec![FP.into()], Some(0), None, None, false).unwrap_err(),
            ZoneError::ZeroMaxUpload(_)
        ));
        assert!(matches!(
            Zone::new(
                "/u/",
                vec![FP.into()],
                None,
                Some(vec!["".into()]),
                None,
                false
            )
            .unwrap_err(),
            ZoneError::EmptyMime(_)
        ));
        assert!(matches!(
            Zone::new("/u/", vec![FP.into()], None, None, Some("".into()), false).unwrap_err(),
            ZoneError::EmptyToken(_)
        ));
    }

    #[test]
    fn constant_time_eq_is_correct_whatever_else_it_is() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod apply_tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_root(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("usv-titan-apply-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn an_upload_lands_in_the_content_tree() {
        let root = tmp_root("write");
        apply(&root, "/notes/hello.gmi", b"# hi\n", false)
            .await
            .unwrap();
        let written = std::fs::read_to_string(root.join("notes/hello.gmi")).unwrap();
        assert_eq!(written, "# hi\n");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn an_upload_replaces_an_existing_page_atomically() {
        let root = tmp_root("replace");
        std::fs::write(root.join("p.gmi"), b"old").unwrap();
        apply(&root, "/p.gmi", b"new", false).await.unwrap();
        assert_eq!(std::fs::read_to_string(root.join("p.gmi")).unwrap(), "new");
        // No temp files left behind for the render walk to trip over.
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("usv-upload"))
            .collect();
        assert!(leftovers.is_empty(), "temp files must not survive");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn traversal_can_never_escape_the_content_tree() {
        let root = tmp_root("traversal");
        let outside = root.parent().unwrap().join("usv-titan-escape-marker");
        let _ = std::fs::remove_file(&outside);

        // Outright rejections: a `..` segment (encoded or not) is refused
        // lexically, and a poison NUL never reaches the filesystem.
        for path in [
            "/../usv-titan-escape-marker",
            "/a/../../usv-titan-escape-marker",
            "/%2e%2e/usv-titan-escape-marker",
            "/a%00.gmi",
        ] {
            let result = apply(&root, path, b"owned", false).await;
            assert!(result.is_err(), "{path} must be refused");
        }

        // Double-encoded is a different case, and the honest expectation
        // is different too: `%252e%252e` decodes ONCE to the literal
        // string "%2e%2e", which is an ordinary (if odd) directory name.
        // Decoding it twice would be the bug. So this write succeeds and
        // lands *inside* the tree — the security property is confinement,
        // not refusal.
        apply(&root, "/%252e%252e/inner.gmi", b"harmless", false)
            .await
            .expect("a literal %2e%2e directory name is not an escape");
        assert!(
            root.join("%2e%2e/inner.gmi").exists(),
            "it must land inside the tree, under the literal name"
        );

        assert!(
            !outside.exists(),
            "nothing may ever be written outside the tree"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn writing_the_docroot_itself_is_refused() {
        let root = tmp_root("root-write");
        assert!(apply(&root, "/", b"x", false).await.is_err());
        assert!(apply(&root, "", b"x", false).await.is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn delete_removes_the_page_and_reports_a_missing_one() {
        let root = tmp_root("delete");
        std::fs::write(root.join("gone.gmi"), b"bye").unwrap();
        apply(&root, "/gone.gmi", b"", true).await.unwrap();
        assert!(!root.join("gone.gmi").exists());

        let err = apply(&root, "/never-existed.gmi", b"", true)
            .await
            .unwrap_err();
        assert!(matches!(err, ApplyError::NotFound));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_symlinked_directory_pointing_out_of_the_tree_is_refused() {
        #[cfg(unix)]
        {
            let root = tmp_root("symlink");
            let outside = root.parent().unwrap().join("usv-titan-outside-dir");
            std::fs::create_dir_all(&outside).unwrap();
            std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();

            let result = apply(&root, "/escape/evil.gmi", b"owned", false).await;
            assert!(result.is_err(), "a symlink out of the tree must be refused");
            assert!(!outside.join("evil.gmi").exists());

            let _ = std::fs::remove_dir_all(&outside);
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}
