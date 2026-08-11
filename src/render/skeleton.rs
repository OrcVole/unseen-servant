//! The first-run content skeleton (director's "beautiful placeholder"
//! note, docs/internal/notes/integration-ideas.md): a fresh capsule with no
//! authored content is a normal state (ADR 0008), and the page a visitor
//! or the owner sees there should be *gorgeous by default*, not a techy
//! test page. Three moods, matching the brief's "offer a few stock
//! moods: nothing here yet, under construction, a minimal card":
//! [`QUIET`] (the default — warm, and functional for the owner), plus
//! [`UNDER_CONSTRUCTION`] and [`CALLING_CARD`] as alternatives a future
//! config or `usv init` wizard (C5) can offer a choice between.
//!
//! Each is a complete, valid gemtext document — served as-is on Gemini,
//! and through the normal render pipeline for the HTML surface, so the
//! placeholder is never a special case for either surface. Each opens
//! with a real-words heading rather than a bare glyph or punctuation
//! mark: the heading becomes the page `<title>` and a feed/history
//! entry, both places where "·" or "—" alone would be useless.

/// The default mood: a warm, richly-formatted welcome that doubles as a
/// live demonstration of the renderer — it exercises every gemtext line
/// type (heading levels, prose, quote, list, links, a preformatted
/// block) so the fresh capsule looks considered on both surfaces, and
/// stays functional for the owner (it says, in plain words, where
/// content lives and how to add it).
pub const QUIET: &str = "\
# Welcome to a new capsule

You have reached a capsule served by Unseen Servant — freshly installed, \
and waiting for its first words. Nothing has been written here yet, so \
for the moment there is only this page, and a quiet invitation.

> Small pages, plainly made, and kept for a long time.

## If this capsule is yours

Your writing lives in one content folder, a single gemtext file per \
page. Add a file, save it, and it appears here within moments — served \
natively to Gemini clients and rendered to the web at the same instant, \
from the very same source. No build step. No deploy. No waiting.

Nearly the whole of the format is three kinds of line:

* a heading, like the ones set across this page
* a link, each given a line of its own
* a plain paragraph of prose, as long as you please

Here is what a small page looks like, written out in full:

```an example gemtext page
# A heading

A paragraph of prose. Write as much or as little as suits you.

=> gemini://example.org   A link, on a line of its own
```

That is genuinely most of it. You can hold the entire format in your \
head, which is rather the point.

## A few good places to begin

=> gemini://geminiprotocol.net/ The Gemini protocol, gently explained
=> gemini://geminiprotocol.net/docs/cheatsheet.gmi The one-page gemtext cheatsheet
=> gemini://geminiprotocol.net/software/ A field of clients to wander with
=> /usv What usv is, and what else this capsule answers on

## For a visitor who arrived early

There is nothing here yet, and that is perfectly alright — you have \
simply come before the first post. Wander back when there is something \
to read, or don't; either way, thank you for looking in.

> Unseen Servant
";

/// A lighter, more playful mood for an operator who wants their capsule
/// to say "I'm actively building this" rather than "I haven't started."
pub const UNDER_CONSTRUCTION: &str = "\
# Under construction

Behind this page: someone deciding what to say, and taking their time \
about it.

That's the whole update. There's a content folder waiting, and the \
moment something gets written into it, this notice steps out of the \
way on its own.

>Unseen Servant
";

/// The most restrained mood — a blank calling card, for an operator who
/// wants a bare domain to say something rather than nothing, without
/// saying much.
pub const CALLING_CARD: &str = "\
# A quiet capsule

Reserved. Nothing written yet.

>Unseen Servant
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::gemtext;

    fn assert_valid_gemtext_document(text: &str) {
        let lines = gemtext::parse(text);
        assert!(!lines.is_empty(), "skeleton must not be empty");
        assert!(
            matches!(lines.first(), Some(gemtext::Line::Heading { level: 1, .. })),
            "skeleton should open with a level-1 heading (it becomes the page title)"
        );
    }

    #[test]
    fn all_moods_are_valid_nonempty_gemtext() {
        for mood in [QUIET, UNDER_CONSTRUCTION, CALLING_CARD] {
            assert_valid_gemtext_document(mood);
        }
    }

    #[test]
    fn quiet_mentions_the_content_folder() {
        // The default mood must be functional for the owner, not just
        // charming — it's the one thing a fresh install actually shows.
        assert!(QUIET.contains("content"));
    }

    #[test]
    fn every_mood_has_a_real_words_title_not_a_bare_glyph() {
        // The opening heading becomes the HTML <title> and any feed
        // entry — a single punctuation mark there is useless in a
        // browser tab or a history list, however minimal it looks in
        // the body.
        for mood in [QUIET, UNDER_CONSTRUCTION, CALLING_CARD] {
            let lines = gemtext::parse(mood);
            let title = gemtext::extract_title(&lines, std::path::Path::new("index.gmi"));
            assert!(
                title.chars().filter(|c| c.is_alphabetic()).count() >= 3,
                "title {title:?} for mood {mood:?} should be real words"
            );
        }
    }
}
