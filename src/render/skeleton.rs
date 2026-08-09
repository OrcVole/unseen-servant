//! The first-run content skeleton (director's "beautiful placeholder"
//! note, docs/notes/integration-ideas.md): a fresh capsule with no
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

/// The default mood: gentle and a little warm, and functional — it tells
/// the *owner* exactly what to do next without reading like an error
/// page to a visitor who just wandered by.
pub const QUIET: &str = "\
# Still empty, still here

Some capsules start as a blank page instead of a plan. This is one of \
them — nothing has been written into it yet, and that's a fine way to \
begin.

If you're the one who set this up: your content lives in a folder \
Cloudron gave you, one gemtext file per page. Write something, save it, \
and this notice quietly steps aside — no build, no deploy, no waiting.

If you're a visitor who found this early: there's nothing to apologize \
for. Come back when there's something worth reading, or don't. Either \
way, thanks for looking.

=> gemini://geminiprotocol.net/ What Gemini is, if you're new here

>Unseen Servant
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
