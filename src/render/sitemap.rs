//! Site map emission for both surfaces (ADR 0010): `map.gmi` for Gemini
//! and `sitemap.xml` for the web, from the page list the render walk
//! already produces.
//!
//! Chosen because one artifact serves three audiences at once, which is
//! the ADR's whole test for whether a "legibility" feature earns its
//! place:
//!
//! - **Assistive users**: a site map is WCAG 2.4.5 ("Multiple Ways") —
//!   a second route to every page that doesn't depend on finding a link
//!   buried in prose.
//! - **Humans**: an ordinary, useful index of what's here.
//! - **Agents and crawlers**: the complete inventory without crawling.
//!   Feeds only ever cover *dated* posts; everything else was previously
//!   discoverable only by following links from the index.
//!
//! `sitemap.xml` is a real, established convention rather than an
//! invention; the Gemini side is plain gemtext link lines, which is
//! simply what an index page looks like there.

/// One page in the map: where it lives and what it's called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageEntry {
    /// Root-relative path as served on Gemini, e.g. `/blog/post.gmi`.
    pub gemini_path: String,
    /// Root-relative path as served on the web, e.g. `/blog/post.html`.
    pub web_path: String,
    /// The page's title (its first level-1 heading, or a filename
    /// fallback — `gemtext::extract_title`).
    pub title: String,
}

/// Render the Gemini-side map as gemtext: a heading and one link line
/// per page, sorted by path so the output is deterministic (a map that
/// reshuffles on every render would churn the file and the watcher).
pub fn render_gemtext(entries: &[PageEntry]) -> String {
    let mut sorted: Vec<&PageEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.gemini_path.cmp(&b.gemini_path));

    let mut out = String::from(
        "# Everything on this capsule\n\n\
         A complete list of pages, regenerated whenever content changes.\n\n",
    );
    for entry in sorted {
        out.push_str("=> ");
        out.push_str(&entry.gemini_path);
        out.push(' ');
        out.push_str(&entry.title);
        out.push('\n');
    }
    out
}

/// Render the web-side `sitemap.xml`. Returns `None` without a base URL:
/// the format requires absolute `<loc>` values, and a sitemap full of
/// unresolvable paths is worse than none (the same rule the Atom feed
/// follows).
pub fn render_xml(entries: &[PageEntry], base_url: &str) -> Option<String> {
    if base_url.is_empty() || entries.is_empty() {
        return None;
    }
    let base = base_url.trim_end_matches('/');
    let mut sorted: Vec<&PageEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.web_path.cmp(&b.web_path));

    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for entry in sorted {
        out.push_str("<url><loc>");
        crate::render::escape_into(&mut out, base);
        crate::render::escape_into(&mut out, &entry.web_path);
        out.push_str("</loc></url>\n");
    }
    out.push_str("</urlset>\n");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, title: &str) -> PageEntry {
        PageEntry {
            gemini_path: format!("/{path}.gmi"),
            web_path: format!("/{path}.html"),
            title: title.to_string(),
        }
    }

    #[test]
    fn gemtext_map_lists_every_page_as_a_link() {
        let entries = [entry("index", "Home"), entry("about", "About")];
        let out = render_gemtext(&entries);
        assert!(out.contains("=> /index.gmi Home"));
        assert!(out.contains("=> /about.gmi About"));
    }

    #[test]
    fn gemtext_map_is_sorted_so_output_is_stable() {
        // Deterministic order matters: an unsorted map would rewrite (and
        // re-trigger the watcher) on every render for no reason.
        let unsorted = [entry("zebra", "Z"), entry("apple", "A")];
        let out = render_gemtext(&unsorted);
        let apple = out.find("/apple.gmi").expect("apple present");
        let zebra = out.find("/zebra.gmi").expect("zebra present");
        assert!(apple < zebra, "entries must be sorted by path");
    }

    #[test]
    fn gemtext_map_of_an_empty_capsule_still_parses() {
        let out = render_gemtext(&[]);
        let lines = crate::render::gemtext::parse(&out);
        assert!(!lines.is_empty(), "still a valid document with a heading");
    }

    #[test]
    fn xml_sitemap_has_absolute_locs() {
        let entries = [entry("about", "About")];
        let out = render_xml(&entries, "https://example.org").expect("has entries");
        assert!(out.contains("<loc>https://example.org/about.html</loc>"));
        assert!(out.starts_with("<?xml"));
        assert!(out.contains("http://www.sitemaps.org/schemas/sitemap/0.9"));
    }

    #[test]
    fn xml_sitemap_trims_a_trailing_slash_on_the_base() {
        let entries = [entry("about", "About")];
        let out = render_xml(&entries, "https://example.org/").expect("has entries");
        assert!(out.contains("https://example.org/about.html"));
        assert!(!out.contains("org//about"));
    }

    #[test]
    fn xml_sitemap_needs_a_base_url_and_entries() {
        let entries = [entry("about", "About")];
        assert!(
            render_xml(&entries, "").is_none(),
            "no base URL → no sitemap"
        );
        assert!(
            render_xml(&[], "https://example.org").is_none(),
            "no pages → no sitemap"
        );
    }

    #[test]
    fn xml_sitemap_escapes_paths() {
        let entries = [PageEntry {
            gemini_path: "/x.gmi".into(),
            web_path: "/a&b.html".into(),
            title: "x".into(),
        }];
        let out = render_xml(&entries, "https://example.org").expect("has entries");
        assert!(out.contains("a&amp;b"), "XML text must be escaped");
        assert!(
            !out.contains("a&b."),
            "a raw ampersand would be invalid XML"
        );
    }
}
