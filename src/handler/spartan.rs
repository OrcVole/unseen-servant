//! Serving Spartan from the cleartext tree (ADR 0012).
//!
//! Spartan's document format *is* gemtext, so there is no third render
//! target: it serves the same files the gopher tree already holds —
//! which is also what keeps the wall (ADR 0012 §6) honest, since gated
//! pages were never written there.
//!
//! One request per connection, no keep-alive, and the response is a
//! status line followed by an optional body.

use std::path::{Path, PathBuf};

use crate::protocol::spartan::{self, Request};

/// Serve one parsed request from `root`.
pub async fn serve(req: &Request, root: &Path) -> Vec<u8> {
    // Uploads first, before anything touches the filesystem: a write
    // attempt is refused on its declared length alone (ADR 0012 §5).
    if req.content_length > 0 {
        return spartan::client_error(
            "uploads are not accepted here — this server only takes authenticated writes, \
             over Titan, which Spartan cannot express",
        )
        .into_bytes();
    }

    let Some(path) = resolve(&req.path, root) else {
        return spartan::client_error("bad path").into_bytes();
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mut out = spartan::success(&mime_for(&path)).into_bytes();
            out.extend_from_slice(&bytes);
            out
        }
        Err(_) => spartan::client_error("not found").into_bytes(),
    }
}

/// The MIME type for a served file.
///
/// Gemtext is the default document type, and UTF-8 is the default
/// encoding for `text/*` in Spartan, so it is stated rather than
/// assumed.
fn mime_for(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".gmi") || lower.ends_with(".gemini") {
        "text/gemini;charset=utf-8".to_string()
    } else {
        crate::handler::mime::lookup(name).to_string()
    }
}

/// Map a request path onto a file, or refuse it.
///
/// Same shared sanitiser as the Gemini and gopher handlers — one
/// fuzzed implementation with one regression corpus, because two would
/// eventually disagree and the disagreement would be the hole.
fn resolve(request_path: &str, root: &Path) -> Option<PathBuf> {
    let relative = crate::handler::static_file::sanitize_request_path(request_path)?;
    let mut path = root.join(relative);
    if path.is_dir() || request_path.ends_with('/') {
        path = path.join("index.gmi");
    }
    Some(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "unwrap is idiomatic in tests")]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "usv-spartan-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("sub")).unwrap();
        std::fs::write(p.join("index.gmi"), "# Home\n").unwrap();
        std::fs::write(p.join("sub/index.gmi"), "# Sub\n").unwrap();
        std::fs::write(p.join("note.txt"), "plain\n").unwrap();
        p
    }

    fn req(path: &str, len: u64) -> Request {
        Request {
            host: "example.org".into(),
            path: path.into(),
            content_length: len,
        }
    }

    #[tokio::test]
    async fn a_page_is_served_as_gemtext_with_utf8_stated() {
        let d = tmp("ok");
        let out = String::from_utf8(serve(&req("/index.gmi", 0), &d).await).unwrap();
        assert!(out.starts_with("2 text/gemini;charset=utf-8\r\n"), "{out}");
        assert!(out.contains("# Home"));
    }

    #[tokio::test]
    async fn a_directory_serves_its_index() {
        let d = tmp("dir");
        let out = String::from_utf8(serve(&req("/sub/", 0), &d).await).unwrap();
        assert!(out.contains("# Sub"), "{out}");
    }

    #[tokio::test]
    async fn a_non_gemtext_file_keeps_its_own_type() {
        let d = tmp("mime");
        let out = String::from_utf8(serve(&req("/note.txt", 0), &d).await).unwrap();
        assert!(out.starts_with("2 text/plain"), "{out}");
    }

    #[tokio::test]
    async fn every_upload_is_refused_without_touching_the_disk() {
        // Refused on the DECLARED length, so a peer announcing four
        // gigabytes gets a refusal rather than a server making room.
        let d = tmp("upload");
        for len in [1u64, 1024, u64::MAX] {
            let out = String::from_utf8(serve(&req("/index.gmi", len), &d).await).unwrap();
            assert!(out.starts_with('4'), "len {len} was not refused: {out}");
            assert!(
                out.contains("Titan"),
                "should say where writes do work: {out}"
            );
            assert!(!out.contains("# Home"), "served content anyway: {out}");
        }
    }

    #[tokio::test]
    async fn a_missing_page_is_a_client_error() {
        let d = tmp("404");
        let out = String::from_utf8(serve(&req("/nope.gmi", 0), &d).await).unwrap();
        assert!(out.starts_with("4 not found"), "{out}");
    }

    #[tokio::test]
    async fn traversal_is_refused_by_the_shared_sanitiser() {
        let d = tmp("trav");
        for attempt in ["/../../etc/passwd", "/..%2f..%2fetc/passwd", "/a\0b"] {
            let out = String::from_utf8(serve(&req(attempt, 0), &d).await).unwrap();
            assert!(out.starts_with('4'), "{attempt}: {out}");
            assert!(!out.contains("root:"), "{attempt} leaked passwd");
        }
    }
}
