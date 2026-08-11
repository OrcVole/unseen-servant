//! The render pipeline: walk a content tree, render every `.gmi` file to
//! HTML, swap the result into place atomically. Resolves design brief §5.4
//! (`docs/internal/notes/c3-render-design-brief.md`): **full-tree rebuild every
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

use super::sitemap::PageEntry;
use super::{gemtext, html, llms, markdown, robots, sitemap, skeleton};

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
    /// BCP 47 language tag for the capsule (ADR 0010) — becomes the HTML
    /// `lang` attribute on every rendered page.
    pub lang: String,
    /// Cleartext output (ADR 0012 §4). `None` — the default — means no
    /// cleartext tree is built at all, which is what an operator who
    /// enabled none of these protocols should get: nothing to leak.
    pub cleartext: Option<CleartextRender>,
}

/// What the cleartext targets need, and what they must leave out.
///
/// Two trees come out of this, because the protocols want different
/// bytes for the same page:
///
/// * `gopher/` — rendered **menus**, which only gopher understands;
/// * `cleartext/` — the **gemtext** sources, which Spartan and Nex serve
///   as-is (Spartan's document format *is* gemtext, and Nex reuses
///   gemtext's `=> ` link convention).
///
/// Found by pointing Spartan and Nex at the gopher tree and watching a
/// real client receive tab-delimited menu lines where gemtext belonged.
/// They cannot simply read the content directory instead, because that
/// still holds the gated pages — hence a second gate-filtered tree
/// rather than a fallback.
#[derive(Debug, Clone)]
pub struct CleartextRender {
    /// Paths gated behind a client certificate, which no cleartext tree
    /// may contain (ADR 0012 §6). Applied at build time, so no request
    /// path can serve what was never written.
    pub gate: super::cleartext::Gate,
    /// Host and advertised port for gopher menus, when gopher is on.
    /// `None` means Spartan and/or Nex are enabled but gopher is not —
    /// then only the gemtext tree is built.
    pub gopher: Option<super::gopher::Context>,
}

impl RenderContext {
    /// A minimal context for tests and the no-feed case: a stylesheet, no
    /// base URL (so no Atom feed is produced), a default title.
    pub fn plain(theme_css: impl Into<String>) -> RenderContext {
        RenderContext {
            theme_css: theme_css.into(),
            web_base_url: String::new(),
            capsule_title: "Unseen Servant".to_string(),
            lang: "en".to_string(),
            cleartext: None,
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
    /// Number of pages listed in the generated site map (ADR 0010).
    pub mapped_pages: usize,
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

/// The reserved filename usv generates for the Gemini-side site map
/// (ADR 0010). Like the feed, it is written into the *content* directory
/// so Gemini serves it directly, so the watcher must ignore it too.
pub const GENERATED_MAP_NAME: &str = "map.gmi";

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
    // The cleartext tree gets its own staging root and its own swap: it
    // must never live inside the web tree, which the HTTP surface serves
    // wholesale (see gopher_output_path).
    let gopher_staging = state_dir.join("gopher.tmp");
    let gemtext_staging = state_dir.join("cleartext.tmp");
    let live = state_dir.join("html");
    let old = state_dir.join("html.old");

    // A staging dir surviving from a crashed prior run must not leak
    // stale pages into this run's output.
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging).await?;
    let _ = tokio::fs::remove_dir_all(&gopher_staging).await;
    let _ = tokio::fs::remove_dir_all(&gemtext_staging).await;
    if ctx.cleartext.is_some() {
        tokio::fs::create_dir_all(&gopher_staging).await?;
        tokio::fs::create_dir_all(&gemtext_staging).await?;
    }

    // The gemsub feed is Gemini-native gemtext, so it is written into the
    // *content* directory (served on Gemini directly) BEFORE the walk, so
    // the walk also renders it to `feed.html` for the web. Done first so
    // it is part of this same render pass, not the next one.
    let feed_entries = write_gemsub_feed(content_dir).await?;
    let mut stats = RenderStats {
        feed_entries,
        ..Default::default()
    };

    let mut pages: Vec<PageEntry> = Vec::new();
    if tokio::fs::metadata(content_dir).await.is_ok() {
        render_dir(
            content_dir,
            content_dir,
            &staging,
            &ctx.lang,
            &mut stats,
            &mut pages,
            ctx.cleartext
                .as_ref()
                .map(|c| (c, gopher_staging.as_path(), gemtext_staging.as_path())),
        )
        .await?;
    }

    // Site map (ADR 0010), from the walk just completed. The gemtext map
    // goes into the content dir so Gemini serves it, and is rendered to
    // `map.html` here rather than on the next pass — the same treatment
    // the gemsub feed gets. It lists every page except itself.
    write_site_map(content_dir, &staging, ctx, &pages, &mut stats).await?;

    // The bundled stylesheet every rendered page links to. Written into
    // the tree rather than served from memory so the output directory is
    // a complete, portable static site — the same property `usv export`
    // (C5) and the OnionShare recipe depend on.
    tokio::fs::write(staging.join("style.css"), &ctx.theme_css).await?;

    // The Atom feed is web-only (XML, absolute links), so it goes into the
    // rendered HTML tree, not the content dir. Same entries as the gemsub
    // feed above — ADR 0004's one-source-both-surfaces guarantee.
    write_atom_feed(content_dir, &staging, ctx).await?;

    // Web robots.txt. If the operator wrote a Gemini robots.txt with rules,
    // translate it (see `robots.rs` for why this is a translation, not a
    // copy). Otherwise write the permissive-by-doctrine default (ADR 0011):
    // an explicit open-access posture pointing crawlers at the sitemap,
    // rather than the silence a missing file would be.
    let content_robots = tokio::fs::read_to_string(content_dir.join("robots.txt"))
        .await
        .ok();
    match content_robots.as_deref().and_then(robots::to_web_robots) {
        Some(web_robots) => {
            tokio::fs::write(staging.join("robots.txt"), web_robots).await?;
            stats.robots_mirrored = true;
        }
        None => {
            tokio::fs::write(
                staging.join("robots.txt"),
                robots::default_web_robots(&ctx.web_base_url),
            )
            .await?;
        }
    }

    let _ = tokio::fs::remove_dir_all(&old).await;
    if tokio::fs::metadata(&live).await.is_ok() {
        tokio::fs::rename(&live, &old).await?;
    }
    tokio::fs::rename(&staging, &live).await?;
    let _ = tokio::fs::remove_dir_all(&old).await;

    // Same swap discipline for the cleartext tree, on its own roots: a
    // reader never sees a half-written gopherspace either.
    if ctx.cleartext.is_some() {
        for (staging, name) in [(&gopher_staging, "gopher"), (&gemtext_staging, "cleartext")] {
            let live = state_dir.join(name);
            let old = state_dir.join(format!("{name}.old"));
            let _ = tokio::fs::remove_dir_all(&old).await;
            if tokio::fs::metadata(&live).await.is_ok() {
                tokio::fs::rename(&live, &old).await?;
            }
            tokio::fs::rename(staging, &live).await?;
            let _ = tokio::fs::remove_dir_all(&old).await;
        }
    }

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

/// Write the site map for both surfaces (ADR 0010): `map.gmi` into the
/// content directory (Gemini-served, and rendered to `map.html` here in
/// this same pass) and `sitemap.xml` into the staging tree. The map lists
/// every page *except itself* — a self-reference is noise, and excluding
/// it also keeps the map's own content stable across renders.
async fn write_site_map(
    content_dir: &Path,
    staging: &Path,
    ctx: &RenderContext,
    pages: &[PageEntry],
    stats: &mut RenderStats,
) -> std::io::Result<()> {
    let listed: Vec<PageEntry> = pages
        .iter()
        .filter(|p| p.gemini_path != format!("/{GENERATED_MAP_NAME}"))
        .cloned()
        .collect();
    if listed.is_empty() {
        return Ok(());
    }
    stats.mapped_pages = listed.len();

    let gemtext_map = sitemap::render_gemtext(&listed);
    tokio::fs::write(content_dir.join(GENERATED_MAP_NAME), &gemtext_map).await?;

    // Render the map to HTML (and Markdown) now rather than waiting for the
    // next pass, so both surfaces gain it in the same render.
    let lines = gemtext::parse(&gemtext_map);
    let title = gemtext::extract_title(&lines, Path::new(GENERATED_MAP_NAME));
    tokio::fs::write(
        staging.join("map.html"),
        html::render_document(&lines, &title, &ctx.lang),
    )
    .await?;
    tokio::fs::write(staging.join("map.md"), markdown::render(&lines)).await?;

    if let Some(xml) = sitemap::render_xml(&listed, &ctx.web_base_url) {
        tokio::fs::write(staging.join("sitemap.xml"), xml).await?;
    }

    // `/llms.txt` (ADR 0011): the same inventory re-serialized into the
    // llms.txt convention, at the web-root path HTTP agents look for. Web
    // surface only — the Gemini side already has `map.gmi`.
    let llms_txt = llms::render(&listed, &ctx.capsule_title, &ctx.web_base_url);
    tokio::fs::write(staging.join(llms::LLMS_TXT_NAME), llms_txt).await?;
    Ok(())
}

/// Recursive directory walk. Async fns can't recurse directly (the
/// future's size would be infinite); boxing is the standard pattern.
/// Collects a [`PageEntry`] per rendered page as it goes, so the site
/// map (ADR 0010) comes out of the walk usv already performs rather than
/// costing a second traversal.
fn render_dir<'a>(
    root: &'a Path,
    dir: &'a Path,
    staging_root: &'a Path,
    lang: &'a str,
    stats: &'a mut RenderStats,
    pages: &'a mut Vec<PageEntry>,
    cleartext: Option<(&'a CleartextRender, &'a Path, &'a Path)>,
) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                render_dir(root, &path, staging_root, lang, stats, pages, cleartext).await?;
            } else if file_type.is_file() && is_gemtext(&path) {
                let title = render_page(root, &path, staging_root, lang, cleartext).await?;
                stats.pages_rendered += 1;
                let relative = path.strip_prefix(root).unwrap_or(&path);
                pages.push(page_entry(relative, title));
            } else if file_type.is_file() {
                // Non-gemtext assets. The Gemini surface serves these from
                // the content directory directly, but a cleartext tree has
                // no such fallback by design — its whole safety property is
                // that it contains *only* what may be served (ADR 0012 §6),
                // so anything reachable from a menu has to be copied in,
                // subject to the same gate.
                if let Some((target, gopher_root, gemtext_root)) = cleartext {
                    let relative = path.strip_prefix(root).unwrap_or(&path);
                    let selector = gopher_selector(relative);
                    if !target.gate.excludes(&selector) {
                        // Both trees: a link to an asset has to resolve
                        // whichever cleartext protocol followed it.
                        let mut dests = vec![gemtext_root.join(relative)];
                        if target.gopher.is_some() {
                            dests.push(gopher_output_path(gopher_root, relative));
                        }
                        for dest in dests {
                            if let Some(parent) = dest.parent() {
                                tokio::fs::create_dir_all(parent).await?;
                            }
                            tokio::fs::copy(&path, &dest).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    })
}

fn is_gemtext(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("gmi")
}

/// Build a site-map entry for a content-relative path. Paths are
/// normalised to forward slashes with a leading `/` so they are URLs
/// rather than platform paths.
fn page_entry(relative: &Path, title: String) -> PageEntry {
    let as_url = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/");
    let web = as_url.strip_suffix(".gmi").unwrap_or(&as_url).to_string();
    PageEntry {
        gemini_path: format!("/{as_url}"),
        web_path: format!("/{web}.html"),
        title,
    }
}

/// Render one `.gmi` file into its mirrored `.html` path under
/// `staging_root`, preserving the content tree's own directory structure.
/// Returns the page's title so the caller can build the site map without
/// re-reading and re-parsing the file.
async fn render_page(
    root: &Path,
    path: &Path,
    staging_root: &Path,
    lang: &str,
    cleartext: Option<(&CleartextRender, &Path, &Path)>,
) -> std::io::Result<String> {
    let text = tokio::fs::read_to_string(path).await?;
    let lines = gemtext::parse(&text);
    let title = gemtext::extract_title(&lines, path);
    let doc = html::render_document(&lines, &title, lang);

    let relative = path.strip_prefix(root).unwrap_or(path);
    let out_path = html_output_path(staging_root, relative);
    if let Some(parent) = out_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&out_path, doc).await?;

    // The Markdown serialization (ADR 0011): `page.md` beside `page.html`,
    // the clean form the HTTP agent audience prefers over scraping HTML.
    // Same source, distinct address — an addressable resource, not
    // user-agent-switched content.
    tokio::fs::write(out_path.with_extension("md"), markdown::render(&lines)).await?;

    // The cleartext targets, last and conditionally (ADR 0012 §4/§6).
    if let Some((target, gopher_root, gemtext_root)) = cleartext {
        let selector = gopher_selector(relative);
        // The wall: a gated page is never written into any cleartext
        // tree. Nothing downstream has to remember to check.
        if !target.gate.excludes(&selector) {
            // Gemtext, for Spartan and Nex — their document format is
            // gemtext, so this is the source verbatim.
            let gem_path = gemtext_root.join(relative);
            if let Some(parent) = gem_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&gem_path, &text).await?;

            // Menus, for gopher, only when gopher itself is on.
            if let Some(gctx) = &target.gopher {
                let page_dir = selector.rsplit_once('/').map_or("/", |(dir, _)| dir);
                let page_dir = if page_dir.is_empty() { "/" } else { page_dir };
                let menu = super::gopher::render_menu(&lines, &title, page_dir, gctx, &target.gate);
                let gopher_path = gopher_output_path(gopher_root, relative);
                if let Some(parent) = gopher_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(&gopher_path, menu).await?;
            }
        }
    }
    Ok(title)
}

/// The selector a content-tree-relative path is reachable at.
fn gopher_selector(relative: &Path) -> String {
    let mut s = String::from("/");
    s.push_str(&relative.to_string_lossy().replace('\\', "/"));
    s
}

/// Where a page's gopher menu is written, inside the gopher tree's own
/// staging root, mirroring the content tree's structure.
///
/// A **sibling** of the web tree, never inside it: the HTTP surface
/// serves everything under its own root, so a gopher subtree living
/// there would quietly become web-reachable at `/gopher/...`. Separate
/// roots also make "everything the cleartext listener may serve" one
/// directory, so ADR 0012 §6's exclusion is visible as an absent file
/// rather than something a serving path has to re-derive.
fn gopher_output_path(gopher_root: &Path, relative: &Path) -> PathBuf {
    gopher_root.join(relative)
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
            lang: "en".to_string(),
            cleartext: None,
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
        // b.gmi plus the generated map.gmi from the first pass, which is
        // itself a real page and renders to map.html.
        assert_eq!(stats.pages_rendered, 2);
        assert!(
            !base.join("html/a.html").exists(),
            "stale output must not survive a re-render"
        );
        assert!(base.join("html/b.html").exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn the_gopher_tree_is_written_and_gated_pages_are_absent_from_it() {
        // The end-to-end proof of ADR 0012 §6: the wall is applied where
        // the tree is BUILT, so a gated page has no file at all — there
        // is nothing for any request path to serve by mistake.
        let base = tmp_dir("gopher-wall");
        let content = base.join("content");
        std::fs::create_dir_all(content.join("private")).unwrap();
        std::fs::write(
            content.join("index.gmi"),
            "# Home\n\n=> private/s.gmi Secret\n",
        )
        .unwrap();
        std::fs::write(
            content.join("private/s.gmi"),
            "# Secret\n\nNot for gopher.\n",
        )
        .unwrap();

        let mut gate_host = crate::config::HostConfig {
            name: "example.org".into(),
            docroot: content.clone(),
            redirects: Vec::new(),
            cert_zones: vec![crate::handler::cert_zone::Zone {
                path_prefix: "/private/".into(),
                allowed_fingerprints: Vec::new(),
            }],
            titan_zones: Vec::new(),
        };
        gate_host.docroot = content.clone();

        let ctx = RenderContext {
            theme_css: "body{}".to_string(),
            web_base_url: "https://example.org".to_string(),
            capsule_title: "example.org".to_string(),
            lang: "en".to_string(),
            cleartext: Some(CleartextRender {
                gate: super::super::cleartext::Gate::for_host(&gate_host),
                gopher: Some(super::super::gopher::Context {
                    host: "example.org".into(),
                    port: 70,
                }),
            }),
        };
        render_tree(&content, &base, &ctx).await.unwrap();

        // The public page is there, and is a menu.
        let home = std::fs::read_to_string(base.join("gopher/index.gmi")).unwrap();
        assert!(home.contains("iHome\t"), "{home}");
        assert!(home.ends_with(".\r\n"));

        // Spartan and Nex read GEMTEXT, not menus: the cleartext tree
        // must carry the source, byte for byte.
        let gem = std::fs::read_to_string(base.join("cleartext/index.gmi")).unwrap();
        assert!(gem.starts_with("# Home"), "not gemtext: {gem:?}");
        assert!(!gem.contains('\t'), "menu leaked into the gemtext tree");

        // The gated page was never written into EITHER cleartext tree.
        assert!(
            !base.join("gopher/private/s.gmi").exists(),
            "a cert-zoned page must not exist in the gopher tree"
        );
        assert!(
            !base.join("cleartext/private/s.gmi").exists(),
            "a cert-zoned page must not exist in the gemtext tree"
        );
        // ...while still existing on the surfaces that can authenticate.
        assert!(base.join("html/private/s.html").exists());
    }

    #[tokio::test]
    async fn assets_are_copied_into_the_gopher_tree_but_gated_ones_are_not() {
        // A cleartext tree has no content-dir fallback by design, so a
        // menu link to a non-gemtext file only works if it was copied in
        // — and a gated asset must not be, for the same reason a gated
        // page is not.
        let base = tmp_dir("gopher-assets");
        let content = base.join("content");
        std::fs::create_dir_all(content.join("private")).unwrap();
        std::fs::write(content.join("index.gmi"), "# Home\n").unwrap();
        std::fs::write(content.join("readme.txt"), "plain\n.dotted\n").unwrap();
        std::fs::write(content.join("private/secret.txt"), "no\n").unwrap();

        let host = crate::config::HostConfig {
            name: "example.org".into(),
            docroot: content.clone(),
            redirects: Vec::new(),
            cert_zones: vec![crate::handler::cert_zone::Zone {
                path_prefix: "/private/".into(),
                allowed_fingerprints: Vec::new(),
            }],
            titan_zones: Vec::new(),
        };
        let ctx = RenderContext {
            theme_css: "body{}".to_string(),
            web_base_url: String::new(),
            capsule_title: "example.org".to_string(),
            lang: "en".to_string(),
            cleartext: Some(CleartextRender {
                gate: super::super::cleartext::Gate::for_host(&host),
                gopher: Some(super::super::gopher::Context {
                    host: "example.org".into(),
                    port: 70,
                }),
            }),
        };
        render_tree(&content, &base, &ctx).await.unwrap();

        assert!(base.join("gopher/readme.txt").exists(), "asset not copied");
        assert!(
            base.join("cleartext/readme.txt").exists(),
            "asset missing from the gemtext tree Spartan and Nex read"
        );
        assert!(
            !base.join("gopher/private/secret.txt").exists(),
            "a gated asset must not reach the cleartext tree"
        );
        assert!(
            !base.join("cleartext/private/secret.txt").exists(),
            "a gated asset must not reach the gemtext tree either"
        );
    }

    #[tokio::test]
    async fn no_gopher_tree_is_written_when_the_target_is_off() {
        // A capsule that never enabled gopher has no cleartext tree at
        // all, so there is nothing to leak even by misconfiguration.
        let base = tmp_dir("gopher-off");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(content.join("index.gmi"), "# Home\n").unwrap();
        let ctx = RenderContext::plain("body{}");
        render_tree(&content, &base, &ctx).await.unwrap();
        assert!(!base.join("gopher").exists());
    }

    #[tokio::test]
    async fn packaging_tier_files_land_on_the_web_surface() {
        // ADR 0011: every page gets a .md sibling; the web root gets
        // /llms.txt and a permissive robots.txt; the site map is emitted
        // in .md too. All are re-serializations of the one content tree.
        let base = tmp_dir("packaging-tier");
        let content = base.join("content");
        std::fs::create_dir_all(content.join("blog")).unwrap();
        std::fs::write(
            content.join("index.gmi"),
            "# Home\n\n=> /blog/p Read a post\n",
        )
        .unwrap();
        std::fs::write(content.join("blog/p.gmi"), "# A Post\n\nBody prose.\n").unwrap();

        let ctx = RenderContext {
            theme_css: "body{}".to_string(),
            web_base_url: "https://example.org".to_string(),
            capsule_title: "Example".to_string(),
            lang: "en".to_string(),
            cleartext: None,
        };
        render_tree(&content, &base, &ctx).await.unwrap();

        // Per-page Markdown sibling.
        let post_md = std::fs::read_to_string(base.join("html/blog/p.md")).unwrap();
        assert_eq!(post_md, "# A Post\n\nBody prose.\n");

        // /llms.txt from the inventory, with absolute links from the base.
        // It links the `.md` siblings written just above, not the `.html`
        // pages: the index exists so a reader need not parse markup.
        let llms = std::fs::read_to_string(base.join("html/llms.txt")).unwrap();
        assert!(llms.starts_with("# Example\n"));
        assert!(llms.contains("(https://example.org/index.md)"));
        assert!(llms.contains("(https://example.org/blog/p.md)"));
        assert!(!llms.contains(".html"));

        // Permissive-by-doctrine robots.txt (no operator robots present).
        let robots = std::fs::read_to_string(base.join("html/robots.txt")).unwrap();
        assert!(robots.contains("Allow: /"));
        assert!(robots.contains("Sitemap: https://example.org/sitemap.xml"));

        // The site map itself is serialized to Markdown too.
        assert!(base.join("html/map.md").exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn an_operator_robots_still_wins_over_the_permissive_default() {
        // A capsule that disallows a path must get its rule translated,
        // not the blanket-allow default.
        let base = tmp_dir("operator-robots");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(content.join("index.gmi"), "# Home\n").unwrap();
        std::fs::write(
            content.join("robots.txt"),
            "User-agent: indexer\nDisallow: /private\n",
        )
        .unwrap();
        let stats = render_tree(&content, &base, &test_ctx()).await.unwrap();
        assert!(stats.robots_mirrored, "operator rules were translated");
        let robots = std::fs::read_to_string(base.join("html/robots.txt")).unwrap();
        assert!(robots.contains("Disallow: /private"));
        assert!(!robots.contains("Allow: /"), "not the permissive default");
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
