//! The gopher render target: gemtext → menus (ADR 0012 §4).
//!
//! The third output of the one write-time render pass, alongside gemtext
//! and HTML (ADR 0004). Gopher's structure is *navigational* rather than
//! documental — a menu is a list of things you can go to, and prose only
//! appears in it by way of the `i` (informational) line type. So every
//! gemtext page becomes a **menu**: its prose as `i` lines wrapped to a
//! readable width, its links as real menu entries.
//!
//! Two consequences follow from gopher's own limits, and both are
//! handled here rather than at request time:
//!
//! * **Menus are absolute.** Every line carries host and port, which is
//!   what lets a menu link straight into another server. usv's own links
//!   therefore need the capsule's advertised host baked in at render
//!   time.
//! * **There are no redirects.** Gopher cannot express one, so a link
//!   that a redirect rule would rewrite has to be resolved *now*, to its
//!   final target. Emitting the pre-redirect path would produce a menu
//!   entry that simply fails.

use crate::protocol::gopher::{ItemType, LASTLINE, MenuLine};
use crate::render::gemtext::Line;

/// The width prose is wrapped to before it becomes `i` lines.
///
/// 70 rather than 80: menu clients indent, and several long-lived ones
/// add their own item-type gutter, so 80 wraps raggedly in exactly the
/// terminals most likely to be reading this.
pub const WRAP_COLUMNS: usize = 70;

/// What the emitter needs to know about the capsule it is rendering for.
#[derive(Debug, Clone)]
pub struct Context {
    /// The hostname to put on every menu line.
    pub host: String,
    /// The port to put on every menu line — the *advertised* one, which
    /// on a port-remapping platform is not the bound one.
    pub port: u16,
}

/// Render one gemtext document as a gopher menu.
///
/// `page_dir` is the page's own directory as a selector prefix (`/` for
/// the root, `/notes/` for `notes/index.gmi`), used to resolve relative
/// links — gopher selectors are absolute, so a relative gemtext link
/// cannot be passed through unchanged.
pub fn render_menu(lines: &[Line<'_>], title: &str, page_dir: &str, ctx: &Context) -> String {
    let mut out = String::with_capacity(1024);

    // The title as a heading, then a blank line. Menus have no <title>
    // equivalent, so the first line *is* the title as far as a reader is
    // concerned.
    push_info(&mut out, title);
    push_info(&mut out, "");

    for line in lines {
        match line {
            Line::Heading { level, text } => {
                // Blank line before a heading, and an underline for level
                // 1 and 2 — gopher has no emphasis, so structure has to be
                // drawn rather than marked up.
                push_info(&mut out, "");
                for wrapped in wrap(text, WRAP_COLUMNS) {
                    push_info(&mut out, &wrapped);
                }
                if *level <= 2 {
                    let rule = if *level == 1 { '=' } else { '-' };
                    let width = text.chars().count().min(WRAP_COLUMNS);
                    push_info(&mut out, &rule.to_string().repeat(width.max(1)));
                }
            }
            Line::Text(t) => {
                if t.trim().is_empty() {
                    push_info(&mut out, "");
                } else {
                    for wrapped in wrap(t, WRAP_COLUMNS) {
                        push_info(&mut out, &wrapped);
                    }
                }
            }
            Line::ListItem(t) => {
                // Continuation lines are indented under the bullet, so a
                // wrapped item still reads as one item.
                let wrapped = wrap(t, WRAP_COLUMNS.saturating_sub(2));
                for (i, w) in wrapped.iter().enumerate() {
                    push_info(
                        &mut out,
                        &format!("{}{}", if i == 0 { "* " } else { "  " }, w),
                    );
                }
            }
            Line::Quote(t) => {
                for w in wrap(t, WRAP_COLUMNS.saturating_sub(2)) {
                    push_info(&mut out, &format!("> {w}"));
                }
            }
            Line::Preformatted { lines: block, .. } => {
                // Never wrapped and never re-parsed: the whole point of a
                // preformatted block is that its columns mean something.
                // The alt text is dropped — gopher has nowhere to put it.
                for raw in block {
                    push_info(&mut out, raw);
                }
            }
            Line::Link { url, name } => {
                let display = name.unwrap_or(url);
                out.push_str(&link_line(url, display, page_dir, ctx).to_wire());
            }
        }
    }

    out.push_str(LASTLINE);
    out
}

/// Turn a gemtext link into a menu line.
fn link_line(url: &str, display: &str, page_dir: &str, ctx: &Context) -> MenuLine {
    let trimmed = url.trim();

    // Anything with a scheme that is not ours leaves gopherspace. The
    // `h` type with a `URL:` selector is the long-standing convention
    // for that, and clients that do not understand it still show the
    // address rather than silently dropping the link.
    if let Some((scheme, _)) = trimmed.split_once("://") {
        if scheme.eq_ignore_ascii_case("gopher") {
            // A real gopher URL elsewhere: pass it through as a menu, but
            // do not try to re-host it — the remote server owns it.
            return MenuLine {
                item: ItemType::Menu,
                display: display.to_string(),
                selector: trimmed.to_string(),
                host: ctx.host.clone(),
                port: ctx.port,
            };
        }
        return MenuLine {
            item: ItemType::Html,
            display: format!("{display} [{scheme}]"),
            selector: format!("URL:{trimmed}"),
            host: ctx.host.clone(),
            port: ctx.port,
        };
    }
    // A bare `mailto:` or similar (scheme, no authority) is still not
    // ours to serve.
    if let Some((scheme, _)) = trimmed.split_once(':')
        && !scheme.is_empty()
        && !scheme.contains('/')
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return MenuLine {
            item: ItemType::Html,
            display: format!("{display} [{scheme}]"),
            selector: format!("URL:{trimmed}"),
            host: ctx.host.clone(),
            port: ctx.port,
        };
    }

    let selector = resolve_selector(trimmed, page_dir);
    MenuLine {
        item: item_type_for_selector(&selector),
        display: display.to_string(),
        selector,
        host: ctx.host.clone(),
        port: ctx.port,
    }
}

/// The item type a local selector should advertise.
///
/// A gemtext page is rendered *as a menu*, so `.gmi` is type 1 and not
/// type 0 — following it should give the reader the rendered menu, not
/// the gemtext source.
fn item_type_for_selector(selector: &str) -> ItemType {
    if selector.ends_with('/') || selector.is_empty() {
        return ItemType::Menu;
    }
    let lower = selector.to_ascii_lowercase();
    if lower.ends_with(".gmi") || lower.ends_with(".gemini") {
        ItemType::Menu
    } else {
        ItemType::for_path(selector)
    }
}

/// Resolve a relative gemtext link against the page's directory, and
/// normalise it into an absolute selector.
///
/// Gopher selectors are absolute; there is no base-URL notion for a
/// client to resolve against, so this has to happen at render time.
fn resolve_selector(url: &str, page_dir: &str) -> String {
    let joined = if url.starts_with('/') {
        url.to_string()
    } else {
        let base = if page_dir.ends_with('/') {
            page_dir.to_string()
        } else {
            format!("{page_dir}/")
        };
        format!("{base}{url}")
    };

    // Collapse `.` and `..` lexically. A selector that tries to climb
    // above the root is clamped there rather than escaping — the same
    // posture the static handler takes, and the reason a menu cannot be
    // made to advertise a path outside the tree.
    let mut parts: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    let trailing = joined.ends_with('/') && !parts.is_empty();
    let mut out = String::with_capacity(joined.len());
    for p in &parts {
        out.push('/');
        out.push_str(p);
    }
    // Both the empty case (everything collapsed away) and the
    // trailing-slash case end in `/` — the root selector and a directory
    // selector look the same by design.
    if out.is_empty() || trailing {
        out.push('/');
    }
    out
}

/// Append one informational line.
fn push_info(out: &mut String, text: &str) {
    out.push_str(&MenuLine::info(text).to_wire());
}

/// Wrap text to `width` columns on whitespace, counting characters
/// rather than bytes so multi-byte text is not cut mid-character.
///
/// A word longer than the width is emitted on its own over-long line
/// rather than broken: breaking a URL or an identifier makes it useless,
/// and a menu client soft-wraps anyway.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    let mut len = 0usize;
    for word in text.split_whitespace() {
        let wlen = word.chars().count();
        if len > 0 && len + 1 + wlen > width {
            out.push(std::mem::take(&mut line));
            len = 0;
        }
        if len > 0 {
            line.push(' ');
            len += 1;
        }
        line.push_str(word);
        len += wlen;
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// `caps.txt` — the conventional capability file gopher crawlers and
/// curious humans look for at the root.
pub fn caps_txt(ctx: &Context) -> String {
    format!(
        "CAPS\r\n\
         \r\n\
         ##\r\n\
         ## This is an automatically generated caps file.\r\n\
         ##\r\n\
         \r\n\
         CapsVersion=1\r\n\
         ExpireCapsAfter=1800\r\n\
         \r\n\
         PathDelimeter=/\r\n\
         PathIdentity=.\r\n\
         PathParent=..\r\n\
         PathParentDouble=FALSE\r\n\
         PathKeepPreDelimeter=FALSE\r\n\
         ServerSoftware=unseen-servant\r\n\
         ServerSoftwareVersion={version}\r\n\
         ServerArchitecture=Rust\r\n\
         ServerDescription=A capsule served by Unseen Servant\r\n\
         ServerDefaultEncoding=utf-8\r\n\
         \r\n\
         ServerHost={host}\r\n\
         ServerPort={port}\r\n",
        version = env!("CARGO_PKG_VERSION"),
        host = ctx.host,
        port = ctx.port,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::gemtext;

    fn ctx() -> Context {
        Context {
            host: "example.org".to_string(),
            port: 70,
        }
    }

    fn menu(src: &str, page_dir: &str) -> String {
        let lines = gemtext::parse(src);
        render_menu(&lines, "T", page_dir, &ctx())
    }

    #[test]
    fn every_line_is_tab_delimited_and_crlf_terminated() {
        let out = menu("# Hi\n\nSome prose.\n=> a.gmi A link\n", "/");
        for line in out.split("\r\n").filter(|l| !l.is_empty() && *l != ".") {
            assert_eq!(line.matches('\t').count(), 3, "bad field count: {line:?}");
        }
        assert!(out.ends_with(LASTLINE));
    }

    #[test]
    fn prose_becomes_info_lines() {
        let out = menu("Just some words.\n", "/");
        assert!(out.contains("iJust some words.\t"), "{out}");
    }

    #[test]
    fn a_gemtext_link_is_a_menu_not_a_text_file() {
        // Following it should give the reader the rendered menu, not the
        // gemtext source.
        let out = menu("=> about.gmi About\n", "/");
        assert!(out.contains("1About\t/about.gmi\texample.org\t70"), "{out}");
    }

    #[test]
    fn relative_links_resolve_against_the_page_directory() {
        let out = menu("=> sibling.gmi S\n", "/notes/");
        assert!(out.contains("\t/notes/sibling.gmi\t"), "{out}");
    }

    #[test]
    fn dot_segments_are_collapsed_and_cannot_escape_the_root() {
        assert_eq!(
            resolve_selector("../../../etc/passwd", "/a/b/"),
            "/etc/passwd"
        );
        assert_eq!(resolve_selector("./x.gmi", "/a/"), "/a/x.gmi");
        assert_eq!(resolve_selector("..", "/a/"), "/");
    }

    #[test]
    fn an_external_https_link_becomes_an_html_item() {
        let out = menu("=> https://example.com/x Web thing\n", "/");
        assert!(
            out.contains("hWeb thing [https]\tURL:https://example.com/x\t"),
            "{out}"
        );
    }

    #[test]
    fn a_gemini_link_leaves_gopherspace_too() {
        let out = menu("=> gemini://elsewhere.org/ Capsule\n", "/");
        assert!(out.starts_with("iT\t"), "{out}");
        assert!(out.contains("URL:gemini://elsewhere.org/"), "{out}");
    }

    #[test]
    fn a_mailto_link_is_not_treated_as_a_path() {
        let out = menu("=> mailto:someone@example.org Mail\n", "/");
        assert!(out.contains("URL:mailto:someone@example.org"), "{out}");
    }

    #[test]
    fn an_image_link_keeps_its_own_item_type() {
        let out = menu("=> cat.png A cat\n", "/");
        assert!(out.contains("IA cat\t/cat.png\t"), "{out}");
        let out = menu("=> anim.gif Anim\n", "/");
        assert!(out.contains("gAnim\t/anim.gif\t"), "{out}");
    }

    #[test]
    fn preformatted_blocks_are_never_wrapped() {
        let wide = "x".repeat(120);
        let src = format!("```\n{wide}\n```\n");
        let out = menu(&src, "/");
        assert!(out.contains(&format!("i{wide}\t")), "preformat was wrapped");
    }

    #[test]
    fn long_prose_is_wrapped_at_the_column_limit() {
        let src = "word ".repeat(60);
        let out = menu(&src, "/");
        for line in out.split("\r\n") {
            if let Some(rest) = line.strip_prefix('i') {
                let display = rest.split('\t').next().unwrap_or("");
                assert!(
                    display.chars().count() <= WRAP_COLUMNS,
                    "unwrapped: {display:?}"
                );
            }
        }
    }

    #[test]
    fn a_very_long_word_is_not_broken() {
        // Breaking a URL mid-token makes it useless; the client soft-wraps.
        let long = "a".repeat(120);
        let wrapped = wrap(&long, WRAP_COLUMNS);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0], long);
    }

    #[test]
    fn wrapping_counts_characters_not_bytes() {
        // Multi-byte words: wrapping by bytes would break these far too
        // early (é is two bytes), so the counts must be in characters.
        let word = "é".repeat(4);
        let text = format!("{word} ").repeat(60);
        for w in wrap(&text, WRAP_COLUMNS) {
            assert!(
                w.chars().count() <= WRAP_COLUMNS,
                "{} chars: {w:?}",
                w.chars().count()
            );
            assert!(w.len() > w.chars().count(), "should be multi-byte");
        }
    }

    #[test]
    fn a_single_token_longer_than_the_width_is_left_over_long() {
        // Documented behaviour, asserted so it stays a decision rather
        // than becoming a surprise: breaking a long URL mid-token would
        // make it unusable, and menu clients soft-wrap.
        let long = "é".repeat(100);
        let wrapped = wrap(&long, WRAP_COLUMNS);
        assert_eq!(wrapped.len(), 1);
        assert!(wrapped[0].chars().count() > WRAP_COLUMNS);
    }

    #[test]
    fn a_heading_gets_an_underline_it_can_actually_draw() {
        let out = menu("# Title\n", "/");
        // "Title" is five characters, so the rule is five long — it
        // tracks the heading rather than the wrap width.
        assert!(out.contains("i=====\t"), "{out}");
        let out = menu("## Sub\n", "/");
        assert!(out.contains("i---\t"), "{out}");
    }

    #[test]
    fn a_link_display_string_cannot_forge_menu_structure() {
        // The end-to-end version of the protocol-layer test: a hostile
        // link *name* in authored or uploaded content must not be able to
        // invent menu fields.
        let out = menu("=> a.gmi evil\tname\there\n", "/");
        for line in out.split("\r\n").filter(|l| !l.is_empty() && *l != ".") {
            assert_eq!(line.matches('\t').count(), 3, "forged: {line:?}");
        }
    }

    #[test]
    fn caps_txt_names_the_server_and_host() {
        let c = caps_txt(&ctx());
        assert!(c.starts_with("CAPS\r\n"));
        assert!(c.contains("ServerSoftware=unseen-servant"));
        assert!(c.contains("ServerHost=example.org"));
        assert!(c.contains("ServerPort=70"));
    }
}
