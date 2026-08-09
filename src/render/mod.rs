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

pub mod gemtext;
pub mod metadata;
