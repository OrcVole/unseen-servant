//! `/llms.txt` emission (ADR 0011, the "packaging tier"). The llms.txt
//! convention (llmstxt.org) is a curated Markdown index at a site's root
//! that gives an agent the page inventory in one fetch instead of a
//! crawl — an H1, an optional blockquote summary, then H2-grouped link
//! lists. usv already builds exactly this inventory for the site map
//! (ADR 0010); llms.txt is the same data re-serialized into the format
//! and location HTTP agents look for.
//!
//! It is written only to the web surface: the convention is an HTTP one
//! (`https://…/llms.txt`), and the Gemini surface already has its native
//! equivalent in `map.gmi`. This keeps the "one truth, two renderings"
//! shape — the site map, the XML sitemap, and llms.txt are three
//! serializations of the single page list the render walk produces.

use super::sitemap::PageEntry;

/// The reserved web-surface filename for the llms.txt index.
pub const LLMS_TXT_NAME: &str = "llms.txt";

/// Render `llms.txt` from the page list. `base_url` (may be empty)
/// prefixes each link so an agent gets absolute URLs when the capsule
/// knows its own address; with no base the links are root-relative, which
/// is still valid and resolvable against the fetch origin.
pub fn render(entries: &[PageEntry], capsule_title: &str, base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let mut sorted: Vec<&PageEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.web_path.cmp(&b.web_path));

    let mut out = String::new();
    out.push_str("# ");
    out.push_str(capsule_title);
    out.push_str(
        "\n\n\
         > A complete list of this capsule's pages, generated from its \
         content whenever it changes. One fetch instead of a crawl.\n\n\
         ## Pages\n\n",
    );
    for entry in sorted {
        out.push_str("- [");
        out.push_str(&entry.title);
        out.push_str("](");
        out.push_str(base);
        out.push_str(&markdown_path(&entry.web_path));
        out.push_str(")\n");
    }
    out
}

/// The `.md` sibling of a rendered page. Every page is written twice —
/// `page.html` and `page.md`, from the same render pass — and this index
/// links the Markdown one: the whole reason a reader fetches `llms.txt`
/// is to avoid parsing markup, so handing it back a list of HTML URLs
/// would undo the point of both files. The `.html` form remains at its
/// own address for anything that wants it; this is a choice of which
/// serialization to *link*, not a redirect or a content negotiation.
fn markdown_path(web_path: &str) -> String {
    match web_path.strip_suffix(".html") {
        Some(stem) => format!("{stem}.md"),
        // Anything not ending `.html` has no `.md` sibling to point at,
        // so it is linked unchanged rather than given a path that 404s.
        None => web_path.to_string(),
    }
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
    fn has_the_llms_txt_shape_h1_summary_and_links() {
        let out = render(&[entry("about", "About")], "My Capsule", "");
        assert!(out.starts_with("# My Capsule\n"));
        assert!(out.contains("\n> "), "a blockquote summary line");
        assert!(out.contains("## Pages"));
        assert!(out.contains("- [About](/about.md)"));
    }

    #[test]
    fn a_base_url_makes_links_absolute() {
        let out = render(&[entry("about", "About")], "T", "https://example.org");
        assert!(out.contains("- [About](https://example.org/about.md)"));
    }

    #[test]
    fn a_trailing_slash_on_the_base_is_not_doubled() {
        let out = render(&[entry("x", "X")], "T", "https://example.org/");
        assert!(out.contains("https://example.org/x.md"));
        assert!(!out.contains("org//x"));
    }

    #[test]
    fn links_the_markdown_sibling_not_the_html_one() {
        // The index exists so a reader need not parse markup; linking
        // `.html` from it would defeat both this file and the `.md`
        // siblings the render pass already writes.
        let out = render(&[entry("about", "About")], "T", "");
        assert!(out.contains("/about.md"));
        assert!(!out.contains("/about.html"));
    }

    #[test]
    fn a_path_with_no_markdown_sibling_is_linked_unchanged() {
        let out = render(
            &[PageEntry {
                gemini_path: "/feed.gmi".to_string(),
                web_path: "/atom.xml".to_string(),
                title: "Feed".to_string(),
            }],
            "T",
            "",
        );
        assert!(out.contains("- [Feed](/atom.xml)"));
    }

    #[test]
    fn entries_are_sorted_so_output_is_stable() {
        let out = render(&[entry("zebra", "Z"), entry("apple", "A")], "T", "");
        let apple = out.find("/apple.md").expect("apple");
        let zebra = out.find("/zebra.md").expect("zebra");
        assert!(apple < zebra, "entries must be sorted by web path");
    }
}
