//! Nex: the smallest protocol usv speaks (ADR 0012).
//!
//! The client sends a path terminated by a newline; the server writes
//! the document and closes. There is no status line, no MIME type, and
//! no terminator beyond the close. Content type is inferred by the
//! client from the extension.
//!
//! Because the protocol has **no way to signal failure**, "not found" is
//! by convention an ordinary human-readable body. A Nex client cannot
//! machine-distinguish an error page from content — worth stating in the
//! honesty section of the docs rather than glossing, since it is a real
//! limitation of any Nex mirror, not of this implementation.
//!
//! Nex reuses gemtext's `=> ` link syntax inside plain text, so the
//! existing tree serves as-is: no third render target, and the same
//! cleartext tree the gopher target already builds — which is what keeps
//! the wall (ADR 0012 §6) intact here for free, since gated pages were
//! never written there.
//!
//! Parsing is inline rather than in `protocol/`: "a path, then a
//! newline" has no grammar worth a module of its own, and the traversal
//! defence — the only part that matters — is the shared sanitiser.
//!
//! Note for operators: Nex uses **TCP** 1900. Port scanners tag 1900 as
//! SSDP/UPnP, which is UDP. They do not collide.

use std::path::Path;

/// Longest request accepted, matching the other cleartext listeners.
pub const MAX_REQUEST_BYTES: usize = 1024;

/// Serve one Nex request line from `root`.
pub async fn serve(
    line: &[u8],
    root: &Path,
    addrs: &crate::render::colophon::Addresses,
) -> Vec<u8> {
    let Some(nl) = line.iter().position(|&b| b == b'\n') else {
        return b"bad request\n".to_vec();
    };
    let mut raw = &line[..nl];
    if raw.last() == Some(&b'\r') {
        raw = &raw[..raw.len() - 1];
    }
    // Control bytes never appear in a legitimate path and are a standard
    // way to smuggle something past a later layer.
    if raw.iter().any(|&b| b < 0x20 || b == 0x7f) {
        return b"bad request\n".to_vec();
    }
    let Ok(text) = std::str::from_utf8(raw) else {
        return b"bad request\n".to_vec();
    };

    let requested = if text.trim().is_empty() {
        "/"
    } else {
        text.trim()
    };
    // A leading slash is optional in Nex; normalise before sanitising so
    // the shared implementation sees what it expects.
    let normalised = if requested.starts_with('/') {
        requested.to_string()
    } else {
        format!("/{requested}")
    };

    let Some(relative) = crate::handler::static_file::sanitize_request_path(&normalised) else {
        return b"bad request\n".to_vec();
    };
    let mut path = root.join(relative);
    if path.is_dir() || normalised.ends_with('/') {
        path = path.join("index.gmi");
    }

    match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        // The colophon fills this gap rather than occupying a slot: an
        // operator file at the same path was read above and won.
        Err(_) if crate::render::colophon::matches(&normalised) => {
            crate::render::colophon::gemtext(crate::render::colophon::Protocol::Nex, addrs)
                .into_bytes()
        }
        // No status codes exist: this *is* the error report, and a
        // client cannot tell it from a page that happens to say so.
        Err(_) => b"not found\n".to_vec(),
    }
}

/// Where a Nex request maps to, exposed for tests.
#[cfg(test)]
fn resolve_for_test(requested: &str, root: &Path) -> Option<std::path::PathBuf> {
    let normalised = if requested.starts_with('/') {
        requested.to_string()
    } else {
        format!("/{requested}")
    };
    let relative = crate::handler::static_file::sanitize_request_path(&normalised)?;
    Some(root.join(relative))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "unwrap is idiomatic in tests")]
mod tests {
    use super::*;

    fn addrs() -> crate::render::colophon::Addresses {
        crate::render::colophon::Addresses {
            host: "example.org".into(),
            nex_port: Some(1900),
            ..Default::default()
        }
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "usv-nex-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("sub")).unwrap();
        std::fs::write(p.join("index.gmi"), "# Home\n=> sub/ Deeper\n").unwrap();
        std::fs::write(p.join("sub/index.gmi"), "# Sub\n").unwrap();
        p
    }

    #[tokio::test]
    async fn an_empty_path_serves_the_root_document() {
        let d = tmp("root");
        let out = String::from_utf8(serve(b"\n", &d, &addrs()).await).unwrap();
        assert!(out.contains("# Home"), "{out}");
    }

    #[tokio::test]
    async fn a_path_without_a_leading_slash_works() {
        // Nex clients send bare paths; a leading slash is optional.
        let d = tmp("bare");
        let out = String::from_utf8(serve(b"sub/\n", &d, &addrs()).await).unwrap();
        assert!(out.contains("# Sub"), "{out}");
    }

    #[tokio::test]
    async fn gemtext_link_lines_are_served_verbatim() {
        // Nex reuses "=> " as its own link convention, so no rewriting.
        let d = tmp("links");
        let out = String::from_utf8(serve(b"/\n", &d, &addrs()).await).unwrap();
        assert!(out.contains("=> sub/ Deeper"), "{out}");
    }

    #[tokio::test]
    async fn a_missing_document_is_prose_because_nex_cannot_signal() {
        let d = tmp("404");
        let out = String::from_utf8(serve(b"/nope.gmi\n", &d, &addrs()).await).unwrap();
        assert_eq!(out, "not found\n");
    }

    #[tokio::test]
    async fn traversal_is_refused_by_the_shared_sanitiser() {
        let d = tmp("trav");
        for attempt in [
            &b"/../../etc/passwd\n"[..],
            &b"../../etc/passwd\n"[..],
            &b"/..%2f..%2fetc/passwd\n"[..],
            &b"/a\0b\n"[..],
        ] {
            let out = String::from_utf8_lossy(&serve(attempt, &d, &addrs()).await).to_string();
            assert!(!out.contains("root:"), "{attempt:?} leaked passwd");
        }
    }

    #[tokio::test]
    async fn a_line_without_a_terminator_is_a_bad_request() {
        let d = tmp("noterm");
        let out = String::from_utf8(serve(b"/index.gmi", &d, &addrs()).await).unwrap();
        assert_eq!(out, "bad request\n");
    }

    #[test]
    fn paths_normalise_the_same_with_or_without_a_leading_slash() {
        let root = Path::new("/tmp/x");
        assert_eq!(
            resolve_for_test("a/b.gmi", root),
            resolve_for_test("/a/b.gmi", root)
        );
    }

    #[tokio::test]
    async fn the_colophon_explains_usv_and_names_this_protocol() {
        let d = tmp("colophon");
        let out = String::from_utf8(serve(b"/usv\n", &d, &addrs()).await).unwrap();
        assert!(out.contains("UnSeen serVant"), "{out}");
        assert!(out.contains("This is a Nex page"), "{out}");
        assert!(out.contains("nex://example.org:1900/"), "{out}");
        assert!(out.contains("gelim"), "no native client listed: {out}");
    }

    #[tokio::test]
    async fn an_operator_file_wins_over_the_generated_colophon() {
        // Same rule finger.txt follows: generated text fills a gap, it
        // never overwrites words the operator wrote.
        let d = tmp("colophon-override");
        std::fs::write(d.join("usv"), "my own words\n").unwrap();
        let out = String::from_utf8(serve(b"/usv\n", &d, &addrs()).await).unwrap();
        assert_eq!(out, "my own words\n");
    }
}
