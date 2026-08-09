//! Gemtext line-type grammar, spec v0.24.1 (docs/recon/protocol.md
//! "Gemtext" section; full grammar and rationale in
//! `docs/notes/c3-render-design-brief.md` §2).
//!
//! One pass, one bit of state (normal vs. preformatted), starting in
//! normal mode; state at EOF is meaningless per spec, so an unclosed
//! preformat block at end-of-input is not an error — it just ends there.
//! Decision order, per line, in normal mode: link (`=>`) → heading (`#`,
//! `##`, `###`) → list item (`* `, space mandatory) → quote (`>`, no space
//! required) → fallback to plain text. A preformat toggle (exactly
//! ` ``` ` at column 0) is recognized in *either* mode and is never
//! itself a text/heading/list/quote line; recognizing everything else
//! only happens in normal mode, since preformatted content is verbatim by
//! definition.
//!
//! This parser never fails: every byte sequence produces *some* sequence
//! of [`Line`]s (fuzzed by `fuzz/fuzz_targets/parse_gemtext.rs` to prove
//! it). Optional line types that a lenient renderer chooses not to
//! special-case would fall back to `Line::Text` per spec ("MUST render as
//! plain text"); this parser always recognizes all six types, so that
//! choice is left to callers, not baked in here.

use std::path::Path;

/// One parsed line of a gemtext document, borrowing from the source text.
/// A preformat block collapses its whole run of lines (open toggle to
/// close toggle or EOF) into a single [`Line::Preformatted`] — callers
/// that want one `<pre>` element don't have to reassemble it from
/// individual lines, and the toggle lines themselves (never rendered) are
/// never exposed as their own `Line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line<'a> {
    /// Plain text — the fallback for anything not matching another type,
    /// and the mandatory rendering for optional types a caller skips.
    Text(&'a str),
    /// `=> URL [NAME]`. Content hygiene (validation, encoding) is an
    /// authoring concern, not a parse-time one — both fields are exactly
    /// as written.
    Link {
        /// The link target, exactly as written (no decoding, no
        /// validation). Empty when the line has nothing after `=>`.
        url: &'a str,
        /// The display name, if present and non-empty after trimming
        /// surrounding whitespace.
        name: Option<&'a str>,
    },
    /// `#`/`##`/`###` heading.
    Heading {
        /// 1–3. A run of 4+ `#` characters still yields 3, with the extra
        /// `#`s folded into `text` (the ABNF only ever consumes up to
        /// three).
        level: u8,
        /// The heading text, with one leading whitespace run stripped.
        text: &'a str,
    },
    /// `* text` — the space after `*` is mandatory; `*text` (no space)
    /// is `Text`, not this.
    ListItem(&'a str),
    /// `>text` — no space required after `>`.
    Quote(&'a str),
    /// A preformatted block: the lines between an opening ` ``` ` (with
    /// optional `alt_text`) and the matching close (or EOF), verbatim,
    /// never re-parsed. The closing toggle's own trailing text is
    /// discarded per spec ("MUST be ignored").
    Preformatted {
        /// Trailing text on the opening ` ``` ` line, if any (e.g. a
        /// language name for syntax highlighting).
        alt_text: Option<&'a str>,
        /// The block's content lines, verbatim, never re-parsed.
        lines: Vec<&'a str>,
    },
}

/// Parse a complete gemtext document. Never panics on any input,
/// including invalid UTF-8 boundaries (impossible to construct in `&str`
/// anyway), empty input (yields an empty `Vec` — lenient reading of the
/// spec's `1*gemtext-line`; an empty file is a degenerate document, not a
/// parse error), and a document that opens a preformat block and never
/// closes it.
///
/// Line splitting accepts both CRLF and bare LF uniformly (`str::lines`
/// strips a trailing `\r`), matching content-file leniency (ADR 0004) —
/// distinct from the wire-protocol request parser, which rejects bare LF
/// for an unrelated reason (request-smuggling hygiene, not content
/// leniency).
pub fn parse(input: &str) -> Vec<Line<'_>> {
    let input = strip_leading_bom(input);
    let mut lines = Vec::new();
    let mut iter = input.lines();
    while let Some(raw) = iter.next() {
        if let Some(alt) = raw.strip_prefix("```") {
            let alt_text = if alt.is_empty() { None } else { Some(alt) };
            let mut block = Vec::new();
            for l in iter.by_ref() {
                if l.starts_with("```") {
                    break; // closing toggle; its own trailing text is discarded
                }
                block.push(l);
            }
            lines.push(Line::Preformatted {
                alt_text,
                lines: block,
            });
            continue;
        }
        lines.push(parse_normal_line(raw));
    }
    lines
}

/// Clients/servers SHOULD ignore a leading BOM (recon: 0.24.1 change).
/// Only the very first character of the *document* is checked — a BOM
/// appearing mid-document is just a text character.
fn strip_leading_bom(input: &str) -> &str {
    input.strip_prefix('\u{feff}').unwrap_or(input)
}

fn parse_normal_line(line: &str) -> Line<'_> {
    if let Some(rest) = line.strip_prefix("=>") {
        parse_link(rest)
    } else if let Some(rest) = line.strip_prefix('#') {
        parse_heading(rest, 1)
    } else if let Some(rest) = line.strip_prefix("* ") {
        Line::ListItem(rest)
    } else if let Some(rest) = line.strip_prefix('>') {
        Line::Quote(rest)
    } else {
        Line::Text(line)
    }
}

const LINK_WS: [char; 2] = [' ', '\t'];

fn parse_link(rest: &str) -> Line<'_> {
    let rest = rest.trim_start_matches(LINK_WS);
    match rest.find(LINK_WS) {
        Some(i) => {
            let url = &rest[..i];
            let name = rest[i..].trim_start_matches(LINK_WS);
            Line::Link {
                url,
                name: (!name.is_empty()).then_some(name),
            }
        }
        None => Line::Link {
            url: rest,
            name: None,
        },
    }
}

fn parse_heading(after_hashes: &str, level: u8) -> Line<'_> {
    if level < 3
        && let Some(rest) = after_hashes.strip_prefix('#')
    {
        return parse_heading(rest, level + 1);
    }
    Line::Heading {
        level,
        text: after_hashes.trim_start_matches(LINK_WS),
    }
}

/// First level-1 heading in the document, or a filename-derived fallback
/// (`about.gmi` → `"About"`) when none exists — every document needs a
/// non-empty title for the HTML `<title>` element (`docs/notes/
/// c3-render-design-brief.md` §4/§5.6).
pub fn extract_title<'a>(lines: &'a [Line<'a>], source_path: &Path) -> String {
    for line in lines {
        if let Line::Heading { level: 1, text } = line
            && !text.is_empty()
        {
            return text.to_string();
        }
    }
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled");
    let mut chars = stem.replace(['-', '_'], " ").chars().collect::<Vec<_>>();
    if let Some(first) = chars.first_mut() {
        *first = first.to_ascii_uppercase();
    }
    chars.into_iter().collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_is_empty_vec() {
        assert_eq!(parse(""), Vec::new());
    }

    #[test]
    fn blank_lines_are_preserved_not_collapsed() {
        let lines = parse("a\n\n\nb\n");
        assert_eq!(
            lines,
            vec![
                Line::Text("a"),
                Line::Text(""),
                Line::Text(""),
                Line::Text("b"),
            ]
        );
    }

    #[test]
    fn heading_levels() {
        assert_eq!(
            parse("# One").as_slice(),
            [Line::Heading {
                level: 1,
                text: "One"
            }]
        );
        assert_eq!(
            parse("## Two").as_slice(),
            [Line::Heading {
                level: 2,
                text: "Two"
            }]
        );
        assert_eq!(
            parse("### Three").as_slice(),
            [Line::Heading {
                level: 3,
                text: "Three"
            }]
        );
    }

    #[test]
    fn heading_beyond_three_hashes_degrades_to_level_three_with_extra_hashes_as_text() {
        assert_eq!(
            parse("#### Four").as_slice(),
            [Line::Heading {
                level: 3,
                text: "# Four"
            }]
        );
    }

    #[test]
    fn heading_with_no_whitespace() {
        assert_eq!(
            parse("#Title").as_slice(),
            [Line::Heading {
                level: 1,
                text: "Title"
            }]
        );
    }

    #[test]
    fn heading_with_tab() {
        assert_eq!(
            parse("#\tTitle").as_slice(),
            [Line::Heading {
                level: 1,
                text: "Title"
            }]
        );
    }

    #[test]
    fn list_item_requires_the_space() {
        assert_eq!(parse("* item").as_slice(), [Line::ListItem("item")]);
        assert_eq!(
            parse("*item").as_slice(),
            [Line::Text("*item")],
            "no space after * must NOT be a list item"
        );
    }

    #[test]
    fn list_item_bare_marker() {
        assert_eq!(parse("*").as_slice(), [Line::Text("*")]);
        assert_eq!(parse("* ").as_slice(), [Line::ListItem("")]);
    }

    #[test]
    fn quote_needs_no_space() {
        assert_eq!(parse(">text").as_slice(), [Line::Quote("text")]);
        assert_eq!(parse("> text").as_slice(), [Line::Quote(" text")]);
    }

    #[test]
    fn link_with_name() {
        assert_eq!(
            parse("=> gemini://x/ Home").as_slice(),
            [Line::Link {
                url: "gemini://x/",
                name: Some("Home")
            }]
        );
    }

    #[test]
    fn link_without_name() {
        assert_eq!(
            parse("=> gemini://x/").as_slice(),
            [Line::Link {
                url: "gemini://x/",
                name: None
            }]
        );
    }

    #[test]
    fn link_with_tab_and_multiple_spaces() {
        assert_eq!(
            parse("=>\tgemini://x/   Home").as_slice(),
            [Line::Link {
                url: "gemini://x/",
                name: Some("Home")
            }]
        );
    }

    #[test]
    fn link_with_empty_url() {
        assert_eq!(
            parse("=>").as_slice(),
            [Line::Link {
                url: "",
                name: None
            }]
        );
        assert_eq!(
            parse("=> ").as_slice(),
            [Line::Link {
                url: "",
                name: None
            }]
        );
    }

    #[test]
    fn preformat_block_with_alt_text() {
        let lines = parse("```rust\nfn main() {}\n```trailing ignored\nafter\n");
        assert_eq!(
            lines,
            vec![
                Line::Preformatted {
                    alt_text: Some("rust"),
                    lines: vec!["fn main() {}"],
                },
                Line::Text("after"),
            ]
        );
    }

    #[test]
    fn preformat_block_without_alt_text() {
        let lines = parse("```\ncode\n```\n");
        assert_eq!(
            lines,
            vec![Line::Preformatted {
                alt_text: None,
                lines: vec!["code"]
            }]
        );
    }

    #[test]
    fn markers_inside_preformat_are_never_reparsed() {
        let lines = parse("```\n# not a heading\n* not a list\n=> not a link\n```\n");
        assert_eq!(
            lines,
            vec![Line::Preformatted {
                alt_text: None,
                lines: vec!["# not a heading", "* not a list", "=> not a link"],
            }]
        );
    }

    #[test]
    fn unclosed_preformat_block_runs_to_eof_without_panicking() {
        let lines = parse("```\nline one\nline two");
        assert_eq!(
            lines,
            vec![Line::Preformatted {
                alt_text: None,
                lines: vec!["line one", "line two"],
            }]
        );
    }

    #[test]
    fn document_that_is_only_preformatted_content() {
        let lines = parse("```\na\nb\nc\n```\n");
        assert_eq!(lines.len(), 1);
        assert!(matches!(&lines[0], Line::Preformatted { lines, .. } if lines.len() == 3));
    }

    #[test]
    fn non_ascii_text_lines() {
        assert_eq!(
            parse("café \u{1F600}").as_slice(),
            [Line::Text("café \u{1F600}")]
        );
    }

    #[test]
    fn crlf_and_bare_lf_both_split_lines() {
        assert_eq!(
            parse("a\r\nb\n").as_slice(),
            [Line::Text("a"), Line::Text("b")]
        );
    }

    #[test]
    fn leading_bom_is_stripped() {
        assert_eq!(
            parse("\u{feff}# Title").as_slice(),
            [Line::Heading {
                level: 1,
                text: "Title"
            }]
        );
    }

    #[test]
    fn bom_mid_document_is_just_a_character() {
        let lines = parse("a\n\u{feff}b\n");
        assert_eq!(lines, vec![Line::Text("a"), Line::Text("\u{feff}b")]);
    }

    #[test]
    fn mixed_prefix_only_the_first_token_governs() {
        assert_eq!(
            parse("# > * text").as_slice(),
            [Line::Heading {
                level: 1,
                text: "> * text"
            }]
        );
    }

    #[test]
    fn extremely_long_line_does_not_panic() {
        let long = "a".repeat(1_000_000);
        let lines = parse(&long);
        assert_eq!(lines, vec![Line::Text(long.as_str())]);
    }

    #[test]
    fn extract_title_from_heading() {
        let text = "# My Page\n\nbody\n";
        let lines = parse(text);
        assert_eq!(extract_title(&lines, Path::new("whatever.gmi")), "My Page");
    }

    #[test]
    fn extract_title_falls_back_to_filename() {
        let lines = parse("no heading here\n");
        assert_eq!(extract_title(&lines, Path::new("about-us.gmi")), "About us");
    }

    #[test]
    fn extract_title_ignores_deeper_headings() {
        let lines = parse("## Not the title\n# The Title\n");
        assert_eq!(extract_title(&lines, Path::new("x.gmi")), "The Title");
    }

    /// Fuzz entry point: must never panic, on any byte sequence that forms
    /// valid UTF-8 (the fuzz target itself filters non-UTF-8 input, since
    /// content files are read as UTF-8 text). Called from
    /// `fuzz/fuzz_targets/parse_gemtext.rs`.
    #[test]
    fn fuzz_smoke_corpus_never_panics() {
        for input in [
            "",
            "\u{feff}",
            "```",
            "```\n```\n```",
            "#",
            "##",
            "###",
            "####",
            "*",
            "* ",
            ">",
            "=>",
            "=>\t\t\t",
            "\r\n\r\n\r\n",
            "\0",
        ] {
            let _ = parse(input);
        }
    }
}
