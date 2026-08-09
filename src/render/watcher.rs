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
use tokio::sync::mpsc;

use super::pipeline::{RenderStats, render_tree};

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
/// This function does not return under normal operation — it's meant to
/// be spawned as its own tokio task. It returns `Err` only if the
/// initial watch setup fails; a render error mid-loop is logged via
/// `on_rendered` (as `Err`) and watching continues, since one bad render
/// (a transient I/O error, a permissions hiccup) should not permanently
/// stop the capsule from picking up the next successful edit.
pub async fn watch(
    content_dir: PathBuf,
    state_dir: PathBuf,
    theme_css: String,
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
        if let Ok(event) = &res
            && matches!(
                event.kind,
                notify::EventKind::Access(_) | notify::EventKind::Other
            )
        {
            return;
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
        on_rendered(render_tree(&content_dir, &state_dir, &theme_css).await);
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
                "body{}".to_string(),
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
                "body{}".to_string(),
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
}
