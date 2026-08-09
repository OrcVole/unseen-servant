//! Metadata extraction: titles and feed-worthy dates (BUILD-PLAN C3
//! "metadata pass"). Design decisions recorded in
//! `docs/notes/c3-render-design-brief.md` §4/§5.5, resolved here rather
//! than left open, since nothing about them is security- or auth-
//! sensitive — they are content conventions, changeable later without
//! an ADR.
//!
//! **Title** lives in [`crate::render::gemtext::extract_title`] (the one
//! piece needed by C3's first commit to unblock the HTML emitter's
//! mandatory `<title>`).
//!
//! **Date**, per the *subscription companion spec* convention — the only
//! spec-adjacent date convention that exists — lives on an **index**
//! page's own link lines, not inside the target page: `=> URL
//! YYYY-MM-DD - title` or `=> URL YYYY-MM-DD title`. A page's date is
//! therefore discovered by walking whatever index page(s) reference it,
//! not by reading the page itself. Pages with no referencing dated link
//! anywhere fall back to filesystem mtime for Atom's mandatory
//! `<updated>` field (that fallback is the pipeline's job, not this
//! module's — this module only ever reports what it can read from
//! content).

use time::Date;

use super::gemtext::Line;

/// One dated entry found on an index page's link lines — a page this
/// index treats as "feed-worthy," with the date and title the index
/// itself declared for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedEntry<'a> {
    /// The link target, exactly as written on the index page.
    pub url: &'a str,
    /// The date the index declared for this entry.
    pub date: Date,
    /// The title text following the date in the link's NAME (the part
    /// after `YYYY-MM-DD` and an optional ` - ` separator).
    pub title: &'a str,
}

/// Walk a parsed index document's link lines, keeping only the ones whose
/// NAME starts with a `YYYY-MM-DD` date — the subscription companion
/// spec's feed convention. Ordinary links on the same page (a "Home" link,
/// a link with no NAME, a NAME that doesn't start with a date) are simply
/// not feed entries; this is not an error, just a filter.
pub fn extract_feed_entries<'a>(lines: &'a [Line<'a>]) -> Vec<FeedEntry<'a>> {
    lines
        .iter()
        .filter_map(|line| match line {
            Line::Link {
                url,
                name: Some(name),
            } => {
                let (date, title) = parse_dated_name(name)?;
                Some(FeedEntry { url, date, title })
            }
            _ => None,
        })
        .collect()
}

/// Parse a link NAME of the form `YYYY-MM-DD - title` or `YYYY-MM-DD
/// title` (the separating `-` is optional; any amount of whitespace
/// around it is accepted). Returns `None` — not an error, just "this
/// isn't a dated entry" — for anything that doesn't start with a
/// well-formed calendar date, including an invalid one like `2026-13-40`.
fn parse_dated_name(name: &str) -> Option<(Date, &str)> {
    // "YYYY-MM-DD" is exactly 10 ASCII bytes when well-formed; check the
    // shape before ever asking `time` to parse it, so a merely
    // short/malformed prefix is a clean `None` rather than a parse error
    // to propagate.
    if name.len() < 10 || !name.is_char_boundary(10) {
        return None;
    }
    let (date_str, rest) = name.split_at(10);
    let bytes = date_str.as_bytes();
    let shape_ok = bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit);
    if !shape_ok {
        return None;
    }
    let format = time::macros::format_description!("[year]-[month]-[day]");
    let date = Date::parse(date_str, &format).ok()?;

    let title = rest
        .trim_start()
        .strip_prefix('-')
        .map(str::trim_start)
        .unwrap_or_else(|| rest.trim_start());
    Some((date, title))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;
    use crate::render::gemtext;

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, time::Month::try_from(m).unwrap(), d).unwrap()
    }

    #[test]
    fn dated_name_with_dash_separator() {
        let (d, title) = parse_dated_name("2026-08-09 - My Post").unwrap();
        assert_eq!(d, date(2026, 8, 9));
        assert_eq!(title, "My Post");
    }

    #[test]
    fn dated_name_without_dash_separator() {
        let (d, title) = parse_dated_name("2026-08-09 My Post").unwrap();
        assert_eq!(d, date(2026, 8, 9));
        assert_eq!(title, "My Post");
    }

    #[test]
    fn dated_name_with_extra_whitespace() {
        let (d, title) = parse_dated_name("2026-08-09   -   My Post").unwrap();
        assert_eq!(d, date(2026, 8, 9));
        assert_eq!(title, "My Post");
    }

    #[test]
    fn non_dated_name_is_none() {
        assert!(parse_dated_name("Home").is_none());
        assert!(parse_dated_name("About us").is_none());
        assert!(parse_dated_name("").is_none());
    }

    #[test]
    fn short_prefix_is_none_not_a_panic() {
        assert!(parse_dated_name("2026").is_none());
        assert!(parse_dated_name("2026-08").is_none());
    }

    #[test]
    fn malformed_date_shape_is_none() {
        assert!(parse_dated_name("2026/08/09 title").is_none());
        assert!(parse_dated_name("20260809xx title").is_none());
    }

    #[test]
    fn invalid_calendar_date_is_none_not_a_panic() {
        assert!(parse_dated_name("2026-13-40 title").is_none());
        assert!(parse_dated_name("2026-02-30 title").is_none());
    }

    #[test]
    fn non_ascii_before_byte_ten_does_not_panic() {
        // A multibyte char inside the first 10 bytes must not panic the
        // byte-index slicing — it just isn't a dated name.
        assert!(parse_dated_name("café-08-09 x").is_none());
        assert!(parse_dated_name("💥").is_none());
    }

    #[test]
    fn extract_feed_entries_filters_non_dated_links() {
        let doc = "=> /about About\n\
                    => /posts/1 2026-08-09 - First Post\n\
                    => /posts/2 2026-08-01 Second Post\n\
                    => /posts/3\n\
                    plain text\n";
        let lines = gemtext::parse(doc);
        let entries = extract_feed_entries(&lines);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "/posts/1");
        assert_eq!(entries[0].title, "First Post");
        assert_eq!(entries[0].date, date(2026, 8, 9));
        assert_eq!(entries[1].url, "/posts/2");
        assert_eq!(entries[1].title, "Second Post");
    }

    #[test]
    fn extract_feed_entries_on_empty_or_dateless_index_is_empty() {
        assert_eq!(extract_feed_entries(&gemtext::parse("")), Vec::new());
        assert_eq!(
            extract_feed_entries(&gemtext::parse("=> /a Home\n# heading\ntext\n")),
            Vec::new()
        );
    }
}
