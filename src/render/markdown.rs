//! Gemtext → Markdown (CommonMark), the agent-surface serialization
//! (ADR 0011, the "packaging tier"). The HTTP agent audience — GPTBot,
//! ClaudeBot, Perplexity, and the like — reaches only usv's web mirror,
//! and it prefers clean Markdown to scraping HTML chrome. usv already
//! holds the one source of truth (the gemtext tree); this is a second
//! *serialization* of it, written at `page.md` beside `page.html`, so it
//! is an addressable resource, never header-switched content. That
//! distinction matters: an addressable `.md` URL is the same content at a
//! distinct address (no cloaking), whereas returning different bytes to
//! an agent by user-agent sniffing is the pattern ADR 0010 refuses.
//!
//! Unlike [`super::html`], this is **not a security boundary**: Markdown
//! is served as `text/markdown`, not executed by a browser, so there is
//! no injection surface to escape against. The transform is therefore
//! deliberately pragmatic and near-1:1 — it maps gemtext's six line types
//! to their obvious CommonMark equivalents and emits text verbatim, the
//! same "as authored" stance `html` takes toward link targets. It is a
//! convenience rendering for machine reading, not a canonical format.

use super::gemtext::Line;

/// Render a parsed gemtext document as a CommonMark string. Blocks are
/// separated by a blank line (CommonMark's block separator), so a run of
/// list items or quote lines becomes one list / one blockquote, matching
/// how [`super::html`] groups the same runs.
pub fn render(lines: &[Line<'_>]) -> String {
    let mut blocks: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        match &lines[i] {
            Line::Text(s) => {
                // Blank gemtext lines exist only to separate blocks; the
                // blank-line join below already provides that separation,
                // so an empty text line contributes no block of its own.
                if !s.trim().is_empty() {
                    blocks.push((*s).to_string());
                }
                i += 1;
            }
            Line::Heading { level, text } => {
                let hashes = "#".repeat(*level as usize);
                blocks.push(format!("{hashes} {text}"));
                i += 1;
            }
            Line::Link { url, name } => {
                let text = name.unwrap_or(url);
                blocks.push(format!("[{}]({})", link_label(text), link_dest(url)));
                i += 1;
            }
            Line::ListItem(_) => {
                let mut list = String::new();
                while let Some(Line::ListItem(item)) = lines.get(i) {
                    if !list.is_empty() {
                        list.push('\n');
                    }
                    list.push_str("- ");
                    list.push_str(item);
                    i += 1;
                }
                blocks.push(list);
            }
            Line::Quote(_) => {
                let mut quote = String::new();
                while let Some(Line::Quote(text)) = lines.get(i) {
                    if !quote.is_empty() {
                        quote.push('\n');
                    }
                    quote.push_str("> ");
                    // Gemtext keeps whatever followed `>` verbatim
                    // (`> text` → `" text"`); normalise the single leading
                    // space so the Markdown marker isn't doubled.
                    quote.push_str(text.strip_prefix(' ').unwrap_or(text));
                    i += 1;
                }
                blocks.push(quote);
            }
            Line::Preformatted {
                alt_text,
                lines: block,
            } => {
                // A fence long enough that no content line can close it
                // early — CommonMark lets an info-string-bearing fence be
                // any run of 3+ backticks, so we outrun the longest run in
                // the content.
                //
                // **The alt text is not the info string.** CommonMark reads
                // the first word after the fence as a *language*, which
                // renderers turn into `class="language-…"`. Gemtext alt
                // text is a human sentence describing the block, so
                // emitting it there produces `language-Each` from "Each
                // network, when to serve it…" — the caption is lost and a
                // bogus class is invented. Only a single bare word that
                // could plausibly *be* a language is passed through as one;
                // anything else is emitted as an italic caption line above
                // the fence, which renders correctly everywhere Markdown
                // is read (a forge, a docs generator, a plain reader) and
                // keeps the description visible rather than swallowed.
                let fence = "`".repeat(fence_len(block));
                let mut fenced = String::new();
                let mut info = "";
                if let Some(alt) = alt_text {
                    let alt = alt.trim();
                    if !alt.is_empty() {
                        if is_language_token(alt) {
                            info = alt;
                        } else {
                            // Escape the leading `*` case so a caption
                            // beginning with punctuation cannot open a
                            // list or emphasis run of its own.
                            fenced.push('*');
                            fenced.push_str(&alt.replace('*', r"\*"));
                            fenced.push_str("*\n");
                        }
                    }
                }
                fenced.push_str(&fence);
                if !info.is_empty() {
                    fenced.push_str(info);
                }
                for line in block {
                    fenced.push('\n');
                    fenced.push_str(line);
                }
                fenced.push('\n');
                fenced.push_str(&fence);
                blocks.push(fenced);
                i += 1;
            }
        }
    }

    let mut out = blocks.join("\n\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// Escape the two characters that would break a `[label]` — the brackets
/// themselves. Everything else is left verbatim; this is a convenience
/// rendering, not a canonical escaper.
fn link_label(text: &str) -> String {
    text.replace('[', "\\[").replace(']', "\\]")
}

/// A Markdown link destination. A URL containing whitespace or parentheses
/// would break the `(dest)` form, so those are wrapped in the `<...>`
/// destination form CommonMark provides for exactly this case. Clean URLs
/// (the overwhelming majority) pass through untouched.
fn link_dest(url: &str) -> String {
    if url.contains([' ', '\t', '(', ')']) && !url.contains(['<', '>']) {
        format!("<{url}>")
    } else {
        url.to_string()
    }
}

/// Whether alt text is plausibly a language identifier rather than a
/// human caption. Deliberately strict: one token, no whitespace, short,
/// and made only of the characters real language tags use (`c`, `rust`,
/// `sh`, `objective-c`, `f#`, `c++`). Anything with a space, a comma or a
/// capital-led sentence shape is a caption and is rendered as one.
///
/// Erring toward "caption" is the safe direction — a caption rendered as
/// a caption is always correct, whereas a sentence rendered as a language
/// tag is always wrong.
fn is_language_token(alt: &str) -> bool {
    !alt.is_empty()
        && alt.len() <= 16
        && alt
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '#' | '.' | '_'))
}

/// The number of backticks to fence a preformatted block with: one more
/// than the longest backtick run anywhere in its content, and never fewer
/// than three.
fn fence_len(block: &[&str]) -> usize {
    let longest_run = block
        .iter()
        .flat_map(|line| line.split(|c| c != '`').map(str::len).filter(|&n| n > 0))
        .max()
        .unwrap_or(0);
    (longest_run + 1).max(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::gemtext::parse;

    fn md(src: &str) -> String {
        render(&parse(src))
    }

    #[test]
    fn headings_map_to_atx() {
        assert_eq!(md("# One\n"), "# One\n");
        assert_eq!(md("## Two\n"), "## Two\n");
        assert_eq!(md("### Three\n"), "### Three\n");
    }

    #[test]
    fn text_is_emitted_verbatim_as_a_paragraph() {
        assert_eq!(md("hello world\n"), "hello world\n");
    }

    #[test]
    fn blocks_are_separated_by_a_blank_line() {
        assert_eq!(md("# Title\n\nA paragraph.\n"), "# Title\n\nA paragraph.\n");
    }

    #[test]
    fn blank_text_lines_do_not_produce_empty_blocks() {
        // Two paragraphs separated by one blank line, not three blocks.
        assert_eq!(md("a\n\n\nb\n"), "a\n\nb\n");
    }

    #[test]
    fn link_with_name_becomes_an_inline_link() {
        assert_eq!(md("=> https://x/ Home\n"), "[Home](https://x/)\n");
    }

    #[test]
    fn link_without_name_uses_the_url_as_label() {
        assert_eq!(md("=> https://x/\n"), "[https://x/](https://x/)\n");
    }

    #[test]
    fn link_with_parens_in_the_url_uses_the_angle_form() {
        // A parsed URL never contains whitespace (gemtext splits url from
        // name at the first space), but it can contain parentheses, which
        // would break the `(dest)` form — so those take the `<dest>` form.
        assert_eq!(md("=> /a(b) Label\n"), "[Label](</a(b)>)\n");
    }

    #[test]
    fn link_label_brackets_are_escaped() {
        assert_eq!(md("=> /x [bracketed]\n"), "[\\[bracketed\\]](/x)\n");
    }

    #[test]
    fn consecutive_list_items_share_one_list() {
        assert_eq!(md("* one\n* two\n* three\n"), "- one\n- two\n- three\n");
    }

    #[test]
    fn list_broken_by_text_becomes_two_lists() {
        assert_eq!(md("* one\ntext\n* two\n"), "- one\n\ntext\n\n- two\n");
    }

    #[test]
    fn consecutive_quotes_share_one_blockquote() {
        assert_eq!(md(">line one\n>line two\n"), "> line one\n> line two\n");
    }

    #[test]
    fn quote_leading_space_is_normalised_not_doubled() {
        assert_eq!(md("> spaced\n"), "> spaced\n");
    }

    #[test]
    fn preformatted_becomes_a_fenced_code_block() {
        assert_eq!(
            md("```rust\nfn f() {}\nlet x = 1;\n```\n"),
            "```rust\nfn f() {}\nlet x = 1;\n```\n"
        );
    }

    #[test]
    fn a_descriptive_alt_text_becomes_a_caption_not_a_language_tag() {
        // The defect this guards: CommonMark reads the first token after
        // the fence as a language, so a sentence produced
        // `class="language-Each"` and lost the description entirely.
        let out = md("```Each network, when to serve it\nA  B\n```\n");
        assert!(out.contains("*Each network, when to serve it*"), "{out}");
        assert!(
            !out.contains("```Each"),
            "a sentence must never land in the info string: {out}"
        );
    }

    #[test]
    fn a_bare_language_token_is_still_passed_through_for_highlighting() {
        assert!(md("```rust\nfn main() {}\n```\n").contains("```rust"));
    }

    #[test]
    fn preformatted_without_alt_has_a_bare_fence() {
        assert_eq!(md("```\ncode\n```\n"), "```\ncode\n```\n");
    }

    #[test]
    fn a_fence_outgrows_backticks_in_the_content() {
        // Content containing ``` must be fenced with a longer run so it
        // can't close the block early.
        let out = md("```\nhas ``` inside\n```\n");
        assert!(
            out.starts_with("````"),
            "fence must outrun content: {out:?}"
        );
        assert!(out.trim_end().ends_with("````"));
    }

    #[test]
    fn a_full_document_round_trips_structurally() {
        let src = "# Welcome\n\n\
                   Some prose.\n\n\
                   * a\n* b\n\n\
                   > a quote\n\n\
                   => gemini://x/ A link\n";
        let out = md(src);
        assert_eq!(
            out,
            "# Welcome\n\n\
             Some prose.\n\n\
             - a\n- b\n\n\
             > a quote\n\n\
             [A link](gemini://x/)\n"
        );
    }
}
