//! The wall: what may never be rendered into a cleartext tree
//! (ADR 0012 §6).
//!
//! Gopher, Spartan, Nex and Finger cannot authenticate a client. Not
//! "authenticate weakly" — the protocols have no mechanism at all. usv
//! has a single content tree, and parts of it can be gated behind client
//! certificates (`cert_zone`) or made writable to specific keys
//! (`titan_zone`). Rendering that tree to a cleartext target without
//! subtracting the gated parts would publish, in the clear, exactly the
//! content an operator marked as not-for-everyone.
//!
//! So the gate is applied where the cleartext trees are *built*, not
//! where they are served: a page under a gated prefix is never written
//! into one, which means no request path, no misconfiguration and no
//! future protocol can serve what was never emitted.
//!
//! **Amendment to ADR 0012 §6, made while implementing it.** The ADR
//! said a configuration that would publish a gated path over a cleartext
//! protocol should be a startup error. Building it showed that reading
//! is too strong: since exclusion already makes disclosure impossible,
//! a blanket error would mean a capsule with one small private area
//! could not serve gopher *at all* — which pushes operators to abandon
//! cert zones or abandon gopher, and the likeliest casualty is the cert
//! zone. What survives from the ADR's intent is the part that matters:
//!
//! * exclusion is unconditional and structural (here);
//! * every excluded prefix is **announced at startup**, so an operator
//!   is never quietly missing content they expected to see;
//! * a genuinely contradictory configuration — a cleartext root that
//!   points *into* a gated prefix, which can only mean "publish this
//!   gated thing in the clear" — remains a startup error, because that
//!   one is an instruction rather than an oversight.

use crate::config::HostConfig;

/// The set of path prefixes that must not reach any cleartext tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gate {
    excluded: Vec<String>,
}

impl Gate {
    /// Build the gate for one host from its certificate and Titan zones.
    ///
    /// Titan zones are included as well as certificate zones: a writable
    /// area is one whose *contents* are controlled by a specific key, and
    /// mirroring it into a tree where anyone can read (and any on-path
    /// party can alter) it is the same disclosure with an extra step.
    pub fn for_host(host: &HostConfig) -> Self {
        let mut excluded: Vec<String> = host
            .cert_zones
            .iter()
            .map(|z| normalise(&z.path_prefix))
            .chain(host.titan_zones.iter().map(|z| normalise(&z.path_prefix)))
            .collect();
        excluded.sort();
        excluded.dedup();
        Self { excluded }
    }

    /// Whether anything is gated at all.
    pub fn is_empty(&self) -> bool {
        self.excluded.is_empty()
    }

    /// Every excluded prefix, for the startup announcement.
    pub fn prefixes(&self) -> &[String] {
        &self.excluded
    }

    /// The prefix that gates `path`, if any.
    ///
    /// Matching is on whole path segments. `/up` must not gate
    /// `/uploads`, and equally `/private` must gate `/private` itself as
    /// well as everything beneath it — getting either wrong leaks or
    /// hides the wrong thing.
    pub fn excluded_by(&self, path: &str) -> Option<&str> {
        let p = normalise_path(path);
        self.excluded
            .iter()
            .find(|prefix| {
                // `prefix` always ends in '/'. A path is inside it when it
                // starts with it, or equals it without the trailing slash.
                p.starts_with(prefix.as_str()) || p == prefix.trim_end_matches('/')
            })
            .map(String::as_str)
    }

    /// Whether `path` must be kept out of cleartext trees.
    pub fn excludes(&self, path: &str) -> bool {
        self.excluded_by(path).is_some()
    }
}

/// Reject a cleartext root that points inside a gated prefix.
///
/// This is the one configuration that cannot be read as an oversight:
/// pointing a cleartext protocol's root *at* gated content is an
/// instruction to publish it. Refused at startup, naming both sides, in
/// the same spirit as an empty Titan allowlist (ADR 0006).
pub fn check_cleartext_root(protocol: &str, root: &str, gate: &Gate) -> Result<(), String> {
    match gate.excluded_by(root) {
        Some(prefix) => Err(format!(
            "{protocol} root {root:?} is inside {prefix:?}, which is gated behind a client \
             certificate. {protocol} cannot authenticate a client at all, so serving that \
             path would publish gated content in the clear. Move the {protocol} root outside \
             {prefix:?}, or remove the zone if the content is not actually private."
        )),
        None => Ok(()),
    }
}

/// Announce what a cleartext listener will not be serving.
///
/// An operator who gated `/private/` and then enabled gopher should not
/// have to deduce why it is missing; silence here is how a safety
/// measure gets mistaken for a bug and worked around.
pub fn announce(protocol: &str, gate: &Gate) {
    if gate.is_empty() {
        return;
    }
    tracing::info!(
        protocol,
        excluded = ?gate.prefixes(),
        "these paths are gated behind a client certificate and are therefore excluded from \
         the {} tree — {} cannot authenticate a client, so gated content is never rendered \
         into it",
        protocol,
        protocol
    );
}

/// Normalise a configured prefix to leading and trailing slashes.
fn normalise(prefix: &str) -> String {
    let trimmed = prefix.trim();
    let mut out = String::with_capacity(trimmed.len() + 2);
    if !trimmed.starts_with('/') {
        out.push('/');
    }
    out.push_str(trimmed);
    if !out.ends_with('/') {
        out.push('/');
    }
    out
}

/// Normalise a path for comparison: leading slash, no trailing slash
/// duplication.
fn normalise_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{cert_zone, titan};
    use std::path::PathBuf;

    fn host_with(cert: &[&str], titan_prefixes: &[&str]) -> HostConfig {
        HostConfig {
            name: "example.org".into(),
            docroot: PathBuf::from("/tmp"),
            redirects: Vec::new(),
            cert_zones: cert
                .iter()
                .map(|p| cert_zone::Zone {
                    path_prefix: (*p).to_string(),
                    allowed_fingerprints: Vec::new(),
                })
                .collect(),
            titan_zones: titan_prefixes
                .iter()
                .map(|p| titan::Zone {
                    path_prefix: (*p).to_string(),
                    allowed_fingerprints: vec!["ab".into()],
                    allowed_identities: Vec::new(),
                    max_upload_bytes: 1,
                    allowed_mime: Vec::new(),
                    token: None,
                    allow_delete: false,
                })
                .collect(),
        }
    }

    #[test]
    fn a_capsule_with_no_zones_gates_nothing() {
        let g = Gate::for_host(&host_with(&[], &[]));
        assert!(g.is_empty());
        assert!(!g.excludes("/anything"));
    }

    #[test]
    fn a_cert_zone_is_excluded_with_everything_under_it() {
        let g = Gate::for_host(&host_with(&["/private/"], &[]));
        assert!(g.excludes("/private/"));
        assert!(g.excludes("/private/secret.gmi"));
        assert!(g.excludes("/private/deeper/still.gmi"));
        // The zone root itself, written without the trailing slash.
        assert!(g.excludes("/private"));
    }

    #[test]
    fn a_titan_zone_is_excluded_too() {
        // A writable area's contents are controlled by one key; mirroring
        // it into a tree anyone can read and alter is the same disclosure
        // with an extra step.
        let g = Gate::for_host(&host_with(&[], &["/uploads/"]));
        assert!(g.excludes("/uploads/note.gmi"));
    }

    #[test]
    fn a_prefix_does_not_gate_a_longer_sibling_name() {
        // The bug this is here to prevent: `/up` silently gating
        // `/uploads`, or worse, `/up` NOT gating `/up/x`.
        let g = Gate::for_host(&host_with(&["/up/"], &[]));
        assert!(g.excludes("/up/x.gmi"));
        assert!(!g.excludes("/uploads/x.gmi"));
        assert!(!g.excludes("/upstairs.gmi"));
    }

    #[test]
    fn prefixes_are_normalised_however_they_were_written() {
        let g = Gate::for_host(&host_with(&["private"], &[]));
        assert_eq!(g.prefixes(), ["/private/"]);
        assert!(g.excludes("/private/x"));
    }

    #[test]
    fn duplicate_zones_are_reported_once() {
        let g = Gate::for_host(&host_with(&["/p/", "/p"], &["/p/"]));
        assert_eq!(g.prefixes(), ["/p/"]);
    }

    #[test]
    fn a_cleartext_root_inside_a_gated_prefix_is_refused() {
        let g = Gate::for_host(&host_with(&["/private/"], &[]));
        let Err(err) = check_cleartext_root("gopher", "/private/notes", &g) else {
            panic!("a root inside a gated prefix must be refused");
        };
        assert!(err.contains("/private/"), "{err}");
        assert!(err.contains("gopher"), "{err}");
        // The message must say what to do, not merely that it refused.
        assert!(err.contains("Move the gopher root"), "{err}");
    }

    #[test]
    fn an_ordinary_cleartext_root_is_accepted() {
        let g = Gate::for_host(&host_with(&["/private/"], &[]));
        assert!(check_cleartext_root("gopher", "/", &g).is_ok());
        assert!(check_cleartext_root("gopher", "/public/", &g).is_ok());
    }
}
