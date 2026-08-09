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
    /// A `[titan]` section is present but Titan arrives in phase C4
    /// (ADR 0006).
    TitanReserved,
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
            ConfigError::TitanReserved => write!(
                f,
                "config has a [titan] section, but Titan uploads are not implemented yet \
                 (they arrive in phase C4; ADR 0006). Remove the section for now — \
                 it is reserved so your future settings can never be silently ignored"
            ),
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
    /// Reserved for phase C4 (ADR 0006); presence is a startup error.
    titan: Option<toml::Table>,
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHost {
    name: String,
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
}

impl EnvOverrides {
    /// Capture the `USV_*` overrides from the process environment.
    pub fn from_process_env() -> Self {
        EnvOverrides {
            state_dir: std::env::var_os(env_keys::STATE_DIR).map(PathBuf::from),
            listen: std::env::var(env_keys::LISTEN).ok(),
            advertised_port: std::env::var(env_keys::ADVERTISED_PORT).ok(),
            hostname: std::env::var(env_keys::HOSTNAME).ok(),
        }
    }
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
        if raw.titan.is_some() {
            return Err(ConfigError::TitanReserved);
        }
        if raw.responses.is_some() {
            return Err(ConfigError::ResponsesReserved);
        }
        let server = raw.server.unwrap_or_default();

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

        let host_names: Vec<String> = match &env.hostname {
            Some(name) => vec![name.clone()],
            None if raw.hosts.is_empty() => vec!["localhost".into()],
            None => raw.hosts.into_iter().map(|h| h.name).collect(),
        };
        let mut hosts = Vec::with_capacity(host_names.len());
        for name in host_names {
            let name = validate_hostname(&name)?;
            hosts.push(HostConfig { name });
        }

        let max_connections = server.max_connections.unwrap_or(512);
        if max_connections == 0 {
            return Err(ConfigError::Rejected(
                "server.max_connections must be at least 1".into(),
            ));
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
}

/// Validate and normalize (lowercase) a configured hostname. Clients send
/// punycoded, ASCII hostnames on the wire (the spec is URI-based, not
/// IRI-based), so that is the only form config accepts; rejecting Unicode
/// here catches the "typed my IDN in directly" mistake loudly at startup.
fn validate_hostname(name: &str) -> Result<String, ConfigError> {
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
    fn titan_section_errors_helpfully() {
        let err = Config::from_toml_str("[titan]\n", &no_env()).unwrap_err();
        assert!(matches!(err, ConfigError::TitanReserved));
        assert!(err.to_string().contains("C4"), "message names the phase");
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
}
