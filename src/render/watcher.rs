//! Content-tree watcher: fs events → debounce → [`super::pipeline::render_tree`].
//!
//! Resolves design brief §5.2 (`docs/notes/c3-render-design-brief.md`):
//! debounce is **global**, not per-file — an edit storm across many files
//! (a bulk content sync, a git checkout) coalesces into one rebuild pass
//! once events go quiet for `debounce`, rather than one rebuild per file.
//! A window of 300ms is the default; callers may choose otherwise (a
//! config knob, if wanted later, is the pipeline/CLI's decision, not
//! this module's).
//!
//! Kept separate from [`super::pipeline`] the way `server.rs` is kept
//! separate from `protocol/`: this module knows about `notify` and
//! `tokio` tasks; the pipeline knows nothing about either.

use std::path::PathBuf;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tokio::sync::{mpsc, watch};

use super::pipeline::{RenderContext, RenderStats, render_tree};

/// The default debounce window (design brief §5.2's suggested starting
/// value).
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(300);

/// Why the watcher couldn't start or run.
#[derive(Debug)]
pub enum WatchError {
    /// `notify` failed to install the OS-level watch (bad path, platform
    /// limit on inotify instances, etc.).
    Notify(notify::Error),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::Notify(e) => write!(f, "content watcher: {e}"),
        }
    }
}

impl std::error::Error for WatchError {}

impl From<notify::Error> for WatchError {
    fn from(e: notify::Error) -> Self {
        WatchError::Notify(e)
    }
}

/// Watch `content_dir` for changes and re-render into `state_dir` on a
/// debounced quiet period. Runs until the returned watcher is dropped or
/// an unrecoverable error occurs; each successful render is reported via
/// `on_rendered` (typically just a `tracing::info!` call) so the caller
/// controls logging rather than this module owning a policy on it.
///
/// `ctx` is a `watch::Receiver`, not an owned value: a SIGHUP reload can
/// change `server.advertised_host`, the primary hostname, `http_listen`,
/// the theme, or the language, and every edit-triggered re-render after
/// that must use the new context — never a value frozen at watch-startup
/// time. The sender lives with whoever owns the reload path (main.rs);
/// this function only reads the latest value, once, right before each
/// render fires.
///
/// This function does not return under normal operation — it's meant to
/// be spawned as its own tokio task. It returns `Err` only if the
/// initial watch setup fails; a render error mid-loop is logged via
/// `on_rendered` (as `Err`) and watching continues, since one bad render
/// (a transient I/O error, a permissions hiccup) should not permanently
/// stop the capsule from picking up the next successful edit.
pub async fn watch(
    content_dir: PathBuf,
    state_dir: PathBuf,
    ctx: watch::Receiver<RenderContext>,
    debounce: Duration,
    on_rendered: impl Fn(std::io::Result<RenderStats>),
) -> Result<(), WatchError> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        // Access events (open/read/close-without-write) are not content
        // changes — and critically, `render_tree` itself *reads* every
        // `.gmi` file it renders, which is inside this same watched
        // directory. Forwarding Access events would make every render
        // generate its own follow-up event, self-triggering an endless
        // echo of one extra render after every real one. Only
        // Create/Modify/Remove/rename-shaped events represent an actual
        // content change worth debouncing toward a re-render.
        if let Ok(event) = &res {
            if matches!(
                event.kind,
                notify::EventKind::Access(_) | notify::EventKind::Other
            ) {
                return;
            }
            // `render_tree` writes generated files (the gemsub feed and
            // the site map) back INTO the watched content directory, so
            // Gemini can serve them. Left unfiltered, those writes would
            // trigger the very render that produced them — an endless
            // loop. An event touching only reserved generated names is
            // always self-generated, so drop it. (A real edit arrives as
            // its own separate event.)
            if !event.paths.is_empty()
                && event.paths.iter().all(|p| {
                    matches!(
                        p.file_name().and_then(|n| n.to_str()),
                        Some(super::pipeline::GENERATED_FEED_NAME)
                            | Some(super::pipeline::GENERATED_MAP_NAME)
                    )
                })
            {
                return;
            }
        }
        // The channel receiver only outlives this closure as long as
        // `watch` is still running; a send failure just means we're
        // shutting down, not an error worth surfacing.
        let _ = tx.send(res);
    })?;
    watcher.watch(&content_dir, RecursiveMode::Recursive)?;

    loop {
        // Block for the first event in a quiet period.
        match rx.recv().await {
            Some(Ok(_)) => {}
            Some(Err(e)) => {
                tracing::warn!(error = %e, "content watcher event error");
                continue;
            }
            None => return Ok(()), // sender dropped: watcher was dropped, clean shutdown
        }
        // Drain further events until `debounce` passes with no new ones.
        loop {
            match tokio::time::timeout(debounce, rx.recv()).await {
                Ok(Some(_)) => continue,
                Ok(None) => return Ok(()),
                Err(_elapsed) => break, // quiet period reached
            }
        }
        // Read the latest context right before rendering, not once at
        // watch-startup — a SIGHUP reload in between must be picked up by
        // the very next edit-triggered render, not the next restart.
        let ctx_now = ctx.borrow().clone();
        on_rendered(render_tree(&content_dir, &state_dir, &ctx_now).await);
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fixed context wrapped in a receiver whose sender is immediately
    /// dropped — fine for `.borrow()`, which every test here only needs
    /// once per render; none of these tests exercise a live context
    /// update (that's `tests/smoke.rs`'s
    /// `sighup_reload_reaches_the_watcher_not_just_the_next_restart`,
    /// which needs a real running process to prove end to end).
    fn ctx_channel() -> watch::Receiver<RenderContext> {
        watch::channel(RenderContext::plain("body{}")).1
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("usv-watcher-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[tokio::test]
    async fn a_single_file_write_triggers_exactly_one_render() {
        let base = tmp_dir("single-write");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();

        let render_count = Arc::new(AtomicUsize::new(0));
        let count_for_watch = render_count.clone();
        let watch_content = content.clone();
        let watch_state = base.clone();
        let handle = tokio::spawn(async move {
            watch(
                watch_content,
                watch_state,
                ctx_channel(),
                Duration::from_millis(300),
                move |result| {
                    result.expect("render should succeed");
                    count_for_watch.fetch_add(1, Ordering::SeqCst);
                },
            )
            .await
        });

        // Give the watcher time to install before writing.
        tokio::time::sleep(Duration::from_millis(200)).await;
        std::fs::write(content.join("a.gmi"), "# Hello\n").unwrap();

        // Wait comfortably past the debounce window for the render to land.
        tokio::time::sleep(Duration::from_millis(1000)).await;
        handle.abort();

        assert_eq!(render_count.load(Ordering::SeqCst), 1);
        assert!(base.join("html/a.html").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn a_burst_of_writes_coalesces_into_one_render() {
        let base = tmp_dir("burst");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();

        let render_count = Arc::new(AtomicUsize::new(0));
        let count_for_watch = render_count.clone();
        let watch_content = content.clone();
        let watch_state = base.clone();
        let handle = tokio::spawn(async move {
            watch(
                watch_content,
                watch_state,
                ctx_channel(),
                Duration::from_millis(200),
                move |result| {
                    result.expect("render should succeed");
                    count_for_watch.fetch_add(1, Ordering::SeqCst);
                },
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(200)).await;
        // A burst of writes, each well inside the 200ms debounce window of
        // the previous one, must coalesce into a single render pass —
        // this is the "edit storm" property the C3 exit gate names.
        for i in 0..5 {
            std::fs::write(content.join(format!("{i}.gmi")), format!("# Page {i}\n")).unwrap();
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        tokio::time::sleep(Duration::from_millis(900)).await;
        handle.abort();

        assert_eq!(
            render_count.load(Ordering::SeqCst),
            1,
            "a burst of rapid writes must coalesce into exactly one render"
        );
        for i in 0..5 {
            assert!(base.join(format!("html/{i}.html")).exists());
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dated_index_does_not_cause_a_feed_write_render_loop() {
        // The generated feed.gmi is written INTO the watched content dir.
        // If the watcher didn't ignore it, that write would trigger a
        // render, which writes it again — forever. This proves the loop
        // is broken: one edit to a dated index settles to a fixed number
        // of renders and stops, rather than climbing without bound.
        let base = tmp_dir("feed-loop");
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();

        let render_count = Arc::new(AtomicUsize::new(0));
        let count_for_watch = render_count.clone();
        let watch_content = content.clone();
        let watch_state = base.clone();
        let handle = tokio::spawn(async move {
            watch(
                watch_content,
                watch_state,
                ctx_channel(),
                Duration::from_millis(150),
                move |result| {
                    result.expect("render should succeed");
                    count_for_watch.fetch_add(1, Ordering::SeqCst);
                },
            )
            .await
        });

        tokio::time::sleep(Duration::from_millis(200)).await;
        // One edit: an index with a dated link, which makes render write
        // content/feed.gmi. Without the watcher's feed-ignore, this would
        // loop; with it, it settles.
        std::fs::write(content.join("index.gmi"), "# B\n=> /p 2026-08-09 - Post\n").unwrap();

        // Wait several debounce windows — a loop would rack up renders the
        // whole time; a broken loop stops at one.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        handle.abort();

        let renders = render_count.load(Ordering::SeqCst);
        assert_eq!(
            renders, 1,
            "one dated-index edit must cause exactly one render, not a feed-write loop (got {renders})"
        );
        assert!(
            content.join("feed.gmi").exists(),
            "the feed was still generated"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
