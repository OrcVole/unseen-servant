//! Serving the rendered gopher tree (ADR 0012).
//!
//! Deliberately dull, because all the deciding happened at render time:
//! the tree under `<state_dir>/gopher` already contains exactly what may
//! be served — gated pages were never written into it (ADR 0012 §6) —
//! so this maps a selector to a file and returns bytes.
//!
//! Gopher has no status codes. Every failure is an ordinary one-line
//! menu of item type 3, which is why nothing here returns a `Result` to
//! the caller: a "not found" *is* a valid response.

use std::path::{Path, PathBuf};

use crate::protocol::gopher::{self, ItemType};
use crate::render::gopher::{Context, caps_txt};

/// Serve one selector from `root`.
pub async fn serve(selector: &str, root: &Path, ctx: &Context) -> Vec<u8> {
    let sel = selector.trim();

    // The conventional capability file, generated rather than stored so
    // it cannot drift from the running configuration.
    if sel == "/caps.txt" || sel == "caps.txt" {
        return gopher::text_body(&caps_txt(ctx)).into_bytes();
    }

    let Some(path) = resolve(sel, root) else {
        // Traversal, a NUL, or anything else the shared sanitiser
        // refuses. The same five-step defence the Gemini static handler
        // uses — this deliberately does not get its own path logic.
        return gopher::error_menu("bad selector").into_bytes();
    };

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            // A menu is already framed (it ends with its own lastline);
            // a text file has to be dot-stuffed and terminated, or a
            // line beginning with "." truncates the document. Binary is
            // written raw and framed by the connection close.
            match classify(&path) {
                Served::Menu => bytes,
                Served::Text => match String::from_utf8(bytes) {
                    Ok(text) => gopher::text_body(&text).into_bytes(),
                    // Claimed to be text, is not UTF-8: send it raw
                    // rather than mangling it.
                    Err(e) => e.into_bytes(),
                },
                Served::Binary => bytes,
            }
        }
        Err(_) => gopher::error_menu("not found").into_bytes(),
    }
}

enum Served {
    Menu,
    Text,
    Binary,
}

fn classify(path: &Path) -> Served {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".gmi") || lower.ends_with(".gemini") {
        // Rendered menus live at the source path; the file at
        // `foo.gmi` in the gopher tree *is* the menu for that page.
        return Served::Menu;
    }
    match ItemType::for_path(name) {
        ItemType::Text => Served::Text,
        _ => Served::Binary,
    }
}

/// Map a selector onto a file inside `root`, or refuse it.
///
/// Reuses the static handler's sanitiser rather than growing a second
/// path-safety implementation — that one is fuzzed, has a regression
/// corpus of percent-encoded and NUL-injected attempts, and any
/// divergence between the two would be a hole by definition.
fn resolve(selector: &str, root: &Path) -> Option<PathBuf> {
    let s = if selector.is_empty() { "/" } else { selector };
    let relative = crate::handler::static_file::sanitize_request_path(s)?;
    let mut path = root.join(relative);
    // A directory selector means that directory's index page, which is
    // what a client following a type-1 link to `/notes/` expects.
    if path.is_dir() || s.ends_with('/') {
        path = path.join("index.gmi");
    }
    Some(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "unwrap is idiomatic in tests")]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            host: "example.org".into(),
            port: 70,
        }
    }

    fn tree() -> tempdir::Dir {
        let d = tempdir::Dir::new("usv-gopher-serve");
        std::fs::create_dir_all(d.path().join("notes")).unwrap();
        std::fs::write(
            d.path().join("index.gmi"),
            "iHome\t\terror.host\t1\r\n.\r\n",
        )
        .unwrap();
        std::fs::write(
            d.path().join("notes/index.gmi"),
            "iNotes\t\terror.host\t1\r\n.\r\n",
        )
        .unwrap();
        std::fs::write(d.path().join("readme.txt"), "line one\n.hidden\n").unwrap();
        d
    }

    /// Minimal scratch-directory helper (the crate avoids a tempfile dep).
    mod tempdir {
        use std::path::{Path, PathBuf};
        pub struct Dir(PathBuf);
        impl Dir {
            pub fn new(tag: &str) -> Self {
                // Unique per instance, not just per process: these tests
                // run in parallel threads, and a shared directory means
                // one test's cleanup deletes another's fixtures.
                use std::sync::atomic::{AtomicU32, Ordering};
                static N: AtomicU32 = AtomicU32::new(0);
                let n = N.fetch_add(1, Ordering::Relaxed);
                let p = std::env::temp_dir().join(format!("{tag}-{}-{n}", std::process::id()));
                let _ = std::fs::remove_dir_all(&p);
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[tokio::test]
    async fn an_empty_selector_serves_the_root_menu() {
        let d = tree();
        let out = serve("", d.path(), &ctx()).await;
        assert!(String::from_utf8_lossy(&out).contains("iHome"));
    }

    #[tokio::test]
    async fn a_directory_selector_serves_its_index() {
        let d = tree();
        let out = serve("/notes/", d.path(), &ctx()).await;
        assert!(String::from_utf8_lossy(&out).contains("iNotes"));
    }

    #[tokio::test]
    async fn a_menu_is_served_already_framed() {
        let d = tree();
        let out = serve("/index.gmi", d.path(), &ctx()).await;
        let s = String::from_utf8_lossy(&out);
        assert!(s.ends_with(".\r\n"));
        // Not double-terminated: the file already carried its lastline.
        assert_eq!(s.matches("\r\n.\r\n").count(), 1, "{s:?}");
    }

    #[tokio::test]
    async fn a_text_file_is_dot_stuffed_so_it_cannot_truncate() {
        let d = tree();
        let out = serve("/readme.txt", d.path(), &ctx()).await;
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("..hidden\r\n"), "leading dot not stuffed: {s:?}");
        assert!(s.ends_with(".\r\n"));
    }

    #[tokio::test]
    async fn a_missing_selector_is_a_type_3_menu_not_a_hang() {
        let d = tree();
        let out = serve("/nope.gmi", d.path(), &ctx()).await;
        let s = String::from_utf8_lossy(&out);
        assert!(s.starts_with('3'), "{s:?}");
        assert!(s.ends_with(".\r\n"));
    }

    #[tokio::test]
    async fn traversal_is_refused_by_the_shared_sanitiser() {
        let d = tree();
        for attempt in ["/../../etc/passwd", "/..%2f..%2fetc/passwd", "/a\0b"] {
            let out = serve(attempt, d.path(), &ctx()).await;
            let s = String::from_utf8_lossy(&out);
            assert!(s.starts_with('3'), "{attempt} leaked: {s:?}");
            assert!(!s.contains("root:"), "{attempt} leaked passwd");
        }
    }

    #[tokio::test]
    async fn caps_txt_is_generated_from_the_live_context() {
        let d = tree();
        let out = serve("/caps.txt", d.path(), &ctx()).await;
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("ServerSoftware=unseen-servant"));
        assert!(s.contains("ServerHost=example.org"));
        assert!(s.ends_with(".\r\n"));
    }
}
