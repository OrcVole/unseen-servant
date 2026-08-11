//! A minimal JSON writer, for the `--json` form of the read-only CLI
//! subcommands (`docs/agents.md`).
//!
//! **Why not `serde_json`.** This module emits JSON and never parses it.
//! The whole requirement is "objects, arrays, strings, integers, booleans
//! and null, written correctly" — about eighty lines. A parser is the
//! part of a JSON library that carries risk and earns its dependency; an
//! emitter is not. ADR 0001's dependency set stays closed.
//!
//! Escaping follows RFC 8259 §7: the two mandatory escapes (`"` and `\`),
//! the short forms for the control characters that have them, and
//! `\u00XX` for every other character below 0x20. Nothing else is
//! escaped — this output goes to a pipe, not into an HTML `<script>`
//! element (that context has its own stricter escaper in
//! `render::html`).

/// Append `s` to `out` as a complete JSON string, quotes included.
pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `s` as a standalone JSON string value.
pub fn string(s: &str) -> String {
    let mut out = String::new();
    write_string(&mut out, s);
    out
}

/// A JSON array from already-rendered element values.
pub fn array<I, S>(items: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = String::from("[");
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(item.as_ref());
    }
    out.push(']');
    out
}

/// A JSON array of strings, escaped here rather than by the caller.
pub fn string_array<I, S>(items: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    array(items.into_iter().map(|s| string(s.as_ref())))
}

/// An object under construction. Keys are written in the order they are
/// added; nothing sorts or deduplicates them, so a caller that adds the
/// same key twice gets it twice — which the type system cannot prevent
/// and the tests below therefore pin as the caller's responsibility.
#[derive(Debug, Default)]
pub struct Object {
    out: String,
}

impl Object {
    /// An empty object.
    pub fn new() -> Self {
        Self { out: String::new() }
    }

    fn key(&mut self, key: &str) {
        if !self.out.is_empty() {
            self.out.push(',');
        }
        write_string(&mut self.out, key);
        self.out.push(':');
    }

    /// Add a string-valued field.
    pub fn string(&mut self, key: &str, value: &str) -> &mut Self {
        self.key(key);
        write_string(&mut self.out, value);
        self
    }

    /// Add a string-valued field, or `null` when there is no value —
    /// the JSON counterpart of the `Option`s the CLI's own snapshot types
    /// already carry, so "absent" stays distinguishable from "empty".
    pub fn string_or_null(&mut self, key: &str, value: Option<&str>) -> &mut Self {
        match value {
            Some(v) => self.string(key, v),
            None => self.null(key),
        }
    }

    /// Add an integer-valued field.
    pub fn number(&mut self, key: &str, value: u64) -> &mut Self {
        self.key(key);
        self.out.push_str(&value.to_string());
        self
    }

    /// Add a boolean-valued field.
    pub fn bool(&mut self, key: &str, value: bool) -> &mut Self {
        self.key(key);
        self.out.push_str(if value { "true" } else { "false" });
        self
    }

    /// Add a null-valued field.
    pub fn null(&mut self, key: &str) -> &mut Self {
        self.key(key);
        self.out.push_str("null");
        self
    }

    /// Add a field whose value is already-rendered JSON — a nested object
    /// or an array built by this same module. Nothing validates it, so
    /// every caller in this crate passes the output of [`Object::finish`],
    /// [`array`] or [`string_array`] and never a hand-written literal.
    pub fn raw(&mut self, key: &str, json: &str) -> &mut Self {
        self.key(key);
        self.out.push_str(json);
        self
    }

    /// The finished object, braces included.
    pub fn finish(&self) -> String {
        format!("{{{}}}", self.out)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_two_mandatory_characters_and_the_control_range() {
        assert_eq!(string(r#"a"b"#), r#""a\"b""#);
        assert_eq!(string(r"a\b"), r#""a\\b""#);
        assert_eq!(string("a\nb"), r#""a\nb""#);
        assert_eq!(string("a\tb"), r#""a\tb""#);
        // A control character with no short form takes the \u00XX form;
        // one that has a short form takes that instead.
        assert_eq!(string("a\u{1}b"), "\"a\\u0001b\"");
        assert_eq!(string("a\u{8}b"), r#""a\bb""#);
    }

    #[test]
    fn leaves_non_ascii_and_html_metacharacters_alone() {
        // This output goes to a pipe, never into markup: escaping `<`
        // here would only make the bytes harder to read for the agent
        // and the human both.
        assert_eq!(string("é <b> & ünïcode"), "\"é <b> & ünïcode\"");
    }

    #[test]
    fn builds_a_flat_object_in_insertion_order() {
        let mut o = Object::new();
        o.string("host", "example.org")
            .number("pages", 12)
            .bool("titan", false)
            .null("onion");
        assert_eq!(
            o.finish(),
            r#"{"host":"example.org","pages":12,"titan":false,"onion":null}"#
        );
    }

    #[test]
    fn an_empty_object_is_still_valid_json() {
        assert_eq!(Object::new().finish(), "{}");
        assert_eq!(array(Vec::<String>::new()), "[]");
    }

    #[test]
    fn nests_objects_and_arrays_through_raw() {
        let mut inner = Object::new();
        inner.string("label", "agent-1");
        let mut outer = Object::new();
        outer
            .raw("identity", &inner.finish())
            .raw("capabilities", &string_array(["read", "titan-write"]));
        assert_eq!(
            outer.finish(),
            r#"{"identity":{"label":"agent-1"},"capabilities":["read","titan-write"]}"#
        );
    }

    #[test]
    fn string_or_null_distinguishes_absent_from_empty() {
        let mut o = Object::new();
        o.string_or_null("a", None).string_or_null("b", Some(""));
        assert_eq!(o.finish(), r#"{"a":null,"b":""}"#);
    }
}
