//! Gemtext → HTML: semantic, classless markup (ADR 0004). Every text node
//! and every attribute value is escaped — content is untrusted input to
//! this module regardless of who authored it, the same discipline the
//! protocol layer applies to wire bytes.
//!
//! Resolves design brief §5.1 (`docs/notes/c3-render-design-brief.md`):
//! link targets are emitted **as written, HTML-attribute-escaped**, no
//! URL validation or rewriting. HTML-attribute escaping alone is what
//! makes an href value *safe* to embed (it is what stops attribute
//! injection); percent-encoding spaces or other bytes in the URL itself
//! is a *functionality* nicety for the resulting link, not a security
//! requirement, and is left to content authors per the spec's own
//! guidance that they "MUST percent-encode" when writing gemtext.
//!
//! Consecutive [`Line::ListItem`] and [`Line::Quote`] lines are grouped
//! into one `<ul>`/`<blockquote>` each — HTML validity requires `<li>` to
//! live inside a list container, and a run of quote lines reads as one
//! quoted passage, not N independent blockquotes. Every other line type
//! maps one-to-one to one element, since gemtext's own "never collapse
//! blank lines" rule means a paragraph-merging heuristic would be
//! actively wrong here.

use super::gemtext::Line;

/// Render a complete HTML document from a parsed gemtext body and its
/// already-extracted title (`gemtext::extract_title`).
pub fn render_document(lines: &[Line<'_>], title: &str) -> String {
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>");
    escape_into(&mut out, title);
    out.push_str("</title>\n</head>\n<body>\n");
    render_body(&mut out, lines);
    out.push_str("</body>\n</html>\n");
    out
}

/// Render just the body content (no document wrapper) — the pipeline may
/// want this for embedding into a theme template rather than a bare
/// document (theme integration is task 23's territory).
pub fn render_body(out: &mut String, lines: &[Line<'_>]) {
    let mut i = 0;
    while i < lines.len() {
        match &lines[i] {
            Line::Text(s) => {
                out.push_str("<p>");
                escape_into(out, s);
                out.push_str("</p>\n");
                i += 1;
            }
            Line::Heading { level, text } => {
                let tag = match level {
                    1 => "h1",
                    2 => "h2",
                    _ => "h3", // level is always 1..=3 by construction
                };
                out.push('<');
                out.push_str(tag);
                out.push('>');
                escape_into(out, text);
                out.push_str("</");
                out.push_str(tag);
                out.push_str(">\n");
                i += 1;
            }
            Line::Link { url, name } => {
                out.push_str("<p><a href=\"");
                escape_into(out, url);
                out.push_str("\">");
                escape_into(out, name.unwrap_or(url));
                out.push_str("</a></p>\n");
                i += 1;
            }
            Line::ListItem(_) => {
                out.push_str("<ul>\n");
                while let Some(Line::ListItem(item)) = lines.get(i) {
                    out.push_str("<li>");
                    escape_into(out, item);
                    out.push_str("</li>\n");
                    i += 1;
                }
                out.push_str("</ul>\n");
            }
            Line::Quote(_) => {
                out.push_str("<blockquote>\n");
                while let Some(Line::Quote(text)) = lines.get(i) {
                    out.push_str("<p>");
                    escape_into(out, text);
                    out.push_str("</p>\n");
                    i += 1;
                }
                out.push_str("</blockquote>\n");
            }
            Line::Preformatted {
                alt_text,
                lines: block,
            } => {
                out.push_str("<figure>\n");
                if let Some(alt) = alt_text {
                    out.push_str("<figcaption>");
                    escape_into(out, alt);
                    out.push_str("</figcaption>\n");
                }
                out.push_str("<pre>");
                for (n, line) in block.iter().enumerate() {
                    if n > 0 {
                        out.push('\n');
                    }
                    escape_into(out, line);
                }
                out.push_str("</pre>\n</figure>\n");
                i += 1;
            }
        }
    }
}

/// Escape `&`, `<`, `>`, and `"` — sufficient and necessary for both text
/// nodes and (double-quoted) attribute values, which is every context
/// this module ever writes into. Single quotes are left alone since
/// nothing here ever uses single-quoted attributes.
fn escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::gemtext::parse;

    #[test]
    fn text_line_becomes_paragraph() {
        let lines = parse("hello world\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(out, "<p>hello world</p>\n");
    }

    #[test]
    fn empty_text_line_is_still_a_paragraph() {
        // gemtext never collapses blank lines; the HTML must not either.
        let lines = parse("a\n\nb\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(out, "<p>a</p>\n<p></p>\n<p>b</p>\n");
    }

    #[test]
    fn headings_map_to_h1_h2_h3() {
        let lines = parse("# One\n## Two\n### Three\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(out, "<h1>One</h1>\n<h2>Two</h2>\n<h3>Three</h3>\n");
    }

    #[test]
    fn link_with_name_uses_name_as_text() {
        let lines = parse("=> gemini://x/ Home\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(out, "<p><a href=\"gemini://x/\">Home</a></p>\n");
    }

    #[test]
    fn link_without_name_uses_url_as_text() {
        let lines = parse("=> gemini://x/\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(out, "<p><a href=\"gemini://x/\">gemini://x/</a></p>\n");
    }

    #[test]
    fn consecutive_list_items_share_one_ul() {
        let lines = parse("* one\n* two\n* three\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(
            out,
            "<ul>\n<li>one</li>\n<li>two</li>\n<li>three</li>\n</ul>\n"
        );
    }

    #[test]
    fn list_items_separated_by_text_get_separate_uls() {
        let lines = parse("* one\ntext\n* two\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(
            out,
            "<ul>\n<li>one</li>\n</ul>\n<p>text</p>\n<ul>\n<li>two</li>\n</ul>\n"
        );
    }

    #[test]
    fn consecutive_quotes_share_one_blockquote() {
        let lines = parse(">line one\n>line two\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(
            out,
            "<blockquote>\n<p>line one</p>\n<p>line two</p>\n</blockquote>\n"
        );
    }

    #[test]
    fn preformatted_block_becomes_figure_pre() {
        let lines = parse("```rust\nfn f() {}\nlet x = 1;\n```\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(
            out,
            "<figure>\n<figcaption>rust</figcaption>\n<pre>fn f() {}\nlet x = 1;</pre>\n</figure>\n"
        );
    }

    #[test]
    fn preformatted_without_alt_text_has_no_figcaption() {
        let lines = parse("```\ncode\n```\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(out, "<figure>\n<pre>code</pre>\n</figure>\n");
    }

    #[test]
    fn text_is_html_escaped() {
        let lines = parse("<script>alert(1)</script> & \"quotes\"\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(
            out,
            "<p>&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;quotes&quot;</p>\n"
        );
    }

    #[test]
    fn heading_text_is_html_escaped() {
        let lines = parse("# <b>bold</b>\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert_eq!(out, "<h1>&lt;b&gt;bold&lt;/b&gt;</h1>\n");
    }

    #[test]
    fn link_href_is_attribute_escaped() {
        let lines = parse("=> \"onmouseover=alert(1)// evil\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        // The quote in the URL must not be able to close the href
        // attribute early.
        assert!(!out.contains("href=\"\"onmouseover"));
        assert!(out.contains("&quot;onmouseover"));
    }

    #[test]
    fn preformatted_content_is_html_escaped() {
        let lines = parse("```\n<script>evil()</script>\n```\n");
        let mut out = String::new();
        render_body(&mut out, &lines);
        assert!(out.contains("&lt;script&gt;evil()&lt;/script&gt;"));
        assert!(!out.contains("<script>evil()"));
    }

    #[test]
    fn render_document_wraps_body_with_title() {
        let lines = parse("# Hello\n\nworld\n");
        let doc = render_document(&lines, "Hello");
        assert!(doc.starts_with("<!doctype html>"));
        assert!(doc.contains("<title>Hello</title>"));
        assert!(doc.contains("<h1>Hello</h1>"));
        assert!(doc.ends_with("</html>\n"));
    }

    #[test]
    fn render_document_escapes_the_title() {
        let doc = render_document(&[], "<script>x</script>");
        assert!(doc.contains("<title>&lt;script&gt;x&lt;/script&gt;</title>"));
    }

    #[test]
    fn empty_document_renders_empty_body() {
        let doc = render_document(&[], "Empty");
        assert!(doc.contains("<body>\n</body>"));
    }

    /// The same invariant `fuzz/fuzz_targets/render_html.rs` drives at
    /// scale: every `<` in rendered output must belong to one of the
    /// tags this module actually emits, over a battery of adversarial
    /// gemtext inputs that try to break out of every context this module
    /// writes into (text, heading, link name, link href, list item,
    /// quote, preformatted content, and the alt-text caption).
    #[test]
    fn no_content_ever_produces_a_stray_angle_bracket() {
        const KNOWN_TAGS: &[&str] = &[
            "!doctype",
            "html",
            "head",
            "meta",
            "title",
            "body",
            "h1",
            "h2",
            "h3",
            "p",
            "a",
            "ul",
            "li",
            "blockquote",
            "figure",
            "figcaption",
            "pre",
        ];
        let adversarial = [
            "<script>alert(1)</script>",
            "# <img src=x onerror=alert(1)>",
            "=> \"><script>alert(1)</script> evil",
            "=> /x \"><script>alert(1)</script>",
            "* <b>bold</b> item",
            ">quoted <script>evil()</script>",
            "```<script>evil-alt-text</script>\ncode\n```",
            "```\n<script>evil-in-preformat</script>\n```",
            "<>&\"'",
        ];
        for src in adversarial {
            let lines = parse(src);
            let doc = render_document(&lines, "<script>title-injection</script>");
            let mut rest = doc.as_str();
            while let Some(lt) = rest.find('<') {
                let after = &rest[lt + 1..];
                let after = after.strip_prefix('/').unwrap_or(after);
                let matched = KNOWN_TAGS.iter().any(|tag| {
                    after
                        .get(..tag.len())
                        .is_some_and(|s| s.eq_ignore_ascii_case(tag))
                });
                assert!(
                    matched,
                    "stray '<' for input {src:?}: next bytes {:?}",
                    &after[..after.len().min(20)]
                );
                rest = &after[1..];
            }
        }
    }
}
