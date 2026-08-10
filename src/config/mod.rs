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
    /// without double-bind conflicts.
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
    /// Hostnames this server answers for (authority check layer 3; SNI cert
    /// selection). Requests naming any other host get status 53.
    pub hosts: Vec<HostConfig>,
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
    /// A value parsed but fails validation (bad hostname, empty listen list,
    /// zero connection cap…).
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
    max_connections: Option<usize>,
    request_timeout_secs: Option<u64>,
    response_timeout_secs: Option<u64>,
    http_listen: Option<String>,
    theme: Option<String>,
    lang: Option<String>,
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
        let titan_default_max_upload = raw.titan.unwrap_or_default().max_upload_bytes;

        let state_dir = env
            .state_dir
            .clone()
            .or(server.state_dir)
            .unwrap_or_else(|| default_state_dir(&EnvOverrides::default()));

        let listen_strings: Vec<String> = match &env.listen {
            Some(s) => s.split(',').map(|a| a.trim().to_string()).collect(),
            None => server
                .listen
                .unwrap_or_else(|| vec!["0.0.0.0:1965".into(), "[::]:1965".into()]),
        };
        if listen_strings.is_empty() {
            return Err(ConfigError::Rejected(
                "server.listen is empty; at least one listen address is required".into(),
            ));
        }
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
            None => server.advertised_port.unwrap_or_else(|| listen[0].port()),
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

        Ok(Config {
            state_dir,
            listen,
            advertised_port,
            tls_min,
            hosts,
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
    fn advertised_port_defaults_to_first_listen_port() {
        let cfg = Config::from_toml_str("[server]\nlisten = [\"127.0.0.1:11965\"]", &no_env())
            .expect("valid");
        assert_eq!(cfg.advertised_port, 11965);
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
