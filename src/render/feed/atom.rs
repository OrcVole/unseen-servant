//! Atom feed emission (RFC 4287) for the web surface.
//!
//! Deliberately pure and deterministic: `feed_updated` is a caller-
//! supplied [`time::Date`], not derived from the system clock in here —
//! "now" is a pipeline concern (it knows whether this is a real render or
//! a test), and a function that reaches for the clock itself is harder to
//! test and easier to get subtly wrong. Same reasoning gives every entry
//! its `<updated>` from [`crate::render::metadata::FeedEntry::date`]
//! rather than a file's mtime — the whole point of the metadata pass is
//! that both surfaces agree on the same date for the same content.

use time::Date;

use crate::render::escape_into;
use crate::render::metadata::FeedEntry;

/// Render a complete Atom feed document.
///
/// `feed_id` is a stable, permanent URI identifying the feed itself (not
/// any one entry) — Atom requires it and it must never change once
/// published, so it is the caller's to choose and keep constant, not
/// derived here. `base_url` is prefixed onto any entry URL that isn't
/// already absolute (doesn't contain `://`), so content can link
/// relatively and still produce valid absolute feed URLs.
pub fn render(
    feed_id: &str,
    title: &str,
    base_url: &str,
    feed_updated: Date,
    entries: &[FeedEntry<'_>],
) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n<id>");
    escape_into(&mut out, feed_id);
    out.push_str("</id>\n<title>");
    escape_into(&mut out, title);
    out.push_str("</title>\n<updated>");
    out.push_str(&rfc3339_midnight(feed_updated));
    out.push_str("</updated>\n");

    for entry in entries {
        let link = resolve_url(base_url, entry.url);
        out.push_str("<entry>\n<id>");
        escape_into(&mut out, &link);
        out.push_str("</id>\n<title>");
        escape_into(&mut out, entry.title);
        out.push_str("</title>\n<link href=\"");
        escape_into(&mut out, &link);
        out.push_str("\"/>\n<updated>");
        out.push_str(&rfc3339_midnight(entry.date));
        out.push_str("</updated>\n</entry>\n");
    }

    out.push_str("</feed>\n");
    out
}

/// `base` joined with `url`: `url` returned unchanged if it already looks
/// absolute (contains `://`); otherwise concatenated onto `base`, with
/// exactly one `/` between them regardless of whether either side already
/// has one.
fn resolve_url(base: &str, url: &str) -> String {
    if url.contains("://") {
        return url.to_string();
    }
    let base = base.trim_end_matches('/');
    let url = url.trim_start_matches('/');
    format!("{base}/{url}")
}

fn rfc3339_midnight(d: Date) -> String {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    format!("{}T00:00:00Z", d.format(&format).unwrap_or_default())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, time::Month::try_from(m).unwrap(), d).unwrap()
    }

    #[test]
    fn empty_feed_has_no_entries() {
        let doc = render(
            "gemini://x/atom.xml",
            "My Capsule",
            "gemini://x",
            date(2026, 8, 9),
            &[],
        );
        assert!(doc.starts_with("<?xml"));
        assert!(doc.contains("<title>My Capsule</title>"));
        assert!(!doc.contains("<entry>"));
    }

    #[test]
    fn entry_gets_resolved_absolute_link() {
        let entries = [FeedEntry {
            url: "/posts/1",
            date: date(2026, 8, 9),
            title: "First Post",
        }];
        let doc = render(
            "gemini://x/atom.xml",
            "Capsule",
            "gemini://x",
            date(2026, 8, 9),
            &entries,
        );
        assert!(doc.contains("<link href=\"gemini://x/posts/1\"/>"));
        assert!(doc.contains("<id>gemini://x/posts/1</id>"));
        assert!(doc.contains("<title>First Post</title>"));
        assert!(doc.contains("<updated>2026-08-09T00:00:00Z</updated>"));
    }

    #[test]
    fn already_absolute_entry_url_is_kept_as_is() {
        let entries = [FeedEntry {
            url: "https://elsewhere.example/post",
            date: date(2026, 1, 1),
            title: "External",
        }];
        let doc = render("id", "t", "gemini://x", date(2026, 1, 1), &entries);
        assert!(doc.contains("https://elsewhere.example/post"));
        assert!(!doc.contains("gemini://xhttps"));
    }

    #[test]
    fn resolve_url_handles_slash_variants() {
        assert_eq!(resolve_url("gemini://x", "/a"), "gemini://x/a");
        assert_eq!(resolve_url("gemini://x/", "/a"), "gemini://x/a");
        assert_eq!(resolve_url("gemini://x/", "a"), "gemini://x/a");
        assert_eq!(resolve_url("gemini://x", "a"), "gemini://x/a");
    }

    #[test]
    fn titles_and_ids_are_escaped() {
        let entries = [FeedEntry {
            url: "/x",
            date: date(2026, 1, 1),
            title: "<script>evil()</script>",
        }];
        let doc = render("id", "t", "gemini://x", date(2026, 1, 1), &entries);
        assert!(doc.contains("&lt;script&gt;evil()&lt;/script&gt;"));
        assert!(!doc.contains("<script>evil()"));
    }

    #[test]
    fn feed_updated_is_caller_supplied_not_derived() {
        // No entries at all — the feed-level <updated> still reflects
        // exactly the date passed in, proving it isn't silently defaulted
        // to "now" or the max of an (empty) entry list.
        let doc = render("id", "t", "gemini://x", date(2020, 1, 1), &[]);
        assert!(doc.contains("<updated>2020-01-01T00:00:00Z</updated>"));
    }
}
