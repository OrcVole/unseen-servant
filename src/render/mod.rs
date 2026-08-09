//! The gemtext → HTML/Atom/feeds pipeline (ADR 0004): one content tree,
//! rendered twice. Layering mirrors [`crate::protocol`]'s discipline —
//! each stage is independently testable and fuzzable:
//!
//! 1. **[`gemtext`]** (this phase): the line-type grammar. Identical for
//!    both surfaces — the one module ADR 0004 says must never diverge
//!    between gemtext-native and HTML output. Knows nothing about HTML,
//!    feeds, or files.
//! 2. **`metadata`** (next): walks a parsed document into title/date/feed
//!    facts. Sits between the parser and both emitters so title/date
//!    conventions can't drift between surfaces.
//! 3. **`html`**, **`feed::{atom,gemsub}`**: emitters. Consume parsed
//!    documents plus metadata, produce the two output formats.
//! 4. **`pipeline`**, **`watcher`**: tie the above together per file and
//!    across the whole content tree, own the atomic staging-swap render
//!    and the fs-event debounce. Kept separate from the parser/emitters
//!    the way `server.rs` is separate from `protocol/`: rendering logic
//!    must not know about tokio tasks or fs events.
//!
//! Design questions still open for later stages in this phase are
//! recorded in `docs/notes/c3-render-design-brief.md` §5; each is
//! resolved with a documented, reasonable default at the point its code
//! lands rather than blocking on all of them up front.

pub mod feed;
pub mod gemtext;
pub mod html;
pub mod metadata;
pub mod pipeline;
pub mod robots;
pub mod sitemap;
pub mod skeleton;
pub mod theme;
pub mod watcher;

/// Escape `&`, `<`, `>`, and `"` — sufficient and necessary for XML/HTML
/// text nodes and (double-quoted) attribute values, the only contexts
/// [`html`] and [`feed::atom`] ever write into. Shared here rather than
/// duplicated: the escaping rule is identical in both formats, and having
/// one implementation means a fix or an added-case only ever needs to
/// land once.
pub(crate) fn escape_into(out: &mut String, s: &str) {
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
