//! Static file serving: traversal-proof path resolution, MIME lookup,
//! streaming.
//!
//! **Every rejection here maps to 51 (not found), never anything more
//! specific.** This is deliberate: telling an attacker "that path was
//! blocked as hostile" versus "that file doesn't exist" is a free
//! information leak, and gemini-diagnostics' own URLDotEscape check
//! expects exactly this — "any 5X permanent failure," not a distinguished
//! code. Malformed percent-encoding already can't reach this module (layer
//! 2 validates it); what lands here is well-formed but possibly hostile.
//!
//! Path resolution, in order — each step is a fuzzed, independently tested
//! function so the whole pipeline's safety doesn't rest on getting one
//! giant function right:
//!
//! 1. **Percent-decode** the request path into raw bytes.
//! 2. **Reject NUL bytes** in the decoded form (`%00` truncation attacks;
//!    the raw wire form can't carry a literal NUL past layer 2, but a
//!    *decoded* NUL is exactly the classic poison-null-byte trick).
//! 3. **Require valid UTF-8** in the decoded form — conservative, but
//!    keeps every downstream `Path`/`str` operation honest; content
//!    authors get international filenames, attackers get rejected rather
//!    than an implementation-defined byte-string path.
//! 4. **Split into segments on `/`, reject any `..` segment outright.**
//!    Lexical rejection, not resolution: a literal `..` component never
//!    gets to "cancel out" against a real directory. Empty segments
//!    (`//`) and `.` segments are dropped (harmless, just redundant).
//! 5. **Join onto the docroot**, then **canonicalize both** and verify the
//!    result still starts with the canonical docroot. This is the defense
//!    step 4 can't provide on its own: a *symlink* inside the docroot
//!    that points outside it would pass lexical filtering but fail this
//!    prefix check, because canonicalize follows symlinks.

use std::path::{Path, PathBuf};

use crate::handler::{Body, HandlerResponse};
use crate::protocol::response::{Header, Status, stock};

use super::mime;

/// A file resolved and ready to serve, or the decision to answer 51.
enum Resolved {
    File(PathBuf),
    NotFound,
}

/// Serve `request_path` (the raw, percent-encoded wire path, e.g.
/// `"/notes/%2e%2e/x"`) from `docroot`. Never panics; every failure mode
/// resolves to a 51 response rather than propagating an error, per the
/// module's information-leak policy above.
pub async fn serve(docroot: &Path, request_path: &str) -> HandlerResponse {
    match resolve(docroot, request_path).await {
        Resolved::NotFound => HandlerResponse::header_only(stock::not_found()),
        Resolved::File(path) => match tokio::fs::File::open(&path).await {
            Ok(file) => {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                let meta = mime::lookup(filename);
                match Header::new(Status::Success, Some(meta)) {
                    Ok(header) => HandlerResponse {
                        header,
                        body: Body::File(file),
                    },
                    Err(_) => {
                        // Unreachable in practice (MIME strings are static,
                        // short, control-free); fail closed rather than
                        // panic if the invariant ever breaks.
                        HandlerResponse::header_only(stock::unavailable())
                    }
                }
            }
            // Resolve succeeded (canonicalize proved the path exists) but
            // open failed anyway: permissions, a TOCTOU removal, or it
            // turned out to be unreadable. Same non-informative response.
            Err(_) => HandlerResponse::header_only(stock::not_found()),
        },
    }
}

/// Steps 1–4 from the module docs: decode, reject NUL, require UTF-8,
/// lexically reject any `..` segment. Pure and filesystem-free by design —
/// this is what `fuzz/fuzz_targets/static_path_sanitize.rs` drives — so the
/// hostile-input logic can be exercised without creating files on disk for
/// every case. Returns the sanitized *relative* path (possibly empty,
/// meaning "the docroot itself") on success.
fn sanitize_request_path(request_path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(request_path)?;
    if decoded.contains(&0u8) {
        return None; // step 2: poison-null-byte rejection
    }
    let decoded = String::from_utf8(decoded).ok()?; // step 3: require UTF-8

    let mut safe = PathBuf::new();
    for segment in decoded.split('/') {
        match segment {
            "" | "." => continue, // step 4: collapse redundant separators
            ".." => return None,  // step 4: lexical rejection, never resolved away
            s => safe.push(s),
        }
    }
    Some(safe)
}

/// Step 5, plus assembly: join the sanitized relative path onto `docroot`
/// and canonicalize both to catch symlink escapes. Requires the filesystem,
/// so it lives behind `resolve` rather than in the pure sanitizer above.
async fn resolve(docroot: &Path, request_path: &str) -> Resolved {
    let Some(relative) = sanitize_request_path(request_path) else {
        return Resolved::NotFound;
    };

    let mut candidate = docroot.to_path_buf();
    if relative.as_os_str().is_empty() {
        // Empty path or "/" resolves to the docroot's index, not the
        // docroot itself (directory listing is out of scope for C2).
        candidate.push("index.gmi");
    } else {
        candidate.push(&relative);
    }

    // symlink-aware defense in depth. canonicalize requires the
    // target to exist, which conveniently also answers "not found" for
    // free — a nonexistent file and an escape attempt get the same 51.
    let (Ok(canonical_root), Ok(canonical_candidate)) = (
        tokio::fs::canonicalize(docroot).await,
        tokio::fs::canonicalize(&candidate).await,
    ) else {
        return Resolved::NotFound;
    };
    if !canonical_candidate.starts_with(&canonical_root) {
        return Resolved::NotFound;
    }
    // Directories aren't servable directly (no listing in C2); a request
    // for one 51s rather than silently serving nothing useful.
    match tokio::fs::metadata(&canonical_candidate).await {
        Ok(meta) if meta.is_file() => Resolved::File(canonical_candidate),
        _ => Resolved::NotFound,
    }
}

/// Decode `%XX` percent-escapes into raw bytes. Returns `None` only on
/// malformed escapes — which layer 2 (`protocol::uri`) has already
/// excluded from anything that reaches a handler, so this module treats
/// `None` the same as any other rejection rather than distinguishing it.
/// Kept independent of layer 2's validator so this module's fuzz target
/// exercises the decode logic on its own, without depending on upstream
/// validation always running first.
fn percent_decode(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = *bytes.get(i + 1)?;
            let lo = *bytes.get(i + 2)?;
            let hex = |b: u8| (b as char).to_digit(16);
            let value = (hex(hi)? * 16 + hex(lo)?) as u8;
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

/// Fuzz entry point: `sanitize_request_path` must never panic on any input
/// and must never return a path containing a `..` component. Called from
/// `fuzz/fuzz_targets/static_path_sanitize.rs`.
pub fn fuzz_sanitize(request_path: &str) {
    if let Some(safe) = sanitize_request_path(request_path) {
        assert!(
            !safe
                .components()
                .any(|c| c == std::path::Component::ParentDir),
            "sanitize_request_path must never emit a ParentDir component for {request_path:?}"
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn tmp_docroot(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("usv-static-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir docroot");
        dir
    }

    #[test]
    fn percent_decode_basic() {
        assert_eq!(percent_decode("a%2Fb").unwrap(), b"a/b");
        assert_eq!(percent_decode("hello%20world").unwrap(), b"hello world");
        assert_eq!(percent_decode("noescapes").unwrap(), b"noescapes");
    }

    #[test]
    fn percent_decode_dot_dot() {
        assert_eq!(percent_decode("%2e%2e").unwrap(), b"..");
        assert_eq!(percent_decode("%2E%2E").unwrap(), b".."); // uppercase hex
    }

    #[test]
    fn percent_decode_null_byte() {
        assert_eq!(percent_decode("%00").unwrap(), vec![0u8]);
    }

    #[test]
    fn percent_decode_truncated_escape_fails() {
        assert!(percent_decode("%2").is_none());
        assert!(percent_decode("%").is_none());
    }

    #[tokio::test]
    async fn serves_a_real_file() {
        let root = tmp_docroot("basic");
        std::fs::write(root.join("hello.gmi"), b"# hi\n").expect("write");
        let resp = serve(&root, "/hello.gmi").await;
        assert_eq!(resp.header.status(), Status::Success);
        assert!(matches!(resp.body, Body::File(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn empty_path_serves_index() {
        let root = tmp_docroot("index");
        std::fs::write(root.join("index.gmi"), b"# home\n").expect("write");
        let resp = serve(&root, "/").await;
        assert_eq!(resp.header.status(), Status::Success);
        let resp2 = serve(&root, "").await;
        assert_eq!(resp2.header.status(), Status::Success);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn missing_file_is_not_found() {
        let root = tmp_docroot("missing");
        let resp = serve(&root, "/nope.gmi").await;
        assert_eq!(resp.header.status(), Status::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn literal_dotdot_traversal_is_not_found() {
        let root = tmp_docroot("traversal-literal");
        let outside = root.parent().expect("has parent");
        std::fs::write(outside.join("usv-secret-marker"), b"secret").ok();
        let resp = serve(&root, "/../usv-secret-marker").await;
        assert_eq!(resp.header.status(), Status::NotFound);
        let resp2 = serve(&root, "/sub/../../usv-secret-marker").await;
        assert_eq!(resp2.header.status(), Status::NotFound);
        let _ = std::fs::remove_file(outside.join("usv-secret-marker"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn percent_encoded_traversal_is_not_found() {
        let root = tmp_docroot("traversal-encoded");
        let resp = serve(&root, "/%2e%2e/%2e%2e/etc/passwd").await;
        assert_eq!(resp.header.status(), Status::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn double_encoded_traversal_is_not_found() {
        // %252e decodes ONCE to the literal string "%2e" (not a further
        // decode to '.') — this must still fail to resolve, not be
        // silently treated as safe because it isn't ".." after one pass.
        let root = tmp_docroot("traversal-double");
        let resp = serve(&root, "/%252e%252e/%252e%252e/etc/passwd").await;
        assert_eq!(resp.header.status(), Status::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn null_byte_in_path_is_not_found() {
        let root = tmp_docroot("nullbyte");
        std::fs::write(root.join("secret.gmi"), b"top secret").expect("write");
        let resp = serve(&root, "/secret.gmi%00.txt").await;
        assert_eq!(resp.header.status(), Status::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn symlink_escape_is_not_found() {
        #[cfg(unix)]
        {
            let root = tmp_docroot("symlink");
            let outside = root.parent().expect("has parent");
            std::fs::write(outside.join("usv-symlink-secret"), b"secret").ok();
            std::os::unix::fs::symlink(outside.join("usv-symlink-secret"), root.join("link"))
                .expect("symlink");
            let resp = serve(&root, "/link").await;
            assert_eq!(
                resp.header.status(),
                Status::NotFound,
                "a symlink pointing outside the docroot must not be servable"
            );
            let _ = std::fs::remove_file(outside.join("usv-symlink-secret"));
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[tokio::test]
    async fn directory_request_is_not_found_not_listed() {
        let root = tmp_docroot("dir-noindex");
        std::fs::create_dir_all(root.join("sub")).expect("mkdir");
        std::fs::write(root.join("sub/page.gmi"), b"x").expect("write");
        let resp = serve(&root, "/sub").await;
        assert_eq!(resp.header.status(), Status::NotFound);
        let resp2 = serve(&root, "/sub/").await;
        assert_eq!(resp2.header.status(), Status::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn redundant_slashes_are_collapsed() {
        let root = tmp_docroot("slashes");
        std::fs::create_dir_all(root.join("a")).expect("mkdir");
        std::fs::write(root.join("a/b.gmi"), b"x").expect("write");
        let resp = serve(&root, "//a///b.gmi").await;
        assert_eq!(resp.header.status(), Status::Success);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sanitize_rejects_dotdot_everywhere() {
        assert!(sanitize_request_path("../etc/passwd").is_none());
        assert!(sanitize_request_path("a/../../b").is_none());
        assert!(sanitize_request_path("%2e%2e/etc/passwd").is_none());
    }

    #[test]
    fn sanitize_accepts_ordinary_paths() {
        assert_eq!(
            sanitize_request_path("a/b/c").unwrap(),
            PathBuf::from("a/b/c")
        );
        assert_eq!(
            sanitize_request_path("a/./b").unwrap(),
            PathBuf::from("a/b")
        );
    }

    #[test]
    fn fuzz_sanitize_never_panics_or_emits_traversal() {
        for input in [
            "",
            "/",
            "..",
            "../",
            "%2e%2e",
            "%252e%252e",
            "a/../b",
            "a/b/../../c",
            "%00",
            "\u{0}",
        ] {
            fuzz_sanitize(input);
        }
    }
}
