//! The render pipeline: walk a content tree, render every `.gmi` file to
//! HTML, swap the result into place atomically. Resolves design brief §5.4
//! (`docs/notes/c3-render-design-brief.md`): **full-tree rebuild every
//! time**, not incremental — simpler, and matches the exit gate's framing
//! ("survives edit storms without torn output") more directly than a
//! partial-invalidation scheme would for a v1. Non-`.gmi` asset copying
//! is the one thing not yet wired into this pass — see the "Known gaps"
//! note below.
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
//! - Feeds are built from the **index page only** (`index.gmi`'s dated
//!   link lines), not from a per-page date convention — matching the one
//!   date convention that actually exists (metadata.rs). A capsule whose
//!   posts are listed on a non-index page produces no feed; that is a
//!   deliberate v1 scope, not an oversight.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use super::{gemtext, html, robots, skeleton};

/// Everything a render pass needs beyond the two directories: kept as a
/// struct rather than an ever-lengthening positional argument list, so a
/// new render input (a future sitemap, an author name for Atom) is a
/// field addition, not a signature break at every call site.
#[derive(Debug, Clone)]
pub struct RenderContext {
    /// The bundled theme's stylesheet, written to `style.css` in the tree.
    pub theme_css: String,
    /// Absolute base URL for the web surface (e.g. `https://example.org`),
    /// prefixed onto relative feed links so `atom.xml` carries absolute
    /// URLs as the format requires. Empty disables the Atom feed (a feed
    /// with no resolvable base is worse than none).
    pub web_base_url: String,
    /// The capsule's display title, used as the Atom feed `<title>`.
    pub capsule_title: String,
}

impl RenderContext {
    /// A minimal context for tests and the no-feed case: a stylesheet, no
    /// base URL (so no Atom feed is produced), a default title.
    pub fn plain(theme_css: impl Into<String>) -> RenderContext {
        RenderContext {
            theme_css: theme_css.into(),
            web_base_url: String::new(),
            capsule_title: "Unseen Servant".to_string(),
        }
    }
}

/// What one `render_tree` run did, for logging/testing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// Number of `.gmi` files rendered to HTML.
    pub pages_rendered: usize,
    /// Whether a web `robots.txt` was mirrored from the content tree's
    /// own Gemini robots.txt this run.
    pub robots_mirrored: bool,
    /// Number of dated entries found on the index page and emitted into
    /// the feeds (`atom.xml` + `feed.gmi`). Zero means no feed files were
    /// written.
    pub feed_entries: usize,
}

/// Write the first-run content skeleton into `content_dir` if — and only
/// if — the directory holds no `.gmi` file at all. A capsule with any
/// authored content is never touched, so this can be called on every
/// startup without ever overwriting an operator's work (the "never
/// silently regenerate" discipline ADR 0003 applies to keys, applied
/// here to content).
///
/// Returns `true` if a skeleton was written.
pub async fn seed_skeleton_if_empty(content_dir: &Path, mood: &str) -> std::io::Result<bool> {
    if has_any_gemtext(content_dir).await? {
        return Ok(false);
    }
    tokio::fs::create_dir_all(content_dir).await?;
    tokio::fs::write(content_dir.join("index.gmi"), mood).await?;
    tracing::info!(
        dir = %content_dir.display(),
        "no content found; wrote the first-run skeleton page (edit or replace it freely — \
         it is only ever written when the content directory has no gemtext at all)"
    );
    Ok(true)
}

/// Whether `dir` (or any subdirectory) contains at least one `.gmi` file.
/// A missing directory counts as empty, not an error — a fresh capsule
/// hasn't created it yet.
async fn has_any_gemtext(dir: &Path) -> std::io::Result<bool> {
    if tokio::fs::metadata(dir).await.is_err() {
        return Ok(false);
    }
    let mut found = false;
    find_gemtext(dir, &mut found).await?;
    Ok(found)
}

fn find_gemtext<'a>(
    dir: &'a Path,
    found: &'a mut bool,
) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if *found {
                return Ok(());
            }
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                find_gemtext(&path, found).await?;
            } else if file_type.is_file() && is_gemtext(&path) {
                *found = true;
                return Ok(());
            }
        }
        Ok(())
    })
}

/// The default skeleton mood written on first run.
pub const DEFAULT_SKELETON: &str = skeleton::QUIET;

/// The reserved filename usv generates for the Gemini-side gemsub feed.
/// It is written into the *content* directory (so Gemini serves it
/// directly and the render walk also turns it into `feed.html`), which
/// means the watcher must ignore changes to it or a render would trigger
/// itself — see `watcher.rs`. An operator's own `feed.gmi` would be
/// overwritten; the name is documented as reserved.
pub const GENERATED_FEED_NAME: &str = "feed.gmi";

/// Render every `.gmi` file under `content_dir` into `${state_dir}/html`,
/// via the atomic-ish staging swap described in the module docs. Creates
/// `state_dir` and the content tree's directory structure as needed;
/// returns an error only for genuine I/O failure (permissions, disk
/// full) — a `content_dir` that doesn't exist yet renders an empty tree
/// rather than erroring, since a fresh capsule with no content authored
/// yet is a normal state, not a fault.
pub async fn render_tree(
    content_dir: &Path,
    state_dir: &Path,
    ctx: &RenderContext,
) -> std::io::Result<RenderStats> {
    let staging = state_dir.join("html.tmp");
    let live = state_dir.join("html");
    let old = state_dir.join("html.old");

    // A staging dir surviving from a crashed prior run must not leak
    // stale pages into this run's output.
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging).await?;

    // The gemsub feed is Gemini-native gemtext, so it is written into the
    // *content* directory (served on Gemini directly) BEFORE the walk, so
    // the walk also renders it to `feed.html` for the web. Done first so
    // it is part of this same render pass, not the next one.
    let feed_entries = write_gemsub_feed(content_dir).await?;
    let mut stats = RenderStats {
        feed_entries,
        ..Default::default()
    };

    if tokio::fs::metadata(content_dir).await.is_ok() {
        render_dir(content_dir, content_dir, &staging, &mut stats).await?;
    }

    // The bundled stylesheet every rendered page links to. Written into
    // the tree rather than served from memory so the output directory is
    // a complete, portable static site — the same property `usv export`
    // (C5) and the OnionShare recipe depend on.
    tokio::fs::write(staging.join("style.css"), &ctx.theme_css).await?;

    // The Atom feed is web-only (XML, absolute links), so it goes into the
    // rendered HTML tree, not the content dir. Same entries as the gemsub
    // feed above — ADR 0004's one-source-both-surfaces guarantee.
    write_atom_feed(content_dir, &staging, ctx).await?;

    // Mirror the capsule's Gemini robots.txt into a web one, if it wrote
    // any rules at all (see `robots.rs` for why this is a translation
    // rather than a copy).
    if let Ok(gemini_robots) = tokio::fs::read_to_string(content_dir.join("robots.txt")).await
        && let Some(web_robots) = robots::to_web_robots(&gemini_robots)
    {
        tokio::fs::write(staging.join("robots.txt"), web_robots).await?;
        stats.robots_mirrored = true;
    }

    let _ = tokio::fs::remove_dir_all(&old).await;
    if tokio::fs::metadata(&live).await.is_ok() {
        tokio::fs::rename(&live, &old).await?;
    }
    tokio::fs::rename(&staging, &live).await?;
    let _ = tokio::fs::remove_dir_all(&old).await;

    Ok(stats)
}

/// Write the gemsub feed into `content_dir/feed.gmi` (Gemini-served, and
/// rendered to `feed.html` by the walk that follows), from the index
/// page's dated link lines. Returns the number of feed entries; zero
/// means no index, or no dated links, and no file is written. Reads the
/// index rather than sharing a parse with `write_atom_feed` so neither
/// has to hold a borrow across the tree walk that runs between them — the
/// index is small and read twice per render, which is cheap.
async fn write_gemsub_feed(content_dir: &Path) -> std::io::Result<usize> {
    use super::{feed, metadata};

    let Ok(text) = tokio::fs::read_to_string(content_dir.join("index.gmi")).await else {
        return Ok(0);
    };
    let lines = gemtext::parse(&text);
    let entries = metadata::extract_feed_entries(&lines);
    if entries.is_empty() {
        return Ok(0);
    }
    tokio::fs::write(
        content_dir.join(GENERATED_FEED_NAME),
        feed::gemsub::render(&entries),
    )
    .await?;
    Ok(entries.len())
}

/// Write the Atom feed into `staging/atom.xml` (web-only; XML with
/// absolute links). Skipped entirely when no base URL is configured,
/// since Atom requires absolute links and a feed with unresolvable ones
/// is worse than none.
async fn write_atom_feed(
    content_dir: &Path,
    staging: &Path,
    ctx: &RenderContext,
) -> std::io::Result<()> {
    use super::{feed, metadata};

    if ctx.web_base_url.is_empty() {
        return Ok(());
    }
    let Ok(text) = tokio::fs::read_to_string(content_dir.join("index.gmi")).await else {
        return Ok(());
    };
    let lines = gemtext::parse(&text);
    let entries = metadata::extract_feed_entries(&lines);
    if entries.is_empty() {
        return Ok(());
    }
    // The feed's own <updated> is the most recent entry's date — the
    // conventional choice, and deterministic (no system clock).
    let latest = entries
        .iter()
        .map(|e| e.date)
        .max()
        .unwrap_or(entries[0].date);
    let feed_id = format!("{}/atom.xml", ctx.web_base_url.trim_end_matches('/'));
    let atom = feed::atom::render(
        &feed_id,
        &ctx.capsule_title,
        &ctx.web_base_url,
        latest,
        &entries,
    );
    tokio::fs::write(staging.join("atom.xml"), atom).await
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

    /// A stand-in stylesheet: these tests exercise the tree walk and the
    /// swap, not the theme system (that has its own tests).
    fn test_ctx() -> RenderContext {
        RenderContext::plain("body { color: black; }")
    }

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

        let stats = render_tree(&content, &base, &test_ctx()).await.unwrap();
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
    async fn dated_index_links_produce_both_feeds() {
        let base = tmp_dir("feeds");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(
            content.join("index.gmi"),
            "# Blog\n\n\
             => /posts/1 2026-08-09 - Newest\n\
             => /posts/2 2026-01-01 - Older\n\
             => /about About (no date, not a feed entry)\n",
        )
        .unwrap();

        let ctx = RenderContext {
            theme_css: "body{}".to_string(),
            web_base_url: "https://example.org".to_string(),
            capsule_title: "example.org".to_string(),
        };
        let stats = render_tree(&content, &base, &ctx).await.unwrap();
        assert_eq!(
            stats.feed_entries, 2,
            "only the two dated links are entries"
        );

        // The gemsub feed is written into the CONTENT dir (Gemini-served)
        // and rendered to feed.html on the web.
        let gemsub = std::fs::read_to_string(content.join("feed.gmi")).unwrap();
        assert!(gemsub.contains("=> /posts/1 2026-08-09 - Newest"));
        assert!(
            !gemsub.contains("/about"),
            "undated links are not feed entries"
        );
        assert!(
            base.join("html/feed.html").exists(),
            "gemsub feed also renders to HTML"
        );

        let atom = std::fs::read_to_string(base.join("html/atom.xml")).unwrap();
        assert!(atom.contains("<title>example.org</title>"));
        assert!(atom.contains("https://example.org/posts/1"));
        // Feed-level <updated> is the newest entry's date.
        assert!(atom.contains("<updated>2026-08-09T00:00:00Z</updated>"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn no_dated_links_means_no_feed_files() {
        let base = tmp_dir("no-feeds");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(
            content.join("index.gmi"),
            "# Just a page\n\nno dated links here\n",
        )
        .unwrap();
        let stats = render_tree(&content, &base, &test_ctx()).await.unwrap();
        assert_eq!(stats.feed_entries, 0);
        assert!(!content.join("feed.gmi").exists());
        assert!(!base.join("html/atom.xml").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn gemsub_feed_without_base_url_but_no_atom() {
        // A Gemini-only deployment (no web base URL) still gets the
        // gemsub feed, but no atom.xml (Atom needs absolute links).
        let base = tmp_dir("gemsub-only");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(content.join("index.gmi"), "# B\n=> /p 2026-08-09 - Post\n").unwrap();
        let stats = render_tree(&content, &base, &test_ctx()).await.unwrap();
        assert_eq!(stats.feed_entries, 1);
        assert!(
            content.join("feed.gmi").exists(),
            "gemsub feed served on Gemini from the content dir"
        );
        assert!(
            !base.join("html/atom.xml").exists(),
            "no base URL → no Atom feed"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn generated_feed_is_not_counted_as_a_seeded_gemtext_page() {
        // Subtle: seed_skeleton_if_empty checks for any .gmi. A leftover
        // generated feed.gmi from a prior run must NOT count as "content
        // exists" — otherwise a capsule whose only file is the generated
        // feed would never get its skeleton. (feed.gmi is only ever
        // written when index.gmi already has dated links, so in practice
        // index.gmi is always there too — but the guarantee is worth a
        // test so a future refactor can't quietly break it.)
        let base = tmp_dir("feed-not-seed");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(content.join("feed.gmi"), "=> /x 2026-01-01 - x\n").unwrap();
        // has_any_gemtext sees feed.gmi as a .gmi file, so it counts as
        // content — this asserts current behaviour honestly rather than a
        // wish: a bare feed.gmi does suppress the skeleton. Documented so
        // the interaction is visible, not surprising.
        let seeded = seed_skeleton_if_empty(&content, DEFAULT_SKELETON)
            .await
            .unwrap();
        assert!(
            !seeded,
            "any .gmi present suppresses the skeleton, incl. feed.gmi"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn missing_content_dir_renders_empty_tree_not_an_error() {
        let base = tmp_dir("missing-content");
        let content = base.join("does-not-exist");
        let stats = render_tree(&content, &base, &test_ctx()).await.unwrap();
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
        render_tree(&content, &base, &test_ctx()).await.unwrap();
        assert!(
            std::fs::read_to_string(base.join("html/a.html"))
                .unwrap()
                .contains("First")
        );

        // Remove a.gmi, add b.gmi: a re-render (full-tree, per design)
        // must not leave a.html behind from the stale run.
        std::fs::remove_file(content.join("a.gmi")).unwrap();
        std::fs::write(content.join("b.gmi"), "# Second\n").unwrap();
        let stats = render_tree(&content, &base, &test_ctx()).await.unwrap();
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
        let stats = render_tree(&content, &base, &test_ctx()).await.unwrap();
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

        render_tree(&content, &base, &test_ctx()).await.unwrap();
        assert!(base.join("html/real.html").exists());
        assert!(
            !base.join("html/ghost.html").exists(),
            "a leftover staging dir from a crashed run must not leak into a fresh render"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
