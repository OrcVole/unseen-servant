//! `/admin/status.gmi` — ADR 0011's "observe over the wire" resource: a
//! cert-gated, `admin`-capability gemtext page reporting health, the
//! last render's stats, the identity roster, and recent activity.
//!
//! **Deliberately not a `cert_zone`.** This is one fixed, built-in
//! diagnostic resource, not operator-configured content — authorizing it
//! is "does the presented certificate resolve, via the roster, to an
//! identity holding [`Capability::Admin`]", full stop. No path-prefix
//! configuration, no per-zone fingerprint list to maintain in parallel
//! with the roster. Extending the general `cert_zone` mechanism to
//! understand roster capabilities generically is real, separate design
//! work (already deferred — see ADR 0011's amendment on cert_zone roster
//! adoption); conflating that with this one page would make both worse.
//! Director-confirmed 2026-08-10.
//!
//! Mutations (reload, identity add/revoke/rotate) are **not** here and
//! never will be under this design: ADR 0011's management-reach decision
//! puts every mutation on the host/CLI side, on purpose — there is no
//! remote control plane for an attacker to seize. This resource only
//! ever reads.

use time::{Date, OffsetDateTime};

use crate::cli;
use crate::config::Config;
use crate::handler::ClientCertInfo;
use crate::protocol::response::{Header, Status, stock};
use crate::roster::Capability;
use crate::runtime_state::{ActivityEntry, RenderSnapshot};

/// The one fixed path this resource answers. Not configurable — see the
/// module docs on why a `cert_zone` would be the wrong shape here.
pub const ADMIN_STATUS_PATH: &str = "/admin/status.gmi";

/// Whether a request for [`ADMIN_STATUS_PATH`] is authorized.
#[derive(Debug)]
pub enum Decision {
    /// Answer with this status/META instead — the caller never sees the
    /// page.
    Refuse(Header),
    /// Render and serve the page.
    Allow,
}

/// Decide access to the admin resource: certificate present and valid,
/// and — the one check specific to this resource — the identity it
/// resolves to (via the roster, respecting an open rotation window
/// exactly as Titan does) holds [`Capability::Admin`]. No zone, no
/// path-prefix matching: the path is already fixed by the caller having
/// matched [`ADMIN_STATUS_PATH`] before calling this.
pub fn decide(config: &Config, today: Date, cert: Option<&ClientCertInfo>) -> Decision {
    match cert {
        None => Decision::Refuse(gate(
            Status::ClientCertRequired,
            "this resource requires a client certificate",
        )),
        Some(c) if !c.currently_valid => Decision::Refuse(gate(
            Status::CertNotValid,
            "your certificate is expired or not yet valid",
        )),
        Some(c) => {
            let authorized = config
                .roster
                .lookup(&c.fingerprint_sha256, today)
                .is_some_and(|found| found.identity.holds(Capability::Admin));
            if authorized {
                Decision::Allow
            } else {
                // Deliberately the same status a cert_zone gives an
                // unlisted certificate (61): "not authorized for this
                // resource" reveals nothing about whether the caller is
                // even on the roster at all, let alone which capability
                // is missing.
                Decision::Refuse(gate(
                    Status::CertNotAuthorized,
                    "your certificate is not authorized for this resource",
                ))
            }
        }
    }
}

fn gate(status: Status, message: &str) -> Header {
    Header::new(status, Some(message)).unwrap_or_else(|_| stock::unavailable())
}

/// Render the status page as gemtext. `activity` is expected oldest-first
/// (as [`crate::runtime_state::RuntimeState::recent_activity`] returns
/// it); this reverses it for display so the newest entry reads first —
/// the one an operator checking on an incident actually wants to see
/// without scrolling.
pub async fn render_status(
    config: &Config,
    activity: &[ActivityEntry],
    last_render: Option<&RenderSnapshot>,
    started_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> String {
    let published = cli::inspect_published(&config.state_dir).await;

    let mut out = String::from("# Server status\n\n");

    out.push_str("## Health\n\n");
    out.push_str(&format!("Now: {now}\n"));
    match started_at {
        Some(t) => out.push_str(&format!("Started: {t} (up {})\n", format_duration(now - t))),
        None => out.push_str("Started: unknown\n"),
    }
    out.push('\n');

    out.push_str("## Last render\n\n");
    match last_render {
        Some(r) => {
            out.push_str(&format!("At: {}\n", r.at));
            out.push_str(&format!("Pages rendered: {}\n", r.pages_rendered));
            out.push_str(&format!("Feed entries: {}\n", r.feed_entries));
            out.push_str(&format!("Mapped pages: {}\n", r.mapped_pages));
            out.push_str(&format!(
                "Robots mirrored: {}\n",
                if r.robots_mirrored { "yes" } else { "no" }
            ));
        }
        None => out.push_str("No render has completed yet.\n"),
    }
    out.push('\n');

    out.push_str("## Published\n\n");
    out.push_str(&cli::format_published_stats(&published));
    out.push('\n');

    out.push_str("## Identity roster\n\n");
    out.push_str(&cli::format_roster(config));
    out.push('\n');

    out.push_str("## Recent activity\n\n");
    if activity.is_empty() {
        out.push_str("No requests recorded yet this run.\n");
    } else {
        for entry in activity.iter().rev() {
            out.push_str(&format!("{}  {}  {}\n", entry.at, entry.status, entry.note));
        }
    }

    out
}

/// A short, human-scale rendering of an uptime duration — `time::Duration`'s
/// own `Display` is precise to sub-second resolution, which is noise for
/// "how long has this process been up".
fn format_duration(d: time::Duration) -> String {
    let total_secs = d.whole_seconds().max(0);
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;
    use crate::config::{Config, EnvOverrides};
    use time::macros::datetime;

    fn cert(fingerprint: &str, valid: bool) -> ClientCertInfo {
        ClientCertInfo {
            fingerprint_sha256: fingerprint.to_string(),
            currently_valid: valid,
        }
    }

    fn hex(nibble: char) -> String {
        std::iter::repeat_n(nibble, 64).collect()
    }

    fn today() -> Date {
        datetime!(2026-08-10 00:00:00 UTC).date()
    }

    fn config_with_admin(fingerprint: &str) -> Config {
        Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"root\"\nfingerprint = \"{fingerprint}\"\n\
                 capabilities = [\"admin\"]\n"
            ),
            &EnvOverrides::default(),
        )
        .unwrap()
    }

    #[test]
    fn no_certificate_is_60() {
        let config = Config::from_toml_str("", &EnvOverrides::default()).unwrap();
        assert!(matches!(
            decide(&config, today(), None),
            Decision::Refuse(_)
        ));
    }

    #[test]
    fn an_invalid_certificate_is_refused() {
        let config = config_with_admin(&hex('a'));
        let d = decide(&config, today(), Some(&cert(&hex('a'), false)));
        assert!(matches!(d, Decision::Refuse(_)));
    }

    #[test]
    fn an_identity_holding_admin_is_allowed() {
        let config = config_with_admin(&hex('a'));
        let d = decide(&config, today(), Some(&cert(&hex('a'), true)));
        assert!(matches!(d, Decision::Allow));
    }

    #[test]
    fn an_identity_without_admin_is_refused() {
        let config = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"scribe\"\nfingerprint = \"{}\"\n\
                 capabilities = [\"titan-write\"]\n",
                hex('a')
            ),
            &EnvOverrides::default(),
        )
        .unwrap();
        let d = decide(&config, today(), Some(&cert(&hex('a'), true)));
        assert!(matches!(d, Decision::Refuse(_)));
    }

    #[test]
    fn a_certificate_not_on_the_roster_at_all_is_refused() {
        let config = config_with_admin(&hex('a'));
        let d = decide(&config, today(), Some(&cert(&hex('b'), true)));
        assert!(matches!(d, Decision::Refuse(_)));
    }

    #[test]
    fn a_superseded_key_inside_its_window_is_still_allowed() {
        let config = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"root\"\nfingerprint = \"{}\"\n\
                 superseded = [\"{}\"]\nsuperseded_until = \"2099-01-01\"\n\
                 capabilities = [\"admin\"]\n",
                hex('a'),
                hex('b')
            ),
            &EnvOverrides::default(),
        )
        .unwrap();
        let d = decide(&config, today(), Some(&cert(&hex('b'), true)));
        assert!(matches!(d, Decision::Allow));
    }

    #[tokio::test]
    async fn render_status_includes_every_section() {
        let config = config_with_admin(&hex('a'));
        let activity = [ActivityEntry {
            at: datetime!(2026-08-10 01:00:00 UTC),
            status: 20,
            note: "example for /".to_string(),
        }];
        let render = RenderSnapshot {
            at: datetime!(2026-08-10 00:30:00 UTC),
            pages_rendered: 3,
            feed_entries: 1,
            mapped_pages: 3,
            robots_mirrored: true,
        };
        let out = render_status(
            &config,
            &activity,
            Some(&render),
            Some(datetime!(2026-08-10 00:00:00 UTC)),
            datetime!(2026-08-10 02:00:00 UTC),
        )
        .await;
        assert!(out.contains("# Server status"));
        assert!(out.contains("## Health"));
        assert!(out.contains("## Last render"));
        assert!(out.contains("Pages rendered: 3"));
        assert!(out.contains("## Published"));
        assert!(out.contains("## Identity roster"));
        assert!(out.contains("root"));
        assert!(out.contains("## Recent activity"));
        assert!(out.contains("example for /"));
        assert!(out.contains("up 2h"));
    }

    #[tokio::test]
    async fn render_status_of_a_fresh_server_says_so_plainly() {
        let config = config_with_admin(&hex('a'));
        let out = render_status(&config, &[], None, None, datetime!(2026-08-10 00:00:00 UTC)).await;
        assert!(out.contains("No render has completed yet"));
        assert!(out.contains("No requests recorded yet"));
        assert!(out.contains("Started: unknown"));
    }

    #[tokio::test]
    async fn render_status_shows_newest_activity_first() {
        let config = config_with_admin(&hex('a'));
        let activity = [
            ActivityEntry {
                at: datetime!(2026-08-10 01:00:00 UTC),
                status: 20,
                note: "first".to_string(),
            },
            ActivityEntry {
                at: datetime!(2026-08-10 01:00:01 UTC),
                status: 20,
                note: "second".to_string(),
            },
        ];
        let out = render_status(
            &config,
            &activity,
            None,
            None,
            datetime!(2026-08-10 02:00:00 UTC),
        )
        .await;
        let first_pos = out.find("first").unwrap();
        let second_pos = out.find("second").unwrap();
        assert!(second_pos < first_pos, "newest entry must read first");
    }

    #[test]
    fn duration_formatting_scales_to_the_largest_useful_unit() {
        assert_eq!(format_duration(time::Duration::minutes(5)), "5m");
        assert_eq!(format_duration(time::Duration::minutes(90)), "1h 30m");
        assert_eq!(
            format_duration(time::Duration::hours(50) + time::Duration::minutes(5)),
            "2d 2h"
        );
    }
}
