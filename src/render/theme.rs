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
pub const THEMES: &[Theme] = &[
    EMBER,
    PHOSPHOR,
    CATHODE,
    BURROW,
    DAYBREAK,
    MIDNIGHT,
    TOKYO_NIGHT,
    PAPER,
];

/// The default theme when none is configured.
pub const DEFAULT_THEME_NAME: &str = EMBER.name;

/// The monospace stack the three house themes use. Iosevka is the
/// project's typeface; it is *named*, never shipped — a capsule that
/// pushed a webfont at every visitor would be contradicting the network
/// it serves. A reader who has Iosevka sees it, everyone else gets their
/// own monospace, and nobody downloads anything.
macro_rules! house_type {
    () => {
        r#"
body {
  font-family: Iosevka, "Iosevka Web", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 1rem;
  line-height: 1.7;
}
"#
    };
}

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
/* Accessibility scaffolding (ADR 0010). The skip link is the first
   focusable element on every page: invisible until focused, then it
   appears at the top so a keyboard or voice user can jump straight past
   anything preceding the content. Focus is always visibly outlined —
   never removed — since a voice or keyboard user has no pointer to tell
   them where they are. */
.skip-link {
  position: absolute;
  left: -9999px;
  top: 0;
  padding: 0.6rem 1rem;
  z-index: 10;
  /* Every bundled theme defines these, and the structural block is
     concatenated after the palette, so they always resolve. Without an
     opaque background the focused link would sit unreadably over the
     content it is meant to skip. */
  background: var(--bg);
  color: var(--fg);
}
.skip-link:focus {
  left: 0;
  outline: 2px solid currentColor;
}
:focus-visible { outline: 2px solid currentColor; outline-offset: 2px; }
main { display: block; }
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

/// **Ember** — the default, and the house scheme. Ember Oxide
/// (`#E67916`) on a near-black warm ground, with its light half
/// ("Foolscap") reached through `prefers-color-scheme`, so a reader's own
/// setting picks and nothing has to be chosen twice.
///
/// The amber is burnt down to `#9A4C05` on the light half deliberately:
/// pure Ember Oxide manages about 2.9:1 against paper white, short of the
/// 4.5:1 body text needs, while the darkened tone clears 6.6:1. On the
/// dark half the pure colour clears 6.3:1 and is used as-is.
const EMBER: Theme = Theme {
    name: "ember",
    description: "Ember Oxide amber on warm near-black, with a light half; monospace throughout",
    css: concat!(
        r#":root {
  color-scheme: light dark;
  --bg: #fbf9f5;
  --fg: #1b1a17;
  --muted: #4a4642;
  --accent: #9a4c05;
  --border: #c9c0b2;
  --code-bg: #f6f0e6;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #14120f;
    --fg: #f2eee6;
    --muted: #bdb2a2;
    --accent: #e67916;
    --border: #4a433a;
    --code-bg: #241f18;
  }
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--fg); }
a { color: var(--accent); }
blockquote { color: var(--muted); border-left-color: var(--accent); }
figcaption { color: var(--muted); }
pre { background: var(--code-bg); border: 1px solid var(--border); }
hr { border-top-color: var(--border); }
"#,
        structure!(),
        house_type!()
    ),
};

/// **Phosphor** — monochrome amber CRT: `#ffb000`, the colour the actual
/// phosphor glowed. Always dark, one hue throughout, and the most
/// committed of the set. Shipped as a choice rather than a default
/// because a whole site in one hue is striking for a page and tiring for
/// an evening.
///
/// No scanline or bloom effect is applied to body text. Those belong on
/// decoration nobody has to read, and a stylesheet cannot tell the
/// difference.
const PHOSPHOR: Theme = Theme {
    name: "phosphor",
    description: "Monochrome amber terminal (#ffb000) on black, always dark; monospace throughout",
    css: concat!(
        r#":root {
  color-scheme: dark;
  --bg: #0b0a06;
  --fg: #ffb000;
  --muted: #c08512;
  --accent: #ffd37a;
  --border: #4a3208;
  --code-bg: #181203;
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--fg); }
a { color: var(--accent); }
blockquote { color: var(--muted); border-left-color: var(--border); }
figcaption { color: var(--muted); }
pre { background: var(--code-bg); border: 1px solid var(--border); }
hr { border-top-color: var(--border); }
"#,
        structure!(),
        house_type!()
    ),
};

/// **Cathode** — the green CRT, and the mascot's own glow. `#3afb26`
/// sampled from `assets/mascot.png`, which is the phosphor-green
/// direction the brand began with before the amber mark superseded it
/// (`docs/internal/notes/branding.md`). Always dark, one hue throughout,
/// the green sibling of Phosphor's amber.
///
/// The body green clears 14.3:1 against the ground and the dimmer
/// `--muted` clears 7.6:1, both measured rather than assumed. Links
/// carry a *lighter* green (`#9dff8e`) that is only 1.1:1 against body
/// text by luminance, which is deliberate and safe here only because
/// the structural rules give every link a border-bottom on hover and
/// focus: colour is never the sole cue. Do not lower that.
const CATHODE: Theme = Theme {
    name: "cathode",
    description: "Monochrome green terminal (#3afb26) on black, always dark — the mascot's glow; monospace throughout",
    css: concat!(
        r#":root {
  color-scheme: dark;
  --bg: #050a04;
  --fg: #3afb26;
  --muted: #29b81c;
  --accent: #9dff8e;
  --border: #123c0d;
  --code-bg: #0a1a08;
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--fg); }
a { color: var(--accent); }
blockquote { color: var(--muted); border-left-color: var(--border); }
figcaption { color: var(--muted); }
pre { background: var(--code-bg); border: 1px solid var(--border); }
hr { border-top-color: var(--border); }
"#,
        structure!(),
        house_type!()
    ),
};

/// **Burrow** — the same amber over soil browns rather than neutral
/// black. The gopher nod, for an operator whose capsule leans that way.
/// Always dark.
const BURROW: Theme = Theme {
    name: "burrow",
    description: "Amber over earth browns, always dark — the gopher register; monospace throughout",
    css: concat!(
        r#":root {
  color-scheme: dark;
  --bg: #1a140e;
  --fg: #efe3d0;
  --muted: #bfa98c;
  --accent: #e9a85a;
  --border: #55432e;
  --code-bg: #2a2016;
}
body { background: var(--bg); color: var(--fg); }
h1, h2, h3 { color: var(--fg); }
a { color: var(--accent); }
blockquote { color: var(--muted); border-left-color: var(--accent); }
figcaption { color: var(--muted); }
pre { background: var(--code-bg); border: 1px solid var(--border); }
hr { border-top-color: var(--border); }
"#,
        structure!(),
        house_type!()
    ),
};

/// **Daybreak** — warm, low-contrast cream/near-black in the
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
    fn cathode_carries_the_mascot_green() {
        // The whole point of this theme is that it matches the mark in
        // assets/mascot.png. If the palette is retuned, sample the image
        // again rather than nudging the hex by eye.
        let cathode = find("cathode").expect("theme exists");
        assert!(
            cathode.css.contains("#3afb26"),
            "cathode must carry the sampled mascot green"
        );
        assert!(
            cathode.css.contains("color-scheme: dark"),
            "cathode is always dark"
        );
    }

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
    fn the_default_is_the_house_scheme() {
        assert_eq!(DEFAULT_THEME_NAME, "ember");
    }

    #[test]
    fn the_house_themes_set_the_house_typeface_after_the_shared_structure() {
        // `structure!()` sets a sans stack for every theme; the house
        // themes override it, so the override has to be concatenated
        // *after* it or the cascade silently discards it. Assert the
        // order rather than merely the presence.
        for name in ["ember", "phosphor", "cathode", "burrow"] {
            let theme = find(name).expect("theme exists");
            let sans = theme
                .css
                .find("BlinkMacSystemFont")
                .expect("shared structure present");
            let mono = theme.css.find("Iosevka").expect("house typeface present");
            assert!(
                mono > sans,
                "{name}: the Iosevka rule must come after the shared sans rule to win the cascade"
            );
        }
    }

    #[test]
    fn the_house_themes_carry_ember_oxide_or_its_documented_variants() {
        // The exact tints matter: pure #e67916 fails contrast on a light
        // ground, so the light half must use the burnt-down tone.
        let ember = find("ember").expect("theme exists");
        assert!(
            ember.css.contains("#e67916"),
            "dark half uses pure Ember Oxide"
        );
        assert!(
            ember.css.contains("#9a4c05"),
            "light half uses the burnt-down tone"
        );
        assert!(
            find("phosphor")
                .expect("theme exists")
                .css
                .contains("#ffb000")
        );
    }

    #[test]
    fn always_dark_themes_do_not_offer_a_light_media_query() {
        for name in ["midnight", "tokyo-night", "phosphor", "burrow"] {
            let theme = find(name).expect("theme exists");
            assert!(
                !theme.css.contains("prefers-color-scheme"),
                "{name} is meant to be always-dark, not adaptive"
            );
        }
    }
}
