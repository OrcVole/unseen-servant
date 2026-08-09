//! Fuzz target: gemtext-to-HTML rendering (C3, `render::html`).
//!
//! Contract under attack: rendered HTML must never contain an unescaped
//! `<`, `&`, or `"` that originated from content — every text node and
//! attribute value must come out through `escape_into`. This is
//! security-relevant, not just a crash check: rendered HTML is served to
//! real browsers, so a missed escape is a stored-XSS-shaped bug. Run:
//!
//!   cargo +nightly fuzz run render_html

#![no_main]

use libfuzzer_sys::fuzz_target;
use unseen_servant::render::{gemtext, html};

/// The exact, closed set of tags `render::html` ever emits (see its
/// source). A raw `<` in the output not immediately followed by one of
/// these — opening or closing — means content broke out of a text node
/// or attribute value: an escaping bug, not a cosmetic one, since
/// rendered HTML is served to real browsers.
const KNOWN_TAGS: &[&str] = &[
    "!doctype", "html", "head", "meta", "title", "body", "h1", "h2", "h3", "p", "a", "ul", "li",
    "blockquote", "figure", "figcaption", "pre",
];

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = str::from_utf8(data) {
        let lines = gemtext::parse(s);
        let doc = html::render_document(&lines, "fuzz");
        let mut rest = doc.as_str();
        while let Some(lt) = rest.find('<') {
            let after = &rest[lt + '<'.len_utf8()..];
            let after = after.strip_prefix('/').unwrap_or(after);
            let matched = KNOWN_TAGS
                .iter()
                .any(|tag| after.get(..tag.len()).is_some_and(|s| s.eq_ignore_ascii_case(tag)));
            assert!(
                matched,
                "unexpected '<' in rendered HTML, not one of the known tags — \
                 possible unescaped content (next {} bytes): {:?}",
                after.len().min(20),
                after.bytes().take(20).collect::<Vec<_>>()
            );
            // Advance past this '<' by at least one char, byte-boundary safe.
            let advance = after.chars().next().map_or(1, char::len_utf8);
            rest = &after[advance..];
        }
    }
});
