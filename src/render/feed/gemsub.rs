//! Gemsub feed emission: gemtext link lines in the subscription companion
//! spec's convention (`=> URL YYYY-MM-DD - title`), for the Gemini
//! surface. This is the exact inverse of
//! [`crate::render::metadata::extract_feed_entries`] — round-tripping a
//! rendered gemsub block back through `gemtext::parse` +
//! `extract_feed_entries` reproduces the same entries, proven by a test
//! below, since content authors and CAPCOM/Antenna-style aggregators
//! alike need one unambiguous convention rather than two implementations
//! that happen to usually agree.

use crate::render::metadata::FeedEntry;

/// Render entries as gemtext link lines, one per entry, in the order
/// given (the caller decides sort order — typically newest first, but
/// that's a pipeline policy, not this function's).
pub fn render(entries: &[FeedEntry<'_>]) -> String {
    let mut out = String::new();
    let format = time::macros::format_description!("[year]-[month]-[day]");
    for entry in entries {
        out.push_str("=> ");
        out.push_str(entry.url);
        out.push(' ');
        out.push_str(&entry.date.format(&format).unwrap_or_default());
        out.push_str(" - ");
        out.push_str(entry.title);
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;
    use crate::render::{gemtext, metadata};
    use time::Date;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, time::Month::try_from(m).unwrap(), d).unwrap()
    }

    #[test]
    fn empty_entries_render_empty_string() {
        assert_eq!(render(&[]), "");
    }

    #[test]
    fn single_entry_matches_the_subscription_convention() {
        let entries = [FeedEntry {
            url: "/posts/1",
            date: date(2026, 8, 9),
            title: "First Post",
        }];
        assert_eq!(render(&entries), "=> /posts/1 2026-08-09 - First Post\n");
    }

    #[test]
    fn multiple_entries_one_line_each_in_given_order() {
        let entries = [
            FeedEntry {
                url: "/a",
                date: date(2026, 1, 1),
                title: "A",
            },
            FeedEntry {
                url: "/b",
                date: date(2026, 2, 2),
                title: "B",
            },
        ];
        assert_eq!(
            render(&entries),
            "=> /a 2026-01-01 - A\n=> /b 2026-02-02 - B\n"
        );
    }

    #[test]
    fn round_trips_through_the_real_parser() {
        let entries = [
            FeedEntry {
                url: "/posts/1",
                date: date(2026, 8, 9),
                title: "First Post",
            },
            FeedEntry {
                url: "/posts/2",
                date: date(2026, 1, 1),
                title: "Older",
            },
        ];
        let rendered = render(&entries);
        let lines = gemtext::parse(&rendered);
        let parsed_back = metadata::extract_feed_entries(&lines);
        assert_eq!(parsed_back, entries);
    }
}
