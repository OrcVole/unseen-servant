//! The render pipeline: walk a content tree, render every `.gmi` file to
//! HTML, swap the result into place atomically. Resolves design brief §5.4
//! (`docs/notes/c3-render-design-brief.md`): **full-tree rebuild every
//! time**, not incremental — simpler, and matches the exit gate's framing
//! ("survives edit storms without torn output") more directly than a
//! partial-invalidation scheme would for a v1. Feed emission (Atom/gemsub)
//! and non-`.gmi` asset copying are not yet wired into this pass — see the
//! module-level "Known gaps" note below.
//!
//! **Staging swap** (design brief §5.3): render into `${state_dir}/
//! html.tmp`, then swap it into `${state_dir}/html` via two renames
//! (existing `html` → `html.old`, `html.tmp` → `html`, then remove
//! `html.old`). Each individual rename is atomic on POSIX, so a reader
//! never sees a half-written tree — but the two renames are not a single
//! atomic operation, so there is a real, if narrow, window between them
//! where `html` does not exist at all. `renameat2(RENAME_EXCHANGE)` would
//! close that window but isn't in `std`; noted here rather than silently
//! assumed away, per house policy against unstated caps.
//!
//! **Known gaps, stated rather than hidden:**
//! - Non-`.gmi` files (images, downloads) in the content tree are not
//!   copied into the HTML output tree. The Gemini surface already serves
//!   them directly (C2's static file handler reads `content_dir`
//!   itself); the HTML surface needs its own copy or a shared-asset
//!   strategy, deferred to a follow-up.
//! - Feed emission (`feed::atom`, `feed::gemsub`) is built and tested but
//!   not yet invoked from this pipeline — an index page's dated links
//!   don't yet produce an `atom.xml` or updated gemsub block as a side
//!   effect of rendering. Follow-up, not forgotten.
//! - The fs-event watcher that would call this on content changes
//!   (`watcher.rs`, BUILD-PLAN's "debounce" requirement) does not exist
//!   yet; `render_tree` is currently invoked only by whatever the caller
//!   chooses (tests, or a future manual `usv render --force`).

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use super::{gemtext, html};

/// What one `render_tree` run did, for logging/testing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// Number of `.gmi` files rendered to HTML.
    pub pages_rendered: usize,
}

/// Render every `.gmi` file under `content_dir` into `${state_dir}/html`,
/// via the atomic-ish staging swap described in the module docs. Creates
/// `state_dir` and the content tree's directory structure as needed;
/// returns an error only for genuine I/O failure (permissions, disk
/// full) — a `content_dir` that doesn't exist yet renders an empty tree
/// rather than erroring, since a fresh capsule with no content authored
/// yet is a normal state, not a fault.
pub async fn render_tree(content_dir: &Path, state_dir: &Path) -> std::io::Result<RenderStats> {
    let staging = state_dir.join("html.tmp");
    let live = state_dir.join("html");
    let old = state_dir.join("html.old");

    // A staging dir surviving from a crashed prior run must not leak
    // stale pages into this run's output.
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging).await?;

    let mut stats = RenderStats::default();
    if tokio::fs::metadata(content_dir).await.is_ok() {
        render_dir(content_dir, content_dir, &staging, &mut stats).await?;
    }

    let _ = tokio::fs::remove_dir_all(&old).await;
    if tokio::fs::metadata(&live).await.is_ok() {
        tokio::fs::rename(&live, &old).await?;
    }
    tokio::fs::rename(&staging, &live).await?;
    let _ = tokio::fs::remove_dir_all(&old).await;

    Ok(stats)
}

/// Recursive directory walk. Async fns can't recurse directly (the
/// future's size would be infinite); boxing is the standard pattern.
fn render_dir<'a>(
    root: &'a Path,
    dir: &'a Path,
    staging_root: &'a Path,
    stats: &'a mut RenderStats,
) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                render_dir(root, &path, staging_root, stats).await?;
            } else if file_type.is_file() && is_gemtext(&path) {
                render_page(root, &path, staging_root).await?;
                stats.pages_rendered += 1;
            }
        }
        Ok(())
    })
}

fn is_gemtext(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("gmi")
}

/// Render one `.gmi` file into its mirrored `.html` path under
/// `staging_root`, preserving the content tree's own directory structure.
async fn render_page(root: &Path, path: &Path, staging_root: &Path) -> std::io::Result<()> {
    let text = tokio::fs::read_to_string(path).await?;
    let lines = gemtext::parse(&text);
    let title = gemtext::extract_title(&lines, path);
    let doc = html::render_document(&lines, &title);

    let relative = path.strip_prefix(root).unwrap_or(path);
    let out_path = html_output_path(staging_root, relative);
    if let Some(parent) = out_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&out_path, doc).await
}

/// `staging_root` joined with `relative`, extension swapped `.gmi` →
/// `.html`. Pure and separated from I/O so path mapping is unit-testable
/// without a filesystem.
fn html_output_path(staging_root: &Path, relative: &Path) -> PathBuf {
    let mut out = staging_root.join(relative);
    out.set_extension("html");
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("usv-pipeline-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn output_path_swaps_extension() {
        assert_eq!(
            html_output_path(Path::new("/out"), Path::new("about.gmi")),
            PathBuf::from("/out/about.html")
        );
        assert_eq!(
            html_output_path(Path::new("/out"), Path::new("blog/post.gmi")),
            PathBuf::from("/out/blog/post.html")
        );
    }

    #[tokio::test]
    async fn renders_a_simple_tree() {
        let base = tmp_dir("simple");
        let content = base.join("content");
        std::fs::create_dir_all(content.join("blog")).unwrap();
        std::fs::write(content.join("index.gmi"), "# Home\n").unwrap();
        std::fs::write(content.join("blog/post.gmi"), "# A Post\n").unwrap();
        std::fs::write(content.join("blog/notes.txt"), "not gemtext").unwrap();

        let stats = render_tree(&content, &base).await.unwrap();
        assert_eq!(stats.pages_rendered, 2);

        let index_html = std::fs::read_to_string(base.join("html/index.html")).unwrap();
        assert!(index_html.contains("<h1>Home</h1>"));
        let post_html = std::fs::read_to_string(base.join("html/blog/post.html")).unwrap();
        assert!(post_html.contains("<h1>A Post</h1>"));
        assert!(
            !base.join("html/blog/notes.txt").exists(),
            "non-.gmi files are not copied (documented gap)"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn missing_content_dir_renders_empty_tree_not_an_error() {
        let base = tmp_dir("missing-content");
        let content = base.join("does-not-exist");
        let stats = render_tree(&content, &base).await.unwrap();
        assert_eq!(stats.pages_rendered, 0);
        assert!(base.join("html").is_dir());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn rerendering_replaces_stale_pages() {
        let base = tmp_dir("rerender");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(content.join("a.gmi"), "# First\n").unwrap();
        render_tree(&content, &base).await.unwrap();
        assert!(
            std::fs::read_to_string(base.join("html/a.html"))
                .unwrap()
                .contains("First")
        );

        // Remove a.gmi, add b.gmi: a re-render (full-tree, per design)
        // must not leave a.html behind from the stale run.
        std::fs::remove_file(content.join("a.gmi")).unwrap();
        std::fs::write(content.join("b.gmi"), "# Second\n").unwrap();
        let stats = render_tree(&content, &base).await.unwrap();
        assert_eq!(stats.pages_rendered, 1);
        assert!(
            !base.join("html/a.html").exists(),
            "stale output must not survive a re-render"
        );
        assert!(base.join("html/b.html").exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn nested_directories_are_walked() {
        let base = tmp_dir("nested");
        let content = base.join("content");
        std::fs::create_dir_all(content.join("a/b/c")).unwrap();
        std::fs::write(content.join("a/b/c/deep.gmi"), "# Deep\n").unwrap();
        let stats = render_tree(&content, &base).await.unwrap();
        assert_eq!(stats.pages_rendered, 1);
        assert!(base.join("html/a/b/c/deep.html").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn a_stale_staging_dir_from_a_crashed_run_does_not_leak() {
        let base = tmp_dir("crashed-staging");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(content.join("real.gmi"), "# Real\n").unwrap();

        // Simulate a crashed previous run: a leftover html.tmp with a
        // page that no longer exists in content.
        let leftover_staging = base.join("html.tmp");
        std::fs::create_dir_all(&leftover_staging).unwrap();
        std::fs::write(leftover_staging.join("ghost.html"), "should never appear").unwrap();

        render_tree(&content, &base).await.unwrap();
        assert!(base.join("html/real.html").exists());
        assert!(
            !base.join("html/ghost.html").exists(),
            "a leftover staging dir from a crashed run must not leak into a fresh render"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
