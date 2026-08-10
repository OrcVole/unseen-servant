//! Configuration: one TOML file, gmid semantics, env overrides for
//! platform-injected facts (ADR 0007).
//!
//! The whole surface is a single `usv.toml`. Search order (first hit wins):
//! `--config <path>` flag, then `$USV_CONFIG`, then `${state_dir}/usv.toml`,
//! then built-in defaults — the server must start usefully with no file at
//! all (ADR 0008: zero-arg `usv` starts a working capsule).
//!
//! Layering, lowest to highest precedence: built-in defaults, then the file,
//! then `USV_*` environment variables. Env overrides exist for facts a
//! platform injects at runtime (ports, paths, hostname) and never for
//! content-security settings; the Cloudron profile (ADR 0008) maps the
//! platform's own names (`GEMINI_PORT`, `CLOUDRON_APP_DOMAIN`, …) onto these
//! same `USV_*` knobs in `start.sh` — the core stays Cloudron-free.
//!
//! Unknown keys are startup errors, not warnings: a typo'd security setting
//! must never be silently ignored. The reserved `[titan]` (ADR 0006) and
//! `[responses]` (ADR 0009) sections error helpfully until their phases land.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The parsed, validated configuration the rest of the crate consumes.
///
/// Everything here is immutable after load; SIGHUP builds a fresh `Config`
/// and swaps it (ADR 0002), it never mutates a live one.
#[derive(Debug, Clone)]
pub struct Config {
    /// Where mutable server state lives: certs, and (from C2/C3) content and
    /// rendered output. `/app/data` on Cloudron, XDG state dir or
    /// `/var/lib/usv` standalone (ADR 0008).
    pub state_dir: PathBuf,
    /// Socket addresses the Gemini listener binds. IPv6 addresses are bound
    /// with `IPV6_V6ONLY` set, so the default pair covers both families
    /// without double-bind conflicts. **Empty is valid** and means no
    /// Gemini listener at all — the explicit `USV_LISTEN=""` (or
    /// `server.listen = []`) opt-out, not the *absence* of any listen
    /// configuration, which still means the ADR 0008 zero-config default.
    /// Exists for docs/recon/cloudron-fit.md's hard constraint: usv must
    /// start and pass the HTTP health check when a Cloudron admin disables
    /// the `GEMINI_PORT` tcpPorts service (`GEMINI_PORT` then absent from
    /// the environment; `start.sh` maps that to `USV_LISTEN=""`).
    pub listen: Vec<SocketAddr>,
    /// The port clients are expected to name in request URLs (the authority
    /// check accepts it explicitly or, when it is 1965, omitted). On port-
    /// remapping platforms this is the *external* port; standalone it is the
    /// listen port. Defaults to the first listen address's port. The value 0
    /// means "derive from the actually bound listener" — the ephemeral-port
    /// case the regress suite depends on; the binary substitutes the real
    /// port after binding.
    pub advertised_port: u16,
    /// Minimum TLS version. 1.3 unless the operator opts down to 1.2
    /// (docs/recon/protocol.md "Implementation guidance" §4).
    pub tls_min: TlsMinVersion,
    /// How much of a visitor's address the request log may carry.
    /// Defaults to [`PeerLogging::Off`] (OQ-9).
    pub log_peer: PeerLogging,
    /// The gopher listener, if the operator enabled it. `None` — the
    /// default — means no cleartext service at all (ADR 0012 §2).
    pub gopher: Option<GopherConfig>,
    /// The finger listener, if enabled. Off by default like every other
    /// cleartext service.
    pub finger: Option<FingerConfig>,
    /// Hostnames this server answers for (authority check layer 3; SNI cert
    /// selection). Requests naming any other host get status 53.
    pub hosts: Vec<HostConfig>,
    /// The hostname the render pipeline advertises in generated absolute
    /// links (Atom self-link, sitemap.xml, /llms.txt, robots.txt's Sitemap
    /// directive) — `None` means the first configured host, as before this
    /// field existed. Exists for the case where that isn't the name a
    /// reader should actually use: a capsule reachable at a real hostname
    /// *and* mirrored as a Tor onion service wants its feeds to advertise
    /// the `.onion` name, not whichever `[[host]]` happens to be first
    /// (docs/notes/integration-ideas.md "Tor / I2P"). Does not affect
    /// authority checking or SNI cert selection — those still key off the
    /// real `[[host]]` entries; a Tor deployment adds a `[[host]]` for the
    /// onion address like any other hostname (`validate_hostname` already
    /// accepts onion-shaped labels — no separate acceptance rule needed).
    pub advertised_host: Option<String>,
    /// Maximum concurrent Gemini connections; excess connections wait in the
    /// accept backlog rather than being handed a task.
    pub max_connections: usize,
    /// Seconds a client gets to deliver the complete request line.
    pub request_timeout_secs: u64,
    /// Seconds the whole response write (header + body + close_notify) may
    /// take before the connection is abandoned.
    pub response_timeout_secs: u64,
    /// The HTTP surface's listen address (C3; ADR 0008). `None` standalone
    /// by default — "a pure-Gemini operator shouldn't get a web server
    /// they didn't ask for" — set explicitly to turn it on. The Cloudron
    /// profile always sets this (the dashboard tile depends on it).
    pub http_listen: Option<SocketAddr>,
    /// The bundled theme the HTML surface renders with (C3). An unknown
    /// name is a startup error, never a silent fall back to the default —
    /// a typo'd theme should say so, not quietly serve the wrong one.
    pub theme: &'static crate::render::theme::Theme,
    /// The capsule's language as a BCP 47 tag (ADR 0010). Sets the HTML
    /// `lang` attribute and the `lang` parameter on `text/gemini`
    /// responses. Screen readers pick pronunciation rules from this, so
    /// the default of `en` being wrong for a capsule is a real
    /// accessibility defect rather than a cosmetic one.
    pub lang: String,
    /// Named client identities: labels, rotation state, and capabilities
    /// (ADR 0011). Empty unless the operator defines `[[identity]]`
    /// entries; zones may still name raw fingerprints without one.
    pub roster: crate::roster::Roster,
}

/// Minimum accepted TLS protocol version (ADR 0001 / recon guidance §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMinVersion {
    /// TLS 1.2 — explicit operator opt-in only; the spec floor.
    V1_2,
    /// TLS 1.3 — the default. Client certificates over 1.2 travel in
    /// cleartext, which is the contested-issue #12 concern.
    V1_3,
}

/// The gopher listener's configuration (ADR 0012).
///
/// Cleartext, and therefore off unless the operator says otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GopherConfig {
    /// Where to bind. Defaults to `0.0.0.0:7070` — deliberately *not*
    /// the canonical port 70, which is privileged and would cost ADR
    /// 0002's empty `CapabilityBoundingSet`. Reaching port 70 is
    /// documented (socket activation, a NAT redirect, Cloudron's
    /// `tcpPorts`) rather than defaulted.
    pub listen: SocketAddr,
    /// The selector prefix this listener serves from. A root inside a
    /// gated prefix is a startup error (ADR 0012 §6 as amended).
    pub root: String,
    /// The port menus advertise, which on a port-remapping platform is
    /// not the bound one. Defaults to the bound port.
    pub advertised_port: u16,
}

/// The finger listener's configuration (ADR 0012).
///
/// Finger answers "what is this?" with a few lines of text; it does not
/// serve the content tree, so it has no root and no gate — there is
/// nothing for a gated path to leak through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FingerConfig {
    /// Where to bind. Defaults to `0.0.0.0:7979` — port 79 is
    /// privileged, and the same reasoning as gopher applies.
    pub listen: SocketAddr,
}

/// How much of a visitor's address the request log may carry (OQ-9).
///
/// Geminispace's stated norm is aggressive log minimalism — operators
/// routinely make a point of *not* retaining visitor addresses
/// (`docs/recon/community-wisdom.md` §3). usv's own request line was
/// already query-redacted by construction, since status 10/11 input
/// lands in the query and can contain anything a visitor types; the
/// address was the remaining durable identifier, and the default now
/// matches the norm rather than the habit inherited from web servers.
///
/// The operator can still opt back in: an abuse investigation is a real
/// need, and this is a choice they should make deliberately rather than
/// discover they had already made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PeerLogging {
    /// **Default.** No address in the logs at all; the field renders as
    /// `-`, so the line shape stays stable and greppable.
    #[default]
    Off,
    /// A short digest of the address under a salt generated fresh at
    /// every start. Repeat visits correlate *within one run of the
    /// process* — enough to see one client hammering a path — and
    /// nothing survives a restart, because the salt does not.
    Hashed,
    /// The address, verbatim. Everything a conventional access log
    /// keeps. Deliberately the value an operator has to type out.
    Full,
}

/// Per-hostname configuration: one `[[host]]` table each.
#[derive(Debug, Clone)]
pub struct HostConfig {
    /// The hostname, as clients will send it: lowercase ASCII, punycoded if
    /// internationalized, no port. Matched case-insensitively.
    pub name: String,
    /// Where this host's static content lives. Defaults to
    /// `${state_dir}/content` (ADR 0008); a host-specified `docroot` is
    /// resolved relative to `state_dir` if relative, used as-is if
    /// absolute.
    pub docroot: PathBuf,
    /// Redirect rules, tried in config order (C2; `handler::redirect`).
    pub redirects: Vec<crate::handler::redirect::Rule>,
    /// Certificate zones, longest-prefix-matched (C2; `handler::cert_zone`).
    pub cert_zones: Vec<crate::handler::cert_zone::Zone>,
    /// Writable Titan upload zones (C4; `handler::titan`). Empty means this
    /// host accepts no uploads at all — the default, and the only state in
    /// which no certificate can write anything (ADR 0006).
    pub titan_zones: Vec<crate::handler::titan::Zone>,
}

/// Why configuration loading failed. Every variant renders as one actionable
/// startup error; there are no warnings in this module by design.
#[derive(Debug)]
pub enum ConfigError {
    /// The file was named (flag or env) but could not be read.
    Unreadable(PathBuf, std::io::Error),
    /// TOML syntax or unknown-key rejection, with the file's path for
    /// context. Unknown keys land here via serde's deny_unknown_fields.
    Invalid(String, String),
    /// A `[responses]` section is present but the responses feature has no
    /// release assignment yet (ADR 0009).
    ResponsesReserved,
    /// A value parsed but fails validation (bad hostname, an unparseable
    /// listen address, zero connection cap…). An *empty* listen list is not
    /// one of these — it is the explicit "no Gemini listener" state, see
    /// [`Config::listen`]'s own docs.
    Rejected(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Unreadable(p, e) => {
                write!(f, "config file {} cannot be read: {e}", p.display())
            }
            ConfigError::Invalid(src, e) => write!(f, "config {src} is invalid: {e}"),
            ConfigError::ResponsesReserved => write!(
                f,
                "config has a [responses] section, but the responses feature is not \
                 implemented yet (ADR 0009 records the design). Remove the section for \
                 now — it is reserved so your future settings can never be silently ignored"
            ),
            ConfigError::Rejected(msg) => write!(f, "config rejected: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The serde-facing shape of `usv.toml`. Separate from [`Config`] so that
/// defaults, env layering, and validation happen in exactly one place
/// ([`Config::resolve`]) instead of being scattered through serde attributes.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    server: Option<RawServer>,
    #[serde(default, rename = "host")]
    hosts: Vec<RawHost>,
    /// `[[identity]]` — the roster (ADR 0011).
    #[serde(default, rename = "identity")]
    identities: Vec<RawIdentity>,
    /// Server-wide Titan defaults (C4; ADR 0006). Writable zones themselves
    /// are per-host (`[[host.titan_zone]]`) because writable paths belong
    /// to a host's content tree.
    titan: Option<RawTitan>,
    /// `[gopher]` — the first cleartext listener (ADR 0012). Absent
    /// means off, which is also what `enabled = false` means: a capsule
    /// that says nothing gets no cleartext service.
    gopher: Option<RawGopher>,
    /// `[finger]` — a person's status, not the content tree (ADR 0012).
    finger: Option<RawFinger>,
    /// Reserved (ADR 0009); presence is a startup error.
    responses: Option<toml::Table>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServer {
    state_dir: Option<PathBuf>,
    listen: Option<Vec<String>>,
    advertised_port: Option<u16>,
    tls_min: Option<String>,
    log_peer: Option<String>,
    max_connections: Option<usize>,
    request_timeout_secs: Option<u64>,
    response_timeout_secs: Option<u64>,
    http_listen: Option<String>,
    theme: Option<String>,
    lang: Option<String>,
    advertised_host: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHost {
    name: String,
    docroot: Option<PathBuf>,
    #[serde(default, rename = "redirect")]
    redirects: Vec<RawRedirect>,
    #[serde(default, rename = "cert_zone")]
    cert_zones: Vec<RawCertZone>,
    #[serde(default, rename = "titan_zone")]
    titan_zones: Vec<RawTitanZone>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRedirect {
    pattern: String,
    target: String,
    #[serde(default)]
    permanent: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCertZone {
    path_prefix: String,
    #[serde(default)]
    fingerprints: Vec<String>,
}

/// `[titan]` — server-wide upload defaults (C4; ADR 0006).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTitan {
    /// Default per-upload cap for zones that name none. Falls back to
    /// `handler::titan::DEFAULT_MAX_UPLOAD_BYTES` (10 MiB, the GmCapsule
    /// default) when unset.
    max_upload_bytes: Option<u64>,
}

/// `[[host.titan_zone]]` — one writable upload zone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTitanZone {
    path_prefix: String,
    /// Roster identity labels permitted to write here (ADR 0011).
    #[serde(default)]
    identities: Vec<String>,
    /// SHA-256 fingerprints permitted to write here. Unlike a cert_zone,
    /// an empty list is a startup error, never "any valid certificate" —
    /// see `handler::titan`.
    #[serde(default)]
    fingerprints: Vec<String>,
    max_upload_bytes: Option<u64>,
    mime: Option<Vec<String>>,
    token: Option<String>,
    #[serde(default)]
    allow_delete: bool,
}

/// `[gopher]` — the cleartext gopher listener (ADR 0012).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGopher {
    enabled: Option<bool>,
    listen: Option<String>,
    root: Option<String>,
    advertised_port: Option<u16>,
}

/// `[finger]` — the cleartext finger listener (ADR 0012).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFinger {
    enabled: Option<bool>,
    listen: Option<String>,
}

/// `[[identity]]` — one named client identity (ADR 0011).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIdentity {
    label: String,
    fingerprint: String,
    /// Fingerprints being retired; requires `superseded_until`.
    #[serde(default)]
    superseded: Vec<String>,
    /// `YYYY-MM-DD` — the day the rotation window closes.
    superseded_until: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    /// `YYYY-MM-DD` — provenance only; usv records when a key was added,
    /// never who it belongs to (ADR 0011: continuity, not attestation).
    enrolled: Option<String>,
}

/// Environment override names honored by the core (ADR 0007). Kept in one
/// place so the docs and the code cannot drift.
pub mod env_keys {
    /// Path to the config file (beaten only by `--config`).
    pub const CONFIG: &str = "USV_CONFIG";
    /// Overrides `server.state_dir`.
    pub const STATE_DIR: &str = "USV_STATE_DIR";
    /// Overrides `server.listen`: comma-separated socket addresses.
    pub const LISTEN: &str = "USV_LISTEN";
    /// Overrides `server.advertised_port`.
    pub const ADVERTISED_PORT: &str = "USV_ADVERTISED_PORT";
    /// Overrides the host list with a single hostname (the common
    /// platform-injected case: one app, one domain).
    pub const HOSTNAME: &str = "USV_HOSTNAME";
    /// Overrides `server.http_listen`. The Cloudron profile always sets
    /// this via `start.sh` — the dashboard tile depends on the HTTP
    /// surface being live.
    pub const HTTP_LISTEN: &str = "USV_HTTP_LISTEN";

    /// Where the gopher listener binds. Setting it to a non-empty value
    /// **enables gopher** — the platform equivalent of `[gopher]
    /// enabled = true`, for deployments where the port is injected
    /// rather than configured (Cloudron's `tcpPorts`). Empty or unset
    /// leaves gopher off, which stays the default everywhere.
    pub const GOPHER_LISTEN: &str = "USV_GOPHER_LISTEN";
    /// The gopher port to advertise in menus, when the platform maps the
    /// bound port to a different external one.
    pub const GOPHER_ADVERTISED_PORT: &str = "USV_GOPHER_ADVERTISED_PORT";
}

/// A snapshot of the `USV_*` environment, taken once at load time so the
/// resolution logic is a pure function that tests can drive without touching
/// the process environment.
#[derive(Debug, Default, Clone)]
pub struct EnvOverrides {
    /// See [`env_keys::STATE_DIR`].
    pub state_dir: Option<PathBuf>,
    /// See [`env_keys::LISTEN`].
    pub listen: Option<String>,
    /// See [`env_keys::ADVERTISED_PORT`].
    pub advertised_port: Option<String>,
    /// See [`env_keys::HOSTNAME`].
    pub hostname: Option<String>,
    /// See [`env_keys::HTTP_LISTEN`].
    pub http_listen: Option<String>,
    /// See [`env_keys::GOPHER_LISTEN`].
    pub gopher_listen: Option<String>,
    /// See [`env_keys::GOPHER_ADVERTISED_PORT`].
    pub gopher_advertised_port: Option<String>,
}

impl EnvOverrides {
    /// Capture the `USV_*` overrides from the process environment.
    pub fn from_process_env() -> Self {
        EnvOverrides {
            state_dir: std::env::var_os(env_keys::STATE_DIR).map(PathBuf::from),
            listen: std::env::var(env_keys::LISTEN).ok(),
            advertised_port: std::env::var(env_keys::ADVERTISED_PORT).ok(),
            hostname: std::env::var(env_keys::HOSTNAME).ok(),
            http_listen: std::env::var(env_keys::HTTP_LISTEN).ok(),
            gopher_listen: std::env::var(env_keys::GOPHER_LISTEN).ok(),
            gopher_advertised_port: std::env::var(env_keys::GOPHER_ADVERTISED_PORT).ok(),
        }
    }
}

/// Parse a `YYYY-MM-DD` config date, naming the field it came from so a
/// malformed value points straight at itself.
fn parse_config_date(
    value: Option<&str>,
    label: &str,
    field: &str,
) -> Result<Option<time::Date>, ConfigError> {
    let Some(text) = value else {
        return Ok(None);
    };
    let format = time::macros::format_description!("[year]-[month]-[day]");
    time::Date::parse(text, &format).map(Some).map_err(|_| {
        ConfigError::Rejected(format!(
            "identity {label:?} has {field} = {text:?}, which is not a YYYY-MM-DD date"
        ))
    })
}

/// The default state directory for this process (ADR 0008): the XDG state
/// home for a user session, `/var/lib/usv` when there is no home to anchor
/// to (system services).
pub fn default_state_dir(env: &EnvOverrides) -> PathBuf {
    if let Some(dir) = &env.state_dir {
        return dir.clone();
    }
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join("usv");
    }
    if let Some(home) = std::env::var_os("HOME")
        && !home.is_empty()
    {
        return PathBuf::from(home).join(".local/state/usv");
    }
    PathBuf::from("/var/lib/usv")
}

impl Config {
    /// Load configuration: locate the file per the ADR 0007 search order,
    /// parse it, apply env overrides, validate. `flag_path` is `--config`.
    pub fn load(flag_path: Option<&Path>, env: &EnvOverrides) -> Result<Config, ConfigError> {
        let explicit = flag_path
            .map(PathBuf::from)
            .or_else(|| std::env::var_os(env_keys::CONFIG).map(PathBuf::from));
        let (raw, source) = match explicit {
            // An explicitly named file must exist; a missing default is fine.
            Some(path) => {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| ConfigError::Unreadable(path.clone(), e))?;
                (Self::parse(&text, &path.display().to_string())?, Some(path))
            }
            None => {
                let path = default_state_dir(env).join("usv.toml");
                match std::fs::read_to_string(&path) {
                    Ok(text) => (Self::parse(&text, &path.display().to_string())?, Some(path)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        (RawConfig::default(), None)
                    }
                    Err(e) => return Err(ConfigError::Unreadable(path, e)),
                }
            }
        };
        if let Some(path) = source {
            tracing::info!(config = %path.display(), "configuration file loaded");
        } else {
            tracing::info!("no configuration file found; built-in defaults apply (ADR 0007)");
        }
        Self::resolve(raw, env)
    }

    /// Parse TOML text into the raw shape. This function must never panic
    /// on any input; the fuzz target drives [`Config::from_toml_str`].
    fn parse(text: &str, source: &str) -> Result<RawConfig, ConfigError> {
        toml::from_str(text).map_err(|e| ConfigError::Invalid(source.to_string(), e.to_string()))
    }

    /// Fuzz/testing entry: full pipeline from TOML text to validated config,
    /// with no filesystem access.
    pub fn from_toml_str(text: &str, env: &EnvOverrides) -> Result<Config, ConfigError> {
        Self::resolve(Self::parse(text, "<string>")?, env)
    }

    fn resolve(raw: RawConfig, env: &EnvOverrides) -> Result<Config, ConfigError> {
        if raw.responses.is_some() {
            return Err(ConfigError::ResponsesReserved);
        }
        let server = raw.server.unwrap_or_default();
        // Server-wide Titan default, applied to zones that name no cap.
        let gopher_raw = raw.gopher;
        let finger_raw = raw.finger;
        let titan_default_max_upload = raw.titan.unwrap_or_default().max_upload_bytes;

        let state_dir = env
            .state_dir
            .clone()
            .or(server.state_dir)
            .unwrap_or_else(|| default_state_dir(&EnvOverrides::default()));

        // An explicitly empty list — `USV_LISTEN=""`, or `server.listen = []`
        // in the file — means "no Gemini listener", not "use the default":
        // the platform-injected-facts case (ADR 0007) a Cloudron admin
        // disabling the tcpPorts service maps onto (docs/recon/cloudron-fit.md
        // hard constraint: usv must still start and pass the health check,
        // HTTP-only, when the Gemini port is off). *Absence* of any listen
        // configuration anywhere still means the zero-config ADR 0008
        // default (1965 on both families) — the two must stay distinct, or
        // a bare `usv` with no config would silently stop serving Gemini.
        let listen_strings: Vec<String> = match &env.listen {
            Some(s) if s.trim().is_empty() => Vec::new(),
            Some(s) => s.split(',').map(|a| a.trim().to_string()).collect(),
            None => server
                .listen
                .unwrap_or_else(|| vec!["0.0.0.0:1965".into(), "[::]:1965".into()]),
        };
        let mut listen = Vec::with_capacity(listen_strings.len());
        for s in &listen_strings {
            let addr: SocketAddr = s.parse().map_err(|_| {
                ConfigError::Rejected(format!(
                    "listen address {s:?} is not a socket address (expected e.g. \
                     \"0.0.0.0:1965\" or \"[::]:1965\")"
                ))
            })?;
            listen.push(addr);
        }

        let advertised_port = match &env.advertised_port {
            Some(s) => s.parse().map_err(|_| {
                ConfigError::Rejected(format!(
                    "{} value {s:?} is not a port number",
                    env_keys::ADVERTISED_PORT
                ))
            })?,
            // With no listener at all (Gemini disabled), there is no "first
            // listen port" to fall back to; the value is moot for routing
            // in that state, but must still be a sane, non-panicking
            // default in case Gemini is re-enabled without ever setting
            // this explicitly.
            None => server.advertised_port.unwrap_or_else(|| {
                listen
                    .first()
                    .map_or(crate::protocol::GEMINI_DEFAULT_PORT, |a| a.port())
            }),
        };

        let tls_min = match server.tls_min.as_deref() {
            None | Some("1.3") => TlsMinVersion::V1_3,
            Some("1.2") => TlsMinVersion::V1_2,
            Some(other) => {
                return Err(ConfigError::Rejected(format!(
                    "server.tls_min {other:?} is not supported: \"1.3\" (default) or the \
                     \"1.2\" opt-in are the only values (the spec floor is 1.2)"
                )));
            }
        };

        let log_peer = match server.log_peer.as_deref() {
            None | Some("off") => PeerLogging::Off,
            Some("hashed") => PeerLogging::Hashed,
            Some("full") => PeerLogging::Full,
            Some(other) => {
                return Err(ConfigError::Rejected(format!(
                    "server.log_peer {other:?} is not a valid setting: \"off\" (default — no \
                     visitor address in the logs), \"hashed\" (a per-boot-salted digest that \
                     correlates repeat visits within one run and survives no restart), or \
                     \"full\" (the address verbatim)"
                )));
            }
        };

        // The roster is built before hosts, so a zone can be checked
        // against it for a mistyped identity label at startup.
        let mut identities = Vec::with_capacity(raw.identities.len());
        for raw_id in raw.identities {
            let mut capabilities = Vec::with_capacity(raw_id.capabilities.len());
            for name in &raw_id.capabilities {
                let cap = crate::roster::Capability::parse(name).ok_or_else(|| {
                    ConfigError::Rejected(
                        crate::roster::RosterError::UnknownCapability {
                            label: raw_id.label.clone(),
                            value: name.clone(),
                        }
                        .to_string(),
                    )
                })?;
                capabilities.push(cap);
            }
            identities.push(crate::roster::Identity {
                label: raw_id.label.clone(),
                fingerprint: raw_id.fingerprint,
                superseded: raw_id.superseded,
                superseded_until: parse_config_date(
                    raw_id.superseded_until.as_deref(),
                    &raw_id.label,
                    "superseded_until",
                )?,
                capabilities,
                enrolled: parse_config_date(raw_id.enrolled.as_deref(), &raw_id.label, "enrolled")?,
            });
        }
        let roster = crate::roster::Roster::new(identities)
            .map_err(|e| ConfigError::Rejected(e.to_string()))?;

        let bare = |name: &str| RawHost {
            name: name.to_string(),
            docroot: None,
            redirects: Vec::new(),
            cert_zones: Vec::new(),
            titan_zones: Vec::new(),
        };
        let raw_hosts: Vec<RawHost> = match &env.hostname {
            // USV_HOSTNAME overrides the *name*, but a file-configured host
            // still contributes its docroot/redirects/cert_zones when the
            // names happen to line up; the common platform case is a single
            // configured host whose name the env fact replaces.
            Some(name) => {
                if let Some(mut h) = raw
                    .hosts
                    .into_iter()
                    .find(|h| h.name.eq_ignore_ascii_case(name))
                {
                    h.name = name.clone();
                    vec![h]
                } else {
                    vec![bare(name)]
                }
            }
            None if raw.hosts.is_empty() => vec![bare("localhost")],
            None => raw.hosts,
        };
        let mut hosts = Vec::with_capacity(raw_hosts.len());
        for raw_host in raw_hosts {
            let name = validate_hostname(&raw_host.name)?;
            let docroot = match raw_host.docroot {
                Some(d) if d.is_absolute() => d,
                Some(d) => state_dir.join(d),
                None => state_dir.join("content"),
            };
            let mut redirects = Vec::with_capacity(raw_host.redirects.len());
            for r in raw_host.redirects {
                let rule = crate::handler::redirect::Rule::new(&r.pattern, &r.target, r.permanent)
                    .map_err(|e| ConfigError::Rejected(format!("host {name:?} redirect: {e}")))?;
                redirects.push(rule);
            }
            let cert_zones = raw_host
                .cert_zones
                .into_iter()
                .map(|z| crate::handler::cert_zone::Zone {
                    path_prefix: z.path_prefix,
                    allowed_fingerprints: z.fingerprints,
                })
                .collect();
            let mut titan_zones = Vec::with_capacity(raw_host.titan_zones.len());
            for z in raw_host.titan_zones {
                // A zone's own cap wins; otherwise the server-wide default;
                // otherwise the bundled 10 MiB. Validation (non-empty
                // allowlist above all) lives in the handler with the rest of
                // the upload policy, so config and enforcement cannot drift.
                // A mistyped identity label would silently authorize
                // nobody, so it is a startup error. The *capability* is
                // deliberately NOT checked here: revoking `titan-write`
                // from an identity should disable it everywhere at once,
                // not refuse to start (ADR 0011).
                for label in &z.identities {
                    if roster.by_label(label).is_none() {
                        return Err(ConfigError::Rejected(format!(
                            "host {name:?} titan_zone {:?} names identity {label:?}, which is \
                             not defined in any [[identity]] section",
                            z.path_prefix
                        )));
                    }
                }
                let zone = crate::handler::titan::Zone::new(crate::handler::titan::ZoneSpec {
                    path_prefix: z.path_prefix,
                    fingerprints: z.fingerprints,
                    identities: z.identities,
                    max_upload_bytes: z.max_upload_bytes.or(titan_default_max_upload),
                    allowed_mime: z.mime,
                    token: z.token,
                    allow_delete: z.allow_delete,
                })
                .map_err(|e| ConfigError::Rejected(format!("host {name:?}: {e}")))?;
                titan_zones.push(zone);
            }
            hosts.push(HostConfig {
                name,
                docroot,
                redirects,
                cert_zones,
                titan_zones,
            });
        }

        let max_connections = server.max_connections.unwrap_or(512);
        if max_connections == 0 {
            return Err(ConfigError::Rejected(
                "server.max_connections must be at least 1".into(),
            ));
        }

        let http_listen_str = env.http_listen.clone().or(server.http_listen);
        let http_listen = match http_listen_str {
            Some(s) => Some(s.parse::<SocketAddr>().map_err(|_| {
                ConfigError::Rejected(format!(
                    "server.http_listen value {s:?} is not a socket address \
                     (expected e.g. \"0.0.0.0:8000\")"
                ))
            })?),
            None => None,
        };

        let theme_name = server
            .theme
            .as_deref()
            .unwrap_or(crate::render::theme::DEFAULT_THEME_NAME);
        let theme = crate::render::theme::find(theme_name).ok_or_else(|| {
            let known: Vec<&str> = crate::render::theme::THEMES
                .iter()
                .map(|t| t.name)
                .collect();
            ConfigError::Rejected(format!(
                "server.theme {theme_name:?} is not a bundled theme (known: {})",
                known.join(", ")
            ))
        })?;

        let lang = server.lang.unwrap_or_else(|| "en".to_string());
        if !is_plausible_lang(&lang) {
            return Err(ConfigError::Rejected(format!(
                "server.lang {lang:?} is not a BCP 47 language tag (e.g. \"en\", \"fr\", \"pt-BR\")"
            )));
        }

        // Same validation and normalization as any `[[host]]` name — this
        // is a name used in generated URLs, not a routing key, but there is
        // no reason its rules should differ (see the field's own docs).
        let advertised_host = server
            .advertised_host
            .as_deref()
            .map(validate_hostname)
            .transpose()?;

        // Built last: the wall (ADR 0012 §6) needs the resolved hosts,
        // because what may not be published in the clear is defined by
        // their certificate and Titan zones.
        // A platform that injects the port (Cloudron's tcpPorts) has no
        // file to edit, so a non-empty USV_GOPHER_LISTEN enables gopher
        // on its own — the same shape as USV_LISTEN for Gemini, where
        // empty means explicitly off and absent means "use the file".
        let env_gopher_listen = env.gopher_listen.as_deref().map(str::trim);
        let gopher_raw = match (gopher_raw, env_gopher_listen) {
            (_, Some("")) => None,
            (existing, Some(addr)) => {
                let mut g = existing.unwrap_or_default();
                g.enabled = Some(true);
                g.listen = Some(addr.to_string());
                Some(g)
            }
            (existing, None) => existing,
        };
        let gopher = match gopher_raw {
            None => None,
            Some(g) if !g.enabled.unwrap_or(false) => None,
            Some(g) => {
                let listen_str = g.listen.as_deref().unwrap_or("0.0.0.0:7070");
                let listen: SocketAddr = listen_str.parse().map_err(|_| {
                    ConfigError::Rejected(format!(
                        "gopher.listen {listen_str:?} is not a valid socket address \
                         (e.g. \"0.0.0.0:7070\")"
                    ))
                })?;
                let root = g.root.as_deref().unwrap_or("/").to_string();
                // A cleartext root pointing into a gated prefix is an
                // instruction to publish gated content in the clear, so it
                // is refused here rather than silently emptied later.
                for host in &hosts {
                    let gate = crate::render::cleartext::Gate::for_host(host);
                    crate::render::cleartext::check_cleartext_root("gopher", &root, &gate)
                        .map_err(ConfigError::Rejected)?;
                }
                let advertised_port = env
                    .gopher_advertised_port
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .and_then(|v| v.parse::<u16>().ok())
                    .or(g.advertised_port)
                    .unwrap_or_else(|| listen.port());
                Some(GopherConfig {
                    listen,
                    root,
                    advertised_port,
                })
            }
        };

        let finger = match finger_raw {
            None => None,
            Some(f) if !f.enabled.unwrap_or(false) => None,
            Some(f) => {
                let listen_str = f.listen.as_deref().unwrap_or("0.0.0.0:7979");
                let listen: SocketAddr = listen_str.parse().map_err(|_| {
                    ConfigError::Rejected(format!(
                        "finger.listen {listen_str:?} is not a valid socket address \
                         (e.g. \"0.0.0.0:7979\")"
                    ))
                })?;
                Some(FingerConfig { listen })
            }
        };

        Ok(Config {
            state_dir,
            listen,
            advertised_port,
            tls_min,
            log_peer,
            gopher,
            finger,
            hosts,
            advertised_host,
            max_connections,
            request_timeout_secs: server.request_timeout_secs.unwrap_or(10),
            response_timeout_secs: server.response_timeout_secs.unwrap_or(300),
            http_listen,
            theme,
            lang,
            roster,
        })
    }

    /// The certs directory (ADR 0003): `${state_dir}/certs`.
    pub fn certs_dir(&self) -> PathBuf {
        self.state_dir.join("certs")
    }

    /// Case-insensitive membership test for the authority check (layer 3).
    pub fn serves_host(&self, name: &str) -> bool {
        self.hosts.iter().any(|h| h.name.eq_ignore_ascii_case(name))
    }

    /// Case-insensitive lookup of a host's full configuration (docroot,
    /// redirects, cert zones) for request dispatch (C2).
    pub fn find_host(&self, name: &str) -> Option<&HostConfig> {
        self.hosts
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
    }
}

/// Not a real BCP 47 parser (recon: no such validator was judged worth a
/// dependency for this) — the same structural sanity check `Config::
/// resolve` applies: non-empty, ASCII. `pub(crate)` so `cli::init` can
/// reject the same bad input the wizard would otherwise let through to a
/// config `Config::resolve` then refuses.
pub(crate) fn is_plausible_lang(lang: &str) -> bool {
    !lang.trim().is_empty() && lang.is_ascii()
}

/// Validate and normalize (lowercase) a configured hostname. Clients send
/// punycoded, ASCII hostnames on the wire (the spec is URI-based, not
/// IRI-based), so that is the only form config accepts; rejecting Unicode
/// here catches the "typed my IDN in directly" mistake loudly at startup.
///
/// `pub(crate)`: `cli::init` reuses this so the wizard rejects a bad
/// hostname with the exact same rule `Config::resolve` would apply to the
/// file it's about to write — one definition, not a second that could
/// accept something the real loader then refuses.
pub(crate) fn validate_hostname(name: &str) -> Result<String, ConfigError> {
    let reject = |why: &str| {
        Err(ConfigError::Rejected(format!(
            "hostname {name:?} {why} (hostnames are sent punycoded and portless on the \
             wire; e.g. \"example.org\" or \"xn--mller-kva.example\")"
        )))
    };
    if name.is_empty() || name.len() > 253 {
        return reject("must be 1–253 characters");
    }
    if !name.is_ascii() {
        return reject("contains non-ASCII characters");
    }
    if name.contains(':') {
        return reject("must not include a port");
    }
    for label in name.split('.') {
        if label.is_empty() || label.len() > 63 {
            return reject("has an empty or over-63-byte label");
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return reject("may only contain letters, digits, hyphens, and dots");
        }
        if label.starts_with('-') || label.ends_with('-') {
            return reject("has a label starting or ending with a hyphen");
        }
    }
    Ok(name.to_ascii_lowercase())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn no_env() -> EnvOverrides {
        EnvOverrides::default()
    }

    #[test]
    fn defaults_stand_alone() {
        let cfg = Config::from_toml_str("", &no_env()).expect("empty config is valid");
        assert_eq!(cfg.advertised_port, 1965);
        assert_eq!(cfg.tls_min, TlsMinVersion::V1_3);
        assert_eq!(cfg.hosts.len(), 1);
        assert_eq!(cfg.hosts[0].name, "localhost");
        assert_eq!(cfg.listen.len(), 2);
    }

    #[test]
    fn unknown_top_level_key_is_a_startup_error() {
        let err = Config::from_toml_str("sever = {}", &no_env()).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(..)), "{err}");
    }

    #[test]
    fn unknown_server_key_is_a_startup_error() {
        let err = Config::from_toml_str("[server]\nmax_conections = 5", &no_env()).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(..)), "{err}");
    }

    #[test]
    fn no_titan_zones_means_nothing_is_writable() {
        // The default posture: a capsule that never mentions Titan cannot
        // be written to by anyone (ADR 0006).
        let cfg = Config::from_toml_str("[[host]]\nname = \"a.example\"\n", &no_env()).unwrap();
        assert!(cfg.hosts[0].titan_zones.is_empty());
    }

    #[test]
    fn a_titan_zone_parses_with_its_defaults() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"a.example\"\n\
             [[host.titan_zone]]\npath_prefix = \"/uploads/\"\nfingerprints = [\"aabb\"]\n",
            &no_env(),
        )
        .unwrap();
        let zone = &cfg.hosts[0].titan_zones[0];
        assert_eq!(zone.path_prefix, "/uploads/");
        assert_eq!(zone.allowed_fingerprints, vec!["aabb".to_string()]);
        assert_eq!(
            zone.max_upload_bytes,
            crate::handler::titan::DEFAULT_MAX_UPLOAD_BYTES
        );
        assert!(!zone.allow_delete, "deletion is opt-in");
        assert!(zone.token.is_none());
    }

    #[test]
    fn a_titan_zone_without_fingerprints_is_a_startup_error() {
        // The asymmetry with cert_zone that matters: a writable zone may
        // never be left open to any certificate.
        let err = Config::from_toml_str(
            "[[host]]\nname = \"a.example\"\n\
             [[host.titan_zone]]\npath_prefix = \"/uploads/\"\n",
            &no_env(),
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Rejected(_)), "{err}");
        assert!(err.to_string().contains("fingerprint"), "{err}");
    }

    #[test]
    fn the_server_wide_titan_cap_is_a_default_zones_can_override() {
        let cfg = Config::from_toml_str(
            "[titan]\nmax_upload_bytes = 4096\n\
             [[host]]\nname = \"a.example\"\n\
             [[host.titan_zone]]\npath_prefix = \"/small/\"\nfingerprints = [\"aa\"]\n\
             [[host.titan_zone]]\npath_prefix = \"/big/\"\nfingerprints = [\"aa\"]\nmax_upload_bytes = 999999\n",
            &no_env(),
        )
        .unwrap();
        let zones = &cfg.hosts[0].titan_zones;
        let small = zones.iter().find(|z| z.path_prefix == "/small/").unwrap();
        let big = zones.iter().find(|z| z.path_prefix == "/big/").unwrap();
        assert_eq!(small.max_upload_bytes, 4096, "inherits the server default");
        assert_eq!(big.max_upload_bytes, 999_999, "own value wins");
    }

    /// A syntactically valid SHA-256 fingerprint of one repeated nibble.
    fn hex64(nibble: char) -> String {
        std::iter::repeat_n(nibble, 64).collect()
    }

    #[test]
    fn an_identity_parses_with_capabilities_and_rotation() {
        let cfg = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"scribe\"\nfingerprint = \"{}\"\n\
                 superseded = [\"{}\"]\nsuperseded_until = \"2026-09-01\"\n\
                 capabilities = [\"titan-write\"]\nenrolled = \"2026-08-10\"\n",
                hex64('a'),
                hex64('b')
            ),
            &no_env(),
        )
        .unwrap();
        let id = cfg.roster.by_label("scribe").expect("identity present");
        assert_eq!(id.fingerprint, hex64('a'));
        assert_eq!(id.superseded, vec![hex64('b')]);
        assert!(id.superseded_until.is_some());
        assert!(id.holds(crate::roster::Capability::TitanWrite));
        assert!(id.enrolled.is_some(), "provenance date is recorded");
    }

    #[test]
    fn an_unknown_capability_is_a_startup_error_that_lists_the_real_ones() {
        let err = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"x\"\nfingerprint = \"{}\"\n\
                 capabilities = [\"write\"]\n",
                hex64('a')
            ),
            &no_env(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("titan-write"),
            "must name the real spelling: {msg}"
        );
    }

    #[test]
    fn rotation_without_a_deadline_is_a_startup_error() {
        let err = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"x\"\nfingerprint = \"{}\"\n\
                 superseded = [\"{}\"]\n",
                hex64('a'),
                hex64('b')
            ),
            &no_env(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("superseded_until"), "{err}");
    }

    #[test]
    fn a_truncated_fingerprint_is_a_startup_error() {
        let err = Config::from_toml_str(
            "[[identity]]\nlabel = \"x\"\nfingerprint = \"aabbcc\"\n",
            &no_env(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("64 hex"), "{err}");
    }

    #[test]
    fn a_zone_naming_an_undefined_identity_is_a_startup_error() {
        // A mistyped label would silently authorize nobody.
        let err = Config::from_toml_str(
            "[[host]]\nname = \"a.example\"\n\
             [[host.titan_zone]]\npath_prefix = \"/u/\"\nidentities = [\"typo\"]\n",
            &no_env(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("typo"), "{err}");
    }

    #[test]
    fn a_zone_may_name_a_defined_identity_instead_of_raw_fingerprints() {
        let cfg = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"scribe\"\nfingerprint = \"{}\"\n\
                 capabilities = [\"titan-write\"]\n\
                 [[host]]\nname = \"a.example\"\n\
                 [[host.titan_zone]]\npath_prefix = \"/u/\"\nidentities = [\"scribe\"]\n",
                hex64('a')
            ),
            &no_env(),
        )
        .unwrap();
        let zone = &cfg.hosts[0].titan_zones[0];
        assert_eq!(zone.allowed_identities, vec!["scribe".to_string()]);
        assert!(zone.allowed_fingerprints.is_empty());
    }

    #[test]
    fn revoking_a_capability_does_not_break_startup() {
        // Deliberate: the zone keeps naming the identity, the identity no
        // longer holds titan-write, and the server still starts — the
        // write is refused at request time instead (ADR 0011).
        let cfg = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"scribe\"\nfingerprint = \"{}\"\n\
                 capabilities = []\n\
                 [[host]]\nname = \"a.example\"\n\
                 [[host.titan_zone]]\npath_prefix = \"/u/\"\nidentities = [\"scribe\"]\n",
                hex64('a')
            ),
            &no_env(),
        );
        assert!(cfg.is_ok(), "revocation must not be a startup failure");
    }

    #[test]
    fn unknown_keys_in_a_titan_zone_are_startup_errors() {
        let err = Config::from_toml_str(
            "[[host]]\nname = \"a.example\"\n\
             [[host.titan_zone]]\npath_prefix = \"/u/\"\nfingerprints = [\"aa\"]\ntypo = 1\n",
            &no_env(),
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(..)), "{err}");
    }

    #[test]
    fn responses_section_errors_helpfully() {
        let err = Config::from_toml_str("[responses]\nmode = \"hold\"", &no_env()).unwrap_err();
        assert!(matches!(err, ConfigError::ResponsesReserved));
        assert!(err.to_string().contains("ADR 0009"));
    }

    #[test]
    fn tls_12_is_an_explicit_opt_in() {
        let cfg = Config::from_toml_str("[server]\ntls_min = \"1.2\"", &no_env()).expect("valid");
        assert_eq!(cfg.tls_min, TlsMinVersion::V1_2);
        let err = Config::from_toml_str("[server]\ntls_min = \"1.1\"", &no_env()).unwrap_err();
        assert!(err.to_string().contains("1.2"));
    }

    #[test]
    fn the_platform_can_enable_gopher_without_a_config_file() {
        // Cloudron injects the port via tcpPorts; there is no file to
        // edit, so a non-empty USV_GOPHER_LISTEN enables it on its own.
        let env = EnvOverrides {
            gopher_listen: Some("0.0.0.0:7070".into()),
            gopher_advertised_port: Some("70".into()),
            ..no_env()
        };
        let cfg = Config::from_toml_str("", &env).expect("valid");
        let g = cfg.gopher.expect("env should have enabled gopher");
        assert_eq!(g.listen.port(), 7070);
        assert_eq!(g.advertised_port, 70, "menus advertise the external port");
    }

    #[test]
    fn an_empty_gopher_listen_env_means_explicitly_off() {
        // The disabled-service case: Cloudron drops the env var, or the
        // admin turns the port off, and gopher must not come up.
        let env = EnvOverrides {
            gopher_listen: Some(String::new()),
            ..no_env()
        };
        let cfg = Config::from_toml_str("[gopher]\nenabled = true", &env).expect("valid");
        assert!(cfg.gopher.is_none(), "empty env must override the file");
    }

    #[test]
    fn gopher_is_off_unless_asked_for() {
        // ADR 0012 §2: a capsule that says nothing gets no cleartext
        // service. Absent section and enabled=false are the same answer.
        assert!(
            Config::from_toml_str("", &no_env())
                .expect("valid")
                .gopher
                .is_none()
        );
        assert!(
            Config::from_toml_str("[gopher]\nenabled = false", &no_env())
                .expect("valid")
                .gopher
                .is_none()
        );
    }

    #[test]
    fn enabling_gopher_defaults_to_a_non_privileged_port() {
        // Port 70 is privileged and would cost ADR 0002's empty
        // CapabilityBoundingSet; reaching it is documented, not defaulted.
        let cfg = Config::from_toml_str("[gopher]\nenabled = true", &no_env()).expect("valid");
        let g = cfg.gopher.expect("enabled");
        assert_eq!(g.listen.port(), 7070);
        assert_eq!(g.advertised_port, 7070);
        assert_eq!(g.root, "/");
    }

    #[test]
    fn gopher_can_advertise_a_port_it_is_not_bound_to() {
        // The port-remapping platform case, same as the Gemini side.
        let cfg = Config::from_toml_str(
            "[gopher]\nenabled = true\nlisten = \"0.0.0.0:7070\"\nadvertised_port = 70",
            &no_env(),
        )
        .expect("valid");
        let g = cfg.gopher.expect("enabled");
        assert_eq!(g.listen.port(), 7070);
        assert_eq!(g.advertised_port, 70);
    }

    #[test]
    fn a_gopher_root_inside_a_cert_zone_is_refused_at_startup() {
        // ADR 0012 §6 as amended: exclusion handles the ordinary case,
        // but pointing the cleartext root AT gated content can only mean
        // "publish this in the clear", so it is refused.
        let err = Config::from_toml_str(
            "[gopher]\nenabled = true\nroot = \"/private/\"\n\
             [[host]]\nname = \"example.org\"\n\
             [[host.cert_zone]]\npath_prefix = \"/private/\"\n",
            &no_env(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("/private/"), "{msg}");
        assert!(msg.contains("cannot authenticate a client"), "{msg}");
    }

    #[test]
    fn an_ordinary_gopher_root_is_fine_alongside_a_cert_zone() {
        // The case the blanket-error version of the ADR would have
        // broken: one private area must not make gopher unusable.
        let cfg = Config::from_toml_str(
            "[gopher]\nenabled = true\n\
             [[host]]\nname = \"example.org\"\n\
             [[host.cert_zone]]\npath_prefix = \"/private/\"\n",
            &no_env(),
        )
        .expect("a private area must not make gopher unusable");
        assert!(cfg.gopher.is_some());
    }

    #[test]
    fn a_bad_gopher_listen_address_is_refused() {
        let err = Config::from_toml_str(
            "[gopher]\nenabled = true\nlisten = \"not-an-address\"",
            &no_env(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("7070"), "must show the shape");
    }

    #[test]
    fn not_logging_visitor_addresses_is_the_default() {
        // OQ-9. The default is the privacy-preserving one, and it holds
        // for a capsule with no configuration file at all — which is the
        // configuration most capsules actually run.
        let cfg = Config::from_toml_str("", &no_env()).expect("valid");
        assert_eq!(cfg.log_peer, PeerLogging::Off);
        let cfg = Config::from_toml_str("[server]\ntheme = \"paper\"", &no_env()).expect("valid");
        assert_eq!(cfg.log_peer, PeerLogging::Off);
    }

    #[test]
    fn keeping_visitor_addresses_has_to_be_asked_for() {
        for (value, want) in [
            ("off", PeerLogging::Off),
            ("hashed", PeerLogging::Hashed),
            ("full", PeerLogging::Full),
        ] {
            let cfg =
                Config::from_toml_str(&format!("[server]\nlog_peer = \"{value}\""), &no_env())
                    .expect("valid");
            assert_eq!(cfg.log_peer, want, "log_peer = {value:?}");
        }
    }

    #[test]
    fn a_mistyped_log_peer_setting_is_refused_not_ignored() {
        // Failing open here would silently keep addresses the operator
        // believed they had turned off.
        let err = Config::from_toml_str("[server]\nlog_peer = \"none\"", &no_env()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("off"), "must list the real values: {msg}");
        assert!(msg.contains("hashed"), "must list the real values: {msg}");
        assert!(msg.contains("full"), "must list the real values: {msg}");
    }

    #[test]
    fn hosts_parse_and_normalize() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"Example.ORG\"\n[[host]]\nname = \"xn--mller-kva.example\"",
            &no_env(),
        )
        .expect("valid");
        assert!(cfg.serves_host("example.org"));
        assert!(cfg.serves_host("EXAMPLE.org"));
        assert!(cfg.serves_host("xn--mller-kva.example"));
        assert!(!cfg.serves_host("other.example"));
    }

    #[test]
    fn docroot_defaults_under_state_dir() {
        let cfg =
            Config::from_toml_str("[[host]]\nname = \"example.org\"", &no_env()).expect("valid");
        assert_eq!(cfg.hosts[0].docroot, cfg.state_dir.join("content"));
    }

    #[test]
    fn relative_docroot_resolves_under_state_dir() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"example.org\"\ndocroot = \"mysite\"",
            &no_env(),
        )
        .expect("valid");
        assert_eq!(cfg.hosts[0].docroot, cfg.state_dir.join("mysite"));
    }

    #[test]
    fn absolute_docroot_is_used_as_is() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"example.org\"\ndocroot = \"/srv/gemini\"",
            &no_env(),
        )
        .expect("valid");
        assert_eq!(cfg.hosts[0].docroot, PathBuf::from("/srv/gemini"));
    }

    #[test]
    fn redirects_parse_and_compile() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"example.org\"\n\
             [[host.redirect]]\npattern = \"^/old$\"\ntarget = \"/new\"\npermanent = true",
            &no_env(),
        )
        .expect("valid");
        assert_eq!(cfg.hosts[0].redirects.len(), 1);
    }

    #[test]
    fn bad_redirect_pattern_is_rejected() {
        let err = Config::from_toml_str(
            "[[host]]\nname = \"example.org\"\n\
             [[host.redirect]]\npattern = \"[unclosed\"\ntarget = \"/new\"",
            &no_env(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("redirect"));
    }

    #[test]
    fn cert_zones_parse() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"example.org\"\n\
             [[host.cert_zone]]\npath_prefix = \"/private/\"\nfingerprints = [\"aabb\"]",
            &no_env(),
        )
        .expect("valid");
        assert_eq!(cfg.hosts[0].cert_zones.len(), 1);
        assert_eq!(cfg.hosts[0].cert_zones[0].path_prefix, "/private/");
    }

    #[test]
    fn bad_hostnames_are_rejected() {
        for bad in [
            "",
            "ex ample.org",
            "host:1965",
            "müller.example",
            "-x.example",
            "a..b",
        ] {
            let toml = format!("[[host]]\nname = {bad:?}");
            assert!(
                Config::from_toml_str(&toml, &no_env()).is_err(),
                "hostname {bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn env_overrides_beat_the_file() {
        let env = EnvOverrides {
            state_dir: Some(PathBuf::from("/app/data")),
            listen: Some("127.0.0.1:11965".into()),
            advertised_port: Some("1965".into()),
            hostname: Some("capsule.example".into()),
            http_listen: None,
            ..no_env()
        };
        let cfg = Config::from_toml_str(
            "[server]\nstate_dir = \"/elsewhere\"\nlisten = [\"0.0.0.0:1965\"]\n\
             [[host]]\nname = \"file.example\"",
            &env,
        )
        .expect("valid");
        assert_eq!(cfg.state_dir, PathBuf::from("/app/data"));
        assert_eq!(cfg.listen, vec!["127.0.0.1:11965".parse().unwrap()]);
        assert_eq!(cfg.advertised_port, 1965);
        assert!(cfg.serves_host("capsule.example"));
        assert!(!cfg.serves_host("file.example"));
    }

    #[test]
    fn empty_listen_via_env_disables_gemini_without_error() {
        let mut env = no_env();
        env.listen = Some(String::new());
        let cfg = Config::from_toml_str("", &env).expect("empty listen must not be rejected");
        assert!(cfg.listen.is_empty());
    }

    #[test]
    fn whitespace_only_listen_via_env_also_disables_gemini() {
        let mut env = no_env();
        env.listen = Some("   ".to_string());
        let cfg = Config::from_toml_str("", &env).expect("valid");
        assert!(cfg.listen.is_empty());
    }

    #[test]
    fn empty_listen_array_in_the_file_disables_gemini() {
        let cfg = Config::from_toml_str("[server]\nlisten = []", &no_env()).expect("valid");
        assert!(cfg.listen.is_empty());
    }

    #[test]
    fn absent_listen_configuration_still_uses_the_zero_config_default() {
        // The critical distinction empty-means-disabled must not blur: no
        // listen setting anywhere (file or env) is the ADR 0008 default,
        // not "disabled".
        let cfg = Config::from_toml_str("", &no_env()).expect("valid");
        assert_eq!(
            cfg.listen.len(),
            2,
            "the default 0.0.0.0:1965 + [::]:1965 pair"
        );
    }

    #[test]
    fn advertised_port_defaults_to_1965_when_gemini_is_disabled() {
        let mut env = no_env();
        env.listen = Some(String::new());
        let cfg = Config::from_toml_str("", &env).expect("valid");
        assert!(cfg.listen.is_empty());
        assert_eq!(
            cfg.advertised_port, 1965,
            "must not panic and must stay sane for a later re-enable"
        );
    }

    #[test]
    fn advertised_port_defaults_to_first_listen_port() {
        let cfg = Config::from_toml_str("[server]\nlisten = [\"127.0.0.1:11965\"]", &no_env())
            .expect("valid");
        assert_eq!(cfg.advertised_port, 11965);
    }

    #[test]
    fn advertised_host_is_absent_by_default() {
        let cfg = Config::from_toml_str("", &no_env()).expect("valid");
        assert_eq!(cfg.advertised_host, None);
    }

    #[test]
    fn advertised_host_is_validated_and_lowercased_like_any_hostname() {
        let cfg = Config::from_toml_str("[server]\nadvertised_host = \"EXAMPLE.ORG\"", &no_env())
            .expect("valid");
        assert_eq!(cfg.advertised_host.as_deref(), Some("example.org"));
    }

    #[test]
    fn advertised_host_accepts_an_onion_v3_shaped_hostname() {
        // Not a real onion address (those are derived from a key), but the
        // same shape: a 56-character base32 label plus the .onion TLD.
        // validate_hostname has no onion-specific rule — this proves the
        // existing structural checks (ASCII, label length, alphanumeric)
        // already accept it, so Tor deployment needs no special-casing.
        let onion = "a".repeat(56) + ".onion";
        let cfg =
            Config::from_toml_str(&format!("[server]\nadvertised_host = {onion:?}"), &no_env())
                .expect("valid");
        assert_eq!(cfg.advertised_host.as_deref(), Some(onion.as_str()));
    }

    #[test]
    fn advertised_host_rejects_the_same_bad_input_a_hostname_would() {
        let err =
            Config::from_toml_str("[server]\nadvertised_host = \"not a hostname!\"", &no_env())
                .unwrap_err();
        assert!(err.to_string().contains("hostname"), "{err}");
    }

    #[test]
    fn bad_listen_address_is_rejected_with_guidance() {
        let err = Config::from_toml_str("[server]\nlisten = [\"1965\"]", &no_env()).unwrap_err();
        assert!(err.to_string().contains("socket address"));
    }

    #[test]
    fn zero_connection_cap_is_rejected() {
        let err = Config::from_toml_str("[server]\nmax_connections = 0", &no_env()).unwrap_err();
        assert!(err.to_string().contains("at least 1"));
    }

    #[test]
    fn http_listen_defaults_to_off() {
        let cfg = Config::from_toml_str("", &no_env()).expect("valid");
        assert_eq!(cfg.http_listen, None);
    }

    #[test]
    fn http_listen_from_file() {
        let cfg = Config::from_toml_str("[server]\nhttp_listen = \"0.0.0.0:8000\"", &no_env())
            .expect("valid");
        assert_eq!(cfg.http_listen, Some("0.0.0.0:8000".parse().unwrap()));
    }

    #[test]
    fn http_listen_env_beats_file() {
        let mut env = no_env();
        env.http_listen = Some("127.0.0.1:9000".into());
        let cfg =
            Config::from_toml_str("[server]\nhttp_listen = \"0.0.0.0:8000\"", &env).expect("valid");
        assert_eq!(cfg.http_listen, Some("127.0.0.1:9000".parse().unwrap()));
    }

    #[test]
    fn theme_defaults_to_the_bundled_default() {
        let cfg = Config::from_toml_str("", &no_env()).expect("valid");
        assert_eq!(cfg.theme.name, crate::render::theme::DEFAULT_THEME_NAME);
    }

    #[test]
    fn theme_can_be_chosen_by_name() {
        let cfg =
            Config::from_toml_str("[server]\ntheme = \"tokyo-night\"", &no_env()).expect("valid");
        assert_eq!(cfg.theme.name, "tokyo-night");
    }

    #[test]
    fn unknown_theme_is_a_startup_error_listing_the_real_ones() {
        // A typo'd theme must say so rather than quietly serving the
        // default — and the message should tell the operator what they
        // could have written instead.
        let err = Config::from_toml_str("[server]\ntheme = \"drakula\"", &no_env()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("drakula"), "{msg}");
        assert!(
            msg.contains("daybreak"),
            "message should list known themes: {msg}"
        );
    }

    #[test]
    fn bad_http_listen_is_rejected() {
        let err = Config::from_toml_str("[server]\nhttp_listen = \"not-an-address\"", &no_env())
            .unwrap_err();
        assert!(err.to_string().contains("socket address"));
    }
}
