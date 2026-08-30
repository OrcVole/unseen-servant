//! Rewriting gemtext link targets for the rendered surfaces.
//!
//! A capsule is written in gemtext and links to gemtext: `=> about.gmi`.
//! The render pass writes each `page.gmi` as `page.html` and `page.md`
//! (see `pipeline::render_page`), so a link copied through unchanged
//! points at a file that surface does not serve. Every internal link on
//! the web mirror was a 404 for that reason, found by fetching the live
//! site on 2026-08-30 rather than by reading the emitter: the HTML was
//! valid, the links were well-formed, and every one of them was wrong.
//!
//! This is the same defect class as the `llms.txt` fix of 2026-08-11,
//! which had linked `.html` from a file whose whole purpose was to hand
//! back Markdown. Both are a rendered surface citing a *sibling* by the
//! source's name. The rule is written once here and used by both
//! emitters so the third surface cannot get it wrong on its own.
//!
//! Only relative links are touched. An absolute URL is left exactly as
//! written: `gemini://example.com/page.gmi` on the web mirror is a
//! deliberate pointer at the Gemini surface, not a broken local link,
//! and rewriting it would silently retarget it.

/// Rewrite a gemtext link target to its sibling in a rendered surface,
/// where `extension` is that surface's file extension (`html`, `md`).
///
/// Relative targets ending in `.gmi` gain the new extension; query and
/// fragment survive. Everything else — absolute URLs, directory links,
/// anchors, anything with another extension — is returned unchanged,
/// because a link this function does not understand is safer left alone
/// than guessed at.
pub fn sibling(url: &str, extension: &str) -> String {
    if has_scheme(url) || url.starts_with("//") {
        return url.to_string();
    }
    // Split the path from the first query or fragment marker, whichever
    // comes first, so `page.gmi#section` and `page.gmi?q=1` both keep
    // their tail.
    let split = url.find(['?', '#']).unwrap_or(url.len());
    let (path, tail) = url.split_at(split);
    match path.strip_suffix(".gmi") {
        Some(stem) => format!("{stem}.{extension}{tail}"),
        None => url.to_string(),
    }
}

/// Whether a URL begins with a scheme (`https:`, `gemini:`, `mailto:`).
/// Per RFC 3986 a scheme is a letter followed by letters, digits, `+`,
/// `-` or `.`, then a colon. Checked by hand rather than with a URL
/// parser because a link line may hold anything an author typed, and a
/// parse failure here must mean "leave it alone", not an error.
fn has_scheme(url: &str) -> bool {
    let mut chars = url.char_indices();
    match chars.next() {
        Some((_, c)) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    for (_, c) in chars {
        match c {
            ':' => return true,
            c if c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.') => {}
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_gemtext_link_gains_the_surface_extension() {
        assert_eq!(sibling("about.gmi", "html"), "about.html");
        assert_eq!(sibling("about.gmi", "md"), "about.md");
        assert_eq!(sibling("docs/faq.gmi", "html"), "docs/faq.html");
        assert_eq!(sibling("/smolnets.gmi", "html"), "/smolnets.html");
        assert_eq!(sibling("../index.gmi", "html"), "../index.html");
    }

    #[test]
    fn query_and_fragment_survive() {
        assert_eq!(sibling("faq.gmi#tofu", "html"), "faq.html#tofu");
        assert_eq!(sibling("s.gmi?q=1", "html"), "s.html?q=1");
        assert_eq!(sibling("s.gmi?q=1#x", "html"), "s.html?q=1#x");
    }

    #[test]
    fn absolute_urls_are_never_rewritten() {
        // The point of the guard: a gemini:// link on the web mirror is
        // a pointer at the other surface, and must stay one.
        for url in [
            "gemini://example.com/page.gmi",
            "https://example.com/page.gmi",
            "spartan://example.com/x.gmi",
            "mailto:someone@example.com",
        ] {
            assert_eq!(sibling(url, "html"), url);
        }
    }

    #[test]
    fn protocol_relative_urls_are_left_alone() {
        assert_eq!(
            sibling("//example.com/p.gmi", "html"),
            "//example.com/p.gmi"
        );
    }

    #[test]
    fn anything_without_the_gemtext_extension_is_unchanged() {
        for url in [
            "/usv",
            "docs/",
            "style.css",
            "image.png",
            "notes.gmi.bak",
            "",
        ] {
            assert_eq!(sibling(url, "html"), url);
        }
    }

    #[test]
    fn a_bare_fragment_or_query_is_unchanged() {
        assert_eq!(sibling("#content", "html"), "#content");
        assert_eq!(sibling("?q=1", "html"), "?q=1");
    }

    #[test]
    fn scheme_detection_does_not_fire_on_a_colon_later_in_a_path() {
        // A colon can appear in a path segment; only a leading scheme
        // counts, and the guard must not swallow a real rewrite.
        assert_eq!(sibling("odd:name/page.gmi", "html"), "odd:name/page.gmi");
        assert_eq!(sibling("dir/odd:name.gmi", "html"), "dir/odd:name.html");
    }
}
