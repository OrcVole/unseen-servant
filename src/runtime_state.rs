//! Live server state that survives `SIGHUP` (ADR 0011's "observe over the
//! wire" resource, `/admin/status.gmi`): recent request activity and the
//! last render's stats.
//!
//! Deliberately **not** a field on [`crate::server::Shared`]. `Shared` is
//! fully rebuilt and swapped on every config reload (`main.rs::
//! build_state`), which is exactly right for config and TLS material —
//! but it would be exactly wrong here: an operator reloading config
//! mid-incident should see the activity log fill in *further*, not watch
//! it reset to empty on the very request they're using it to diagnose.
//! So a single [`RuntimeState`] is created once at process start and
//! threaded through the accept loop alongside `Shared`, untouched by
//! reload.
//!
//! Every mutator takes its timestamp as a parameter rather than reading
//! the clock internally, so the ordering and content of what a test
//! observes is exactly what the test put in — the same discipline
//! [`crate::roster::Roster::lookup`] applies to "today" for the rotation
//! window, for the same reason.

use std::collections::VecDeque;
use std::sync::Mutex;

use time::OffsetDateTime;

/// How many recent request outcomes to retain. A fixed, small bound: this
/// is a live diagnostic, not an audit trail requiring completeness —
/// completeness is what the platform's own log collector is for (ADR
/// 0002: usv logs to stderr only, on purpose, and never to a file it
/// would have to rotate itself).
pub const ACTIVITY_CAPACITY: usize = 50;

/// One request's outcome, exactly as already logged. Never holds a query
/// string: `note` is the same pre-redacted line
/// [`crate::server::handle_connection`] already builds for its
/// `tracing::info!` call (recon §8 — queries are sensitive by default),
/// reused rather than re-derived, so "what's safe to show" has one
/// definition, not two that could drift.
#[derive(Debug, Clone)]
pub struct ActivityEntry {
    /// When the response was sent.
    pub at: OffsetDateTime,
    /// The Gemini status code answered.
    pub status: u8,
    /// The same redacted summary line already sent to stderr.
    pub note: String,
}

/// A snapshot of one completed render pass — the same numbers
/// [`crate::render::pipeline::RenderStats`] carries, plus when it
/// happened, so `/admin/status.gmi` can answer "is content actually
/// current" rather than just "did a render ever succeed".
#[derive(Debug, Clone)]
pub struct RenderSnapshot {
    /// When this render completed.
    pub at: OffsetDateTime,
    /// `.gmi` files rendered to HTML.
    pub pages_rendered: usize,
    /// Dated entries emitted into the feeds.
    pub feed_entries: usize,
    /// Pages listed in the generated site map.
    pub mapped_pages: usize,
    /// Whether a web `robots.txt` was mirrored this render.
    pub robots_mirrored: bool,
}

/// Live, reload-surviving server state. Cheap to clone (an `Arc` around
/// two mutexes) — created once in `main.rs::serve`, cloned into every
/// connection task alongside `Shared`.
#[derive(Debug, Default)]
pub struct RuntimeState {
    activity: Mutex<VecDeque<ActivityEntry>>,
    last_render: Mutex<Option<RenderSnapshot>>,
    /// When this `RuntimeState` — and so, in practice, this process —
    /// started. Set once at construction, never mutated.
    started_at: Option<OffsetDateTime>,
}

impl RuntimeState {
    /// A fresh, empty state, stamped with `now` as the start time.
    pub fn new(now: OffsetDateTime) -> RuntimeState {
        RuntimeState {
            activity: Mutex::new(VecDeque::with_capacity(ACTIVITY_CAPACITY)),
            last_render: Mutex::new(None),
            started_at: Some(now),
        }
    }

    /// When this process started serving.
    pub fn started_at(&self) -> Option<OffsetDateTime> {
        self.started_at
    }

    /// Record one request's outcome. Oldest entry drops once the ring is
    /// at capacity — a bounded structure by construction, never an
    /// unbounded log an attacker could use to grow memory without limit.
    pub fn record_request(&self, at: OffsetDateTime, status: u8, note: String) {
        // A poisoned mutex (a prior panic while holding the lock) must not
        // take the whole request path down with it — recording activity is
        // a diagnostic nicety, never allowed to become a new failure mode
        // for serving requests. Recover the data behind the poison and
        // keep going.
        let mut activity = self.activity.lock().unwrap_or_else(|e| e.into_inner());
        if activity.len() >= ACTIVITY_CAPACITY {
            activity.pop_front();
        }
        activity.push_back(ActivityEntry { at, status, note });
    }

    /// The retained activity, oldest first.
    pub fn recent_activity(&self) -> Vec<ActivityEntry> {
        self.activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    /// Record that a render pass completed, replacing whatever snapshot
    /// was there before — only the most recent render matters here.
    pub fn record_render(&self, snapshot: RenderSnapshot) {
        *self.last_render.lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot);
    }

    /// The most recent render's stats, `None` before the first one
    /// completes.
    pub fn last_render(&self) -> Option<RenderSnapshot> {
        self.last_render
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn at(offset_secs: i64) -> OffsetDateTime {
        datetime!(2026-08-10 00:00:00 UTC) + time::Duration::seconds(offset_secs)
    }

    #[test]
    fn a_fresh_state_has_no_activity_or_render_yet() {
        let rt = RuntimeState::new(at(0));
        assert!(rt.recent_activity().is_empty());
        assert!(rt.last_render().is_none());
        assert_eq!(rt.started_at(), Some(at(0)));
    }

    #[test]
    fn recorded_requests_come_back_in_order() {
        let rt = RuntimeState::new(at(0));
        rt.record_request(at(1), 20, "a".to_string());
        rt.record_request(at(2), 51, "b".to_string());
        let entries = rt.recent_activity();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].status, 20);
        assert_eq!(entries[0].note, "a");
        assert_eq!(entries[1].status, 51);
        assert_eq!(entries[1].note, "b");
    }

    #[test]
    fn the_activity_ring_never_exceeds_its_capacity() {
        let rt = RuntimeState::new(at(0));
        for i in 0..(ACTIVITY_CAPACITY + 10) {
            rt.record_request(at(i as i64), 20, format!("entry {i}"));
        }
        let entries = rt.recent_activity();
        assert_eq!(entries.len(), ACTIVITY_CAPACITY);
    }

    #[test]
    fn the_ring_drops_the_oldest_entry_first() {
        let rt = RuntimeState::new(at(0));
        for i in 0..(ACTIVITY_CAPACITY + 1) {
            rt.record_request(at(i as i64), 20, format!("entry {i}"));
        }
        let entries = rt.recent_activity();
        // "entry 0" was the very first pushed and must be the one dropped.
        assert!(!entries.iter().any(|e| e.note == "entry 0"));
        assert!(entries.iter().any(|e| e.note == "entry 1"));
        assert!(
            entries
                .iter()
                .any(|e| e.note == format!("entry {ACTIVITY_CAPACITY}"))
        );
    }

    #[test]
    fn recording_a_render_replaces_the_previous_snapshot() {
        let rt = RuntimeState::new(at(0));
        rt.record_render(RenderSnapshot {
            at: at(1),
            pages_rendered: 1,
            feed_entries: 0,
            mapped_pages: 1,
            robots_mirrored: false,
        });
        rt.record_render(RenderSnapshot {
            at: at(2),
            pages_rendered: 5,
            feed_entries: 2,
            mapped_pages: 5,
            robots_mirrored: true,
        });
        let snap = rt.last_render().unwrap();
        assert_eq!(snap.at, at(2));
        assert_eq!(snap.pages_rendered, 5);
        assert!(snap.robots_mirrored);
    }
}
