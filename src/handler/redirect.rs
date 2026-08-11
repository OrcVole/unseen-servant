//! Regex-based redirects with capture-group substitution — Molly Brown's
//! model (docs/internal/recon/prior-art.md §3): a list of `(pattern, target)`
//! pairs, tried in config order, first match wins. Single hop only: the
//! target is never itself re-checked against the redirect list, matching
//! recon guidance ("cap any internally-generated redirect chains... ideally
//! a single hop").
//!
//! Targets never carry fragments (spec: "servers SHOULD NOT include
//! fragments in redirects" — enforced by construction, since the target
//! string is config-authored and `Header::new` would reject a literal `#`
//! only if it were a control character, which fragments aren't; the actual
//! guarantee here is a lint at config-load time, not the response layer).

use regex::Regex;

use crate::handler::HandlerResponse;
use crate::protocol::response::{Header, Status};

/// One redirect rule: a compiled pattern and a substitution template using
/// `$1`, `$2`, … for capture groups (the `regex` crate's own `$name`
/// syntax works too, since substitution is delegated to it directly).
#[derive(Debug, Clone)]
pub struct Rule {
    pattern: Regex,
    target: String,
    /// Permanent (31) vs temporary (30) redirect.
    permanent: bool,
}

/// Why a redirect rule failed to compile from config.
#[derive(Debug)]
pub struct RuleError(pub String);

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid redirect pattern: {}", self.0)
    }
}

impl std::error::Error for RuleError {}

impl Rule {
    /// Compile a redirect rule. `pattern` is matched against the request
    /// path (not the full URI); `target` may reference capture groups.
    pub fn new(pattern: &str, target: &str, permanent: bool) -> Result<Rule, RuleError> {
        let pattern = Regex::new(pattern).map_err(|e| RuleError(e.to_string()))?;
        Ok(Rule {
            pattern,
            target: target.to_string(),
            permanent,
        })
    }
}

/// Try each rule in order against `path`; return the first match's
/// response, or `None` if nothing matched (the caller falls through to
/// static file serving).
pub fn try_match(rules: &[Rule], path: &str) -> Option<HandlerResponse> {
    for rule in rules {
        if let Some(captures) = rule.pattern.captures(path) {
            let mut target = String::new();
            captures.expand(&rule.target, &mut target);
            let status = if rule.permanent {
                Status::RedirectPermanent
            } else {
                Status::RedirectTemporary
            };
            // Target is config-authored, not attacker input, but the
            // header constructor's control-character/BOM checks still
            // apply uniformly — a malformed target fails closed as 51
            // rather than emitting a broken redirect header.
            let header =
                Header::new(status, Some(&target)).unwrap_or_else(|_| non_matching_fallback());
            return Some(HandlerResponse::header_only(header));
        }
    }
    None
}

/// Unreachable in practice for well-formed config (targets are plain
/// strings without control characters); exists so a malformed target is a
/// safe 51 rather than a panic.
fn non_matching_fallback() -> Header {
    crate::protocol::response::stock::not_found()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    #[test]
    fn simple_redirect_matches() {
        let rule = Rule::new("^/old$", "/new", false).unwrap();
        let resp = try_match(&[rule], "/old").expect("should match");
        assert_eq!(resp.header.status(), Status::RedirectTemporary);
        assert_eq!(resp.header.to_wire(), b"30 /new\r\n");
    }

    #[test]
    fn permanent_flag_selects_31() {
        let rule = Rule::new("^/gone$", "/elsewhere", true).unwrap();
        let resp = try_match(&[rule], "/gone").expect("should match");
        assert_eq!(resp.header.status(), Status::RedirectPermanent);
    }

    #[test]
    fn capture_groups_substitute() {
        let rule = Rule::new(r"^/posts/(\d+)$", "/blog/$1", false).unwrap();
        let resp = try_match(&[rule], "/posts/42").expect("should match");
        assert_eq!(resp.header.to_wire(), b"30 /blog/42\r\n");
    }

    #[test]
    fn non_matching_path_falls_through() {
        let rule = Rule::new("^/old$", "/new", false).unwrap();
        assert!(try_match(&[rule], "/unrelated").is_none());
    }

    #[test]
    fn first_match_wins() {
        let rules = vec![
            Rule::new("^/x$", "/first", false).unwrap(),
            Rule::new("^/x$", "/second", false).unwrap(),
        ];
        let resp = try_match(&rules, "/x").expect("should match");
        assert_eq!(resp.header.to_wire(), b"30 /first\r\n");
    }

    #[test]
    fn invalid_pattern_is_rejected_at_compile_time() {
        assert!(Rule::new("[unclosed", "/x", false).is_err());
    }
}
