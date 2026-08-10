//! The identity roster (ADR 0011): named client identities, key rotation
//! with a self-closing window, and capability scoping.
//!
//! # Why this exists
//!
//! C2 shipped identity as a flat list of SHA-256 fingerprints per zone
//! (`handler::cert_zone`, `handler::titan`). That is a working
//! authorization primitive and a poor *identity* one: a fingerprint has no
//! name, no history, no way to be rotated, and no meaning outside the one
//! zone that lists it. The agent-web reconnaissance
//! (`docs/recon/agent-web.md`) found usv philosophically ahead of the
//! field on "the key **is** the identity, no CA, no accounts" — Web Bot
//! Auth, SPIFFE and DIDs are all converging on exactly that — but *behind*
//! on lifecycle, where every serious effort (SPIFFE SVIDs, OAuth 2.1 in
//! MCP, IETF WIMSE) treats a durable static credential as an
//! anti-pattern. This module pays that debt.
//!
//! # What an identity is here
//!
//! `fingerprint → { label, capabilities, rotation state }`. The
//! fingerprint remains the durable key: this is still pure TOFU, with no
//! certificate authority, no account system, and no attestation. usv
//! verifies **continuity** ("the same holder as last time"), never
//! **provenance** ("this key belongs to Acme Corp") — the honest limit
//! recorded in ADR 0011, and the same property that keeps the model
//! censorship-resistant.
//!
//! # Rotation, and why the window closes itself
//!
//! An identity has exactly one *current* fingerprint and may list
//! [`Identity::superseded`] keys that are still honored while a holder
//! finishes moving to a new one. Listing any superseded key **requires**
//! [`Identity::superseded_until`] ([`RosterError::RotationWithoutDeadline`]):
//! an overlap that never expires is just two permanent credentials, which
//! is the very anti-pattern rotation exists to escape. Past that date the
//! old keys stop being accepted with no further operator action — the
//! documented TOFU failure mode is a mis-pin nobody remembers to unwind,
//! so forgetting must fail *closed*.
//!
//! # Capabilities
//!
//! A capability is a server-wide grant ([`Capability`]); zone membership
//! is the local one. Both are required, and they are not redundant:
//! removing `titan-write` from an identity disables it everywhere at once
//! without hunting through every zone that named it.
//!
//! Enrollment tokens — ADR 0011's third roster item — are **not** here.
//! Minting one is a mutation, and mutations are host/CLI-only under that
//! ADR's management-reach decision, so enrollment lands with the C5
//! tooling that mints the tokens. Read-gating `cert_zone`s likewise still
//! carry raw fingerprint lists; they adopt the roster in C5. Neither is an
//! oversight: both are recorded here so the gap is visible rather than
//! implied.

use time::Date;

/// A server-wide grant held by an identity.
///
/// Deliberately a closed set: an unknown capability in a configuration
/// file is a startup error, never a silently-ignored word that leaves an
/// operator believing they granted something they did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// May read through a certificate-gated read zone.
    Read,
    /// May upload over Titan (`handler::titan`).
    TitanWrite,
    /// May read the server's status/roster/audit surface (C5; ADR 0011's
    /// "observe over the wire" half). Recorded now so the capability
    /// vocabulary is settled before the surface exists.
    Admin,
}

impl Capability {
    /// The config-facing spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Read => "read",
            Capability::TitanWrite => "titan-write",
            Capability::Admin => "admin",
        }
    }

    /// Parse a config-facing name. `None` for anything unrecognised —
    /// callers turn that into a startup error.
    pub fn parse(name: &str) -> Option<Capability> {
        match name {
            "read" => Some(Capability::Read),
            "titan-write" => Some(Capability::TitanWrite),
            "admin" => Some(Capability::Admin),
            _ => None,
        }
    }

    /// Every capability, for error messages that tell the operator what
    /// they *could* have written.
    pub const ALL: &'static [Capability] =
        &[Capability::Read, Capability::TitanWrite, Capability::Admin];
}

/// One named client identity.
#[derive(Debug, Clone)]
pub struct Identity {
    /// Operator-facing name (`scribe-agent`). What an audit line says
    /// instead of 64 characters of hex.
    pub label: String,
    /// The current SHA-256 certificate fingerprint, lowercase hex.
    pub fingerprint: String,
    /// Fingerprints being retired, still accepted until
    /// [`Identity::superseded_until`].
    pub superseded: Vec<String>,
    /// The day the rotation window closes. Required whenever
    /// [`Identity::superseded`] is non-empty; meaningless without it.
    pub superseded_until: Option<Date>,
    /// Server-wide grants.
    pub capabilities: Vec<Capability>,
    /// When this identity was first added, recorded for provenance. usv
    /// can honestly claim "this key, first seen on this date" and nothing
    /// more (ADR 0011: continuity, not attestation).
    pub enrolled: Option<Date>,
}

impl Identity {
    /// Whether this identity holds `capability`.
    pub fn holds(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Which key a caller authenticated with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAge {
    /// The identity's current fingerprint.
    Current,
    /// A superseded fingerprint inside its still-open rotation window.
    /// Worth logging: the holder has not finished rotating.
    Superseded,
}

/// A successful roster lookup.
#[derive(Debug, Clone, Copy)]
pub struct Match<'a> {
    /// The identity the presented fingerprint belongs to.
    pub identity: &'a Identity,
    /// Whether the current or a retiring key was used.
    pub key_age: KeyAge,
}

/// Why a roster (or one of its entries) was refused at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RosterError {
    /// An identity has an empty label.
    EmptyLabel,
    /// Two identities share a label — references by name would be
    /// ambiguous, so this is refused rather than resolved by order.
    DuplicateLabel(String),
    /// A fingerprint is not 64 lowercase-able hex characters. SHA-256 is
    /// exactly that long; anything else is a truncation or a typo, and a
    /// truncated fingerprint silently matches nothing.
    MalformedFingerprint {
        /// The identity it appeared on.
        label: String,
        /// The offending value.
        value: String,
    },
    /// The same fingerprint appears on more than one identity, which would
    /// make the caller's identity depend on lookup order.
    DuplicateFingerprint(String),
    /// Superseded keys were listed with no date for the window to close.
    RotationWithoutDeadline(String),
    /// A `superseded_until` was set with nothing to supersede.
    DeadlineWithoutRotation(String),
    /// An unrecognised capability name.
    UnknownCapability {
        /// The identity it appeared on.
        label: String,
        /// The unrecognised word.
        value: String,
    },
}

impl std::fmt::Display for RosterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RosterError::EmptyLabel => f.write_str("an identity has an empty label"),
            RosterError::DuplicateLabel(l) => write!(
                f,
                "two identities are both labelled {l:?}; labels are how zones refer to them, \
                 so they must be unique"
            ),
            RosterError::MalformedFingerprint { label, value } => write!(
                f,
                "identity {label:?} has fingerprint {value:?}, which is not a SHA-256 \
                 fingerprint (64 hex characters). A truncated fingerprint matches nothing, \
                 so this would silently lock the identity out"
            ),
            RosterError::DuplicateFingerprint(fp) => write!(
                f,
                "fingerprint {fp} is listed on more than one identity; a certificate must \
                 resolve to exactly one identity"
            ),
            RosterError::RotationWithoutDeadline(l) => write!(
                f,
                "identity {l:?} lists superseded fingerprints but no superseded_until date. \
                 A rotation window that never closes is just two permanent keys — set the \
                 date the old key stops being accepted (e.g. superseded_until = 2026-09-01)"
            ),
            RosterError::DeadlineWithoutRotation(l) => write!(
                f,
                "identity {l:?} sets superseded_until but lists no superseded fingerprints; \
                 the date would govern nothing"
            ),
            RosterError::UnknownCapability { label, value } => {
                let known: Vec<&str> = Capability::ALL.iter().map(|c| c.as_str()).collect();
                write!(
                    f,
                    "identity {label:?} lists unknown capability {value:?}; known \
                     capabilities are: {}",
                    known.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for RosterError {}

/// Every configured identity, indexed for lookup by presented
/// fingerprint.
#[derive(Debug, Clone, Default)]
pub struct Roster {
    identities: Vec<Identity>,
}

impl Roster {
    /// Build and validate a roster. Every rule is checked here rather than
    /// at the config layer, so config and enforcement cannot drift.
    pub fn new(identities: Vec<Identity>) -> Result<Roster, RosterError> {
        let mut seen_labels: Vec<&str> = Vec::with_capacity(identities.len());
        let mut seen_fingerprints: Vec<String> = Vec::new();

        for identity in &identities {
            if identity.label.trim().is_empty() {
                return Err(RosterError::EmptyLabel);
            }
            if seen_labels
                .iter()
                .any(|l| l.eq_ignore_ascii_case(&identity.label))
            {
                return Err(RosterError::DuplicateLabel(identity.label.clone()));
            }
            seen_labels.push(&identity.label);

            for fp in std::iter::once(&identity.fingerprint).chain(identity.superseded.iter()) {
                if !is_sha256_hex(fp) {
                    return Err(RosterError::MalformedFingerprint {
                        label: identity.label.clone(),
                        value: fp.clone(),
                    });
                }
                let normalised = fp.to_ascii_lowercase();
                if seen_fingerprints.contains(&normalised) {
                    return Err(RosterError::DuplicateFingerprint(normalised));
                }
                seen_fingerprints.push(normalised);
            }

            match (identity.superseded.is_empty(), identity.superseded_until) {
                (false, None) => {
                    return Err(RosterError::RotationWithoutDeadline(identity.label.clone()));
                }
                (true, Some(_)) => {
                    return Err(RosterError::DeadlineWithoutRotation(identity.label.clone()));
                }
                _ => {}
            }
        }

        Ok(Roster { identities })
    }

    /// Resolve a presented certificate fingerprint to an identity.
    ///
    /// `today` is passed in rather than read from the clock so the
    /// rotation window is testable and the function stays pure. A
    /// superseded key matches only while its window is open; once
    /// `today` is past `superseded_until` the old key is simply not on
    /// the roster any more, with no operator action and no restart.
    pub fn lookup(&self, fingerprint: &str, today: Date) -> Option<Match<'_>> {
        for identity in &self.identities {
            if identity.fingerprint.eq_ignore_ascii_case(fingerprint) {
                return Some(Match {
                    identity,
                    key_age: KeyAge::Current,
                });
            }
        }
        for identity in &self.identities {
            let window_open = identity
                .superseded_until
                .is_some_and(|until| today <= until);
            if window_open
                && identity
                    .superseded
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(fingerprint))
            {
                return Some(Match {
                    identity,
                    key_age: KeyAge::Superseded,
                });
            }
        }
        None
    }

    /// Look an identity up by label — how a zone names who may use it.
    pub fn by_label(&self, label: &str) -> Option<&Identity> {
        self.identities
            .iter()
            .find(|i| i.label.eq_ignore_ascii_case(label))
    }

    /// Every identity, for the C5 status surface and for validating that
    /// a zone's named identities all exist.
    pub fn identities(&self) -> &[Identity] {
        &self.identities
    }

    /// Whether the roster holds no identities at all.
    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
}

/// Exactly 64 hexadecimal characters — the length of a SHA-256 digest in
/// hex, and nothing else.
///
/// `pub(crate)`: `cli::identity` reuses this to validate a fingerprint
/// *before* generating a config snippet, so a typo is caught immediately
/// rather than only surfacing later when the operator pastes it in and
/// usv refuses to start — one validation rule, not two that could drift.
pub(crate) fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    /// A syntactically valid SHA-256 fingerprint made of one repeated
    /// nibble, so tests can name them readably.
    fn fp(nibble: char) -> String {
        std::iter::repeat_n(nibble, 64).collect()
    }

    fn day(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, time::Month::try_from(m).unwrap(), d).unwrap()
    }

    fn identity(label: &str, current: char) -> Identity {
        Identity {
            label: label.to_string(),
            fingerprint: fp(current),
            superseded: Vec::new(),
            superseded_until: None,
            capabilities: vec![Capability::TitanWrite],
            enrolled: None,
        }
    }

    #[test]
    fn a_current_fingerprint_resolves_to_its_identity() {
        let roster = Roster::new(vec![identity("scribe", 'a')]).unwrap();
        let found = roster.lookup(&fp('a'), day(2026, 8, 10)).unwrap();
        assert_eq!(found.identity.label, "scribe");
        assert_eq!(found.key_age, KeyAge::Current);
    }

    #[test]
    fn an_unknown_fingerprint_resolves_to_nothing() {
        let roster = Roster::new(vec![identity("scribe", 'a')]).unwrap();
        assert!(roster.lookup(&fp('b'), day(2026, 8, 10)).is_none());
    }

    #[test]
    fn fingerprint_matching_is_case_insensitive() {
        let roster = Roster::new(vec![identity("scribe", 'a')]).unwrap();
        let upper: String = std::iter::repeat_n('A', 64).collect();
        assert!(roster.lookup(&upper, day(2026, 8, 10)).is_some());
    }

    #[test]
    fn a_superseded_key_works_inside_its_window_and_is_flagged() {
        // The rotation story: the holder has pinned a new key but some
        // clients still present the old one.
        let mut id = identity("scribe", 'a');
        id.superseded = vec![fp('b')];
        id.superseded_until = Some(day(2026, 9, 1));
        let roster = Roster::new(vec![id]).unwrap();

        let old = roster.lookup(&fp('b'), day(2026, 8, 10)).unwrap();
        assert_eq!(old.identity.label, "scribe");
        assert_eq!(
            old.key_age,
            KeyAge::Superseded,
            "the caller is authorized, but the log should say it used a retiring key"
        );

        // The new key works simultaneously — that is the whole point of an
        // overlap window.
        assert_eq!(
            roster.lookup(&fp('a'), day(2026, 8, 10)).unwrap().key_age,
            KeyAge::Current
        );
    }

    #[test]
    fn the_rotation_window_closes_itself_with_no_operator_action() {
        // The documented TOFU failure mode is a stale pin nobody remembers
        // to remove. Forgetting must fail closed.
        let mut id = identity("scribe", 'a');
        id.superseded = vec![fp('b')];
        id.superseded_until = Some(day(2026, 9, 1));
        let roster = Roster::new(vec![id]).unwrap();

        assert!(
            roster.lookup(&fp('b'), day(2026, 9, 1)).is_some(),
            "the deadline day itself is still inside the window"
        );
        assert!(
            roster.lookup(&fp('b'), day(2026, 9, 2)).is_none(),
            "the day after, the old key is simply not on the roster"
        );
        assert!(
            roster.lookup(&fp('a'), day(2027, 1, 1)).is_some(),
            "the current key is unaffected by the window closing"
        );
    }

    #[test]
    fn superseded_keys_require_a_deadline() {
        let mut id = identity("scribe", 'a');
        id.superseded = vec![fp('b')];
        let err = Roster::new(vec![id]).unwrap_err();
        assert!(matches!(err, RosterError::RotationWithoutDeadline(_)));
        assert!(
            err.to_string().contains("two permanent keys"),
            "the message must explain why: {err}"
        );
    }

    #[test]
    fn a_deadline_without_anything_to_retire_is_refused() {
        let mut id = identity("scribe", 'a');
        id.superseded_until = Some(day(2026, 9, 1));
        assert!(matches!(
            Roster::new(vec![id]).unwrap_err(),
            RosterError::DeadlineWithoutRotation(_)
        ));
    }

    #[test]
    fn labels_must_be_present_and_unique() {
        let mut blank = identity("", 'a');
        blank.label = "  ".into();
        assert!(matches!(
            Roster::new(vec![blank]).unwrap_err(),
            RosterError::EmptyLabel
        ));

        let err = Roster::new(vec![identity("same", 'a'), identity("SAME", 'b')]).unwrap_err();
        assert!(matches!(err, RosterError::DuplicateLabel(_)));
    }

    #[test]
    fn a_fingerprint_may_belong_to_only_one_identity() {
        let err = Roster::new(vec![identity("one", 'a'), identity("two", 'a')]).unwrap_err();
        assert!(matches!(err, RosterError::DuplicateFingerprint(_)));

        // Including when the collision is against a superseded key.
        let mut rotating = identity("three", 'c');
        rotating.superseded = vec![fp('d')];
        rotating.superseded_until = Some(day(2026, 9, 1));
        let clash = identity("four", 'd');
        assert!(matches!(
            Roster::new(vec![rotating, clash]).unwrap_err(),
            RosterError::DuplicateFingerprint(_)
        ));
    }

    #[test]
    fn a_truncated_fingerprint_is_refused_rather_than_silently_useless() {
        // "aabbcc" would simply never match anything, locking the identity
        // out with no diagnostic — so it is a startup error instead.
        let mut short = identity("scribe", 'a');
        short.fingerprint = "aabbcc".into();
        let err = Roster::new(vec![short]).unwrap_err();
        assert!(matches!(err, RosterError::MalformedFingerprint { .. }));
        assert!(err.to_string().contains("matches nothing"), "{err}");

        let mut nonhex = identity("scribe", 'a');
        nonhex.fingerprint = std::iter::repeat_n('z', 64).collect();
        assert!(matches!(
            Roster::new(vec![nonhex]).unwrap_err(),
            RosterError::MalformedFingerprint { .. }
        ));
    }

    #[test]
    fn capabilities_round_trip_and_reject_unknown_names() {
        for c in Capability::ALL {
            assert_eq!(Capability::parse(c.as_str()), Some(*c));
        }
        assert_eq!(
            Capability::parse("titan-write"),
            Some(Capability::TitanWrite)
        );
        assert!(
            Capability::parse("write").is_none(),
            "not a real capability"
        );
        assert!(Capability::parse("TITAN-WRITE").is_none(), "exact spelling");
    }

    #[test]
    fn holding_a_capability_is_exact_not_implied() {
        let id = identity("scribe", 'a');
        assert!(id.holds(Capability::TitanWrite));
        assert!(
            !id.holds(Capability::Admin),
            "no capability implies any other — admin is not a superset"
        );
        assert!(!id.holds(Capability::Read));
    }

    #[test]
    fn lookup_by_label_is_case_insensitive() {
        let roster = Roster::new(vec![identity("Scribe-Agent", 'a')]).unwrap();
        assert!(roster.by_label("scribe-agent").is_some());
        assert!(roster.by_label("nobody").is_none());
    }

    #[test]
    fn an_empty_roster_is_valid_and_resolves_nothing() {
        let roster = Roster::new(Vec::new()).unwrap();
        assert!(roster.is_empty());
        assert!(roster.lookup(&fp('a'), day(2026, 8, 10)).is_none());
    }
}
