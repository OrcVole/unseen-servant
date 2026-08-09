//! The bundled theme set: classless stylesheets served at a fixed path
//! (`/style.css`, wired in `http.rs`) rather than written to disk on
//! every render. Classless by ADR 0004 — each styles bare elements, so
//! any semantic HTML this crate emits looks right without carrying
//! classes.
//!
//! Each theme is one complete, independent stylesheet, not a palette
//! layered on a shared base — deliberately, so any one theme can be
//! read, reviewed, and reasoned about on its own without tracing which
//! rules from elsewhere it inherits. That costs some duplication of the
//! structural rules (box-sizing, column width, spacing scale) across all
//! four; that cost is judged worth it for independent correctness.

/// A theme's identity plus its stylesheet.
#[derive(Debug)]
pub struct Theme {
    /// The config-facing name (`server.theme = "..."`), lowercase-hyphen.
    pub name: &'static str,
    /// One-line description for a future docs gallery / `usv init`.
    pub description: &'static str,
    /// The stylesheet itself.
    pub css: &'static str,
}

/// All bundled themes, in the order a picker should offer them.
pub const THEMES: &[Theme] = &[DAYBREAK, MIDNIGHT, TOKYO_NIGHT, PAPER];

/// The default theme when none is configured.
pub const DEFAULT_THEME_NAME: &str = DAYBREAK.name;

/// Look up a theme by its config name, case-insensitive. `None` for an
/// unknown name — the caller (config validation) turns that into a
/// startup error rather than silently falling back, matching the
/// project's "unknown keys/values are startup errors" discipline.
pub fn find(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

/// The structural rules every theme repeats verbatim (see the module
/// doc for why this is duplicated per theme rather than shared): column
/// width, type scale, spacing. Kept as one literal copied into each
/// theme's own string below, so grep/diff on any single theme still
/// shows the whole picture.
macro_rules! structure {
    () => {
        r#"
* { box-sizing: border-box; }
body {
  margin: 0 auto;
  max-width: 38rem;
  padding: 3rem 1.5rem 6rem;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 1.05rem;
  line-height: 1.65;
}
h1, h2, h3 { line-height: 1.25; }
h1 { font-size: 1.8rem; margin-top: 2.5rem; }
h2 { font-size: 1.4rem; margin-top: 2rem; }
h3 { font-size: 1.15rem; margin-top: 1.5rem; }
p { margin: 1rem 0; }
a { text-decoration: none; border-bottom: 1px solid transparent; }
a:hover, a:focus { border-bottom-color: currentColor; }
ul { padding-left: 1.4rem; }
li { margin: 0.4rem 0; }
blockquote { margin: 1.5rem 0; padding: 0.2rem 1rem; border-left: 3px solid; }
figure { margin: 1.5rem 0; }
figcaption { font-size: 0.85rem; margin-bottom: 0.4rem; }
pre { padding: 1rem; overflow-x: auto; border-radius: 6px; font-size: 0.9rem; line-height: 1.5; }
hr { border: none; border-top: 1px solid; margin: 2.5rem 0; }
"#
    };
}

/// **Daybreak** — the default. Warm, low-contrast cream/near-black in the
/// smolweb's understated register, with `prefers-color-scheme` support
/// built in so a system-dark reader gets a matching (still warm, not
/// stark) dark variant automatically. The adaptive one: pick this if you
/// want the page to follow the reader's own light/dark preference.
const DAYBREAK: Theme = Theme {
    name: "daybreak",
    description: "Warm cream/near-black, adapts to the reader's light/dark preference",
    css: concat!(
        r#":root {
  color-scheme: light dark;
  --bg: #faf7f2;
  --fg: #2a2622;
  --muted: #6b6259;
  --accent: #a8562f;
  --border: #e4ddd0;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1c1a17;
    --fg: #e8e2d8;
    --muted: #a89d8d;
    --accent: #e2955f;
    --border: #3a352e;
  }
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--fg); }
a { color: var(--accent); }
blockquote { color: var(--muted); border-left-color: var(--border); }
figcaption { color: var(--muted); }
pre { background: rgba(128, 110, 90, 0.08); }
hr { border-top-color: var(--border); }
"#,
        structure!()
    ),
};

/// **Midnight** — Daybreak's dark half, forced on regardless of the
/// reader's system preference. For an operator who wants their capsule
/// to always read dark, not just when the visitor's OS says so.
const MIDNIGHT: Theme = Theme {
    name: "midnight",
    description: "Daybreak's warm dark palette, always on — night mode without the auto-switch",
    css: concat!(
        r#":root {
  color-scheme: dark;
  --bg: #1c1a17;
  --fg: #e8e2d8;
  --muted: #a89d8d;
  --accent: #e2955f;
  --border: #3a352e;
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--fg); }
a { color: var(--accent); }
blockquote { color: var(--muted); border-left-color: var(--border); }
figcaption { color: var(--muted); }
pre { background: rgba(255, 255, 255, 0.05); }
hr { border-top-color: var(--border); }
"#,
        structure!()
    ),
};

/// **Tokyo Night** — the well-known deep-navy/purple palette (the same
/// family as the popular editor theme of the same name), always dark.
/// For an operator who wants a specific, recognizable aesthetic rather
/// than Daybreak/Midnight's warm neutrality.
const TOKYO_NIGHT: Theme = Theme {
    name: "tokyo-night",
    description: "Deep navy/purple, always dark — the well-known Tokyo Night palette",
    css: concat!(
        r#":root {
  color-scheme: dark;
  --bg: #1a1b26;
  --fg: #c0caf5;
  --muted: #565f89;
  --accent: #7aa2f7;
  --accent2: #bb9af7;
  --border: #292e42;
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--accent2); }
a { color: var(--accent); }
blockquote { color: var(--muted); border-left-color: var(--border); }
figcaption { color: var(--muted); }
pre { background: rgba(122, 162, 247, 0.08); }
hr { border-top-color: var(--border); }
"#,
        structure!()
    ),
};

/// **Paper** — the minimal option: near-white, near-black, one muted
/// grey, no color accent, nothing decorative. For an operator who finds
/// even Daybreak's warmth too much personality.
const PAPER: Theme = Theme {
    name: "paper",
    description: "Plain black on white (or white on black), no accent color, nothing decorative",
    css: concat!(
        r#":root {
  color-scheme: light dark;
  --bg: #ffffff;
  --fg: #111111;
  --muted: #666666;
  --border: #dddddd;
}
@media (prefers-color-scheme: dark) {
  :root { --bg: #111111; --fg: #eeeeee; --muted: #999999; --border: #333333; }
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--fg); }
a { color: var(--fg); border-bottom: 1px solid var(--muted); }
blockquote { color: var(--muted); border-left-color: var(--border); }
figcaption { color: var(--muted); }
pre { background: transparent; border: 1px solid var(--border); }
hr { border-top-color: var(--border); }
"#,
        structure!()
    ),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_defines_color_scheme() {
        for theme in THEMES {
            assert!(
                theme.css.contains("color-scheme"),
                "{} must set color-scheme",
                theme.name
            );
        }
    }

    #[test]
    fn every_theme_includes_the_shared_structure() {
        for theme in THEMES {
            assert!(
                theme.css.contains("max-width: 38rem"),
                "{} must include the shared structural rules",
                theme.name
            );
        }
    }

    #[test]
    fn theme_names_are_unique() {
        let mut names: Vec<&str> = THEMES.iter().map(|t| t.name).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "theme names must be unique");
    }

    #[test]
    fn find_is_case_insensitive() {
        assert!(find("daybreak").is_some());
        assert!(find("DAYBREAK").is_some());
        assert!(find("Tokyo-Night").is_some());
    }

    #[test]
    fn find_unknown_theme_is_none() {
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn default_theme_name_is_a_real_theme() {
        assert!(find(DEFAULT_THEME_NAME).is_some());
    }

    #[test]
    fn always_dark_themes_do_not_offer_a_light_media_query() {
        for name in ["midnight", "tokyo-night"] {
            let theme = find(name).expect("theme exists");
            assert!(
                !theme.css.contains("prefers-color-scheme"),
                "{name} is meant to be always-dark, not adaptive"
            );
        }
    }
}
