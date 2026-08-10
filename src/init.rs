//! `usv init` (`docs/BUILD-PLAN.md` C5): produce a working `usv.toml`
//! from an empty directory, interactively (a ratatui wizard, in
//! `main.rs`) or non-interactively (`--defaults`).
//!
//! Split the same way every other C5 command is: this module owns
//! validation and file generation — pure, fully testable without a
//! terminal — and `main.rs` owns the interactive event loop that
//! collects an [`InitAnswers`] before calling in here. `--defaults`
//! skips the event loop entirely and calls straight in with
//! [`InitAnswers::defaults`], so the two paths can never validate or
//! render differently.
//!
//! Every field is validated with the **exact rule `Config::resolve`
//! itself applies** ([`crate::config::validate_hostname`],
//! [`crate::config::is_plausible_lang`], [`crate::render::theme::find`])
//! — a second, hand-rolled copy of any of these checks is exactly how a
//! wizard ends up accepting something the real config loader then
//! refuses, which is the one failure mode a first-run wizard must never
//! have.

use std::path::Path;

use crate::config::{is_plausible_lang, validate_hostname};
use crate::render::theme;

/// Everything the wizard collects, whichever way it was collected.
#[derive(Debug, Clone)]
pub struct InitAnswers {
    /// The capsule's hostname, already validated.
    pub hostname: String,
    /// BCP 47 language tag, already validated.
    pub lang: String,
    /// A bundled theme's name, already validated.
    pub theme: String,
    /// The HTTP surface's listen address, if enabled. `None` means
    /// Gemini-only — a deliberate choice this codebase treats as
    /// first-class (ADR 0008), not a wizard shortcut.
    pub http_listen: Option<String>,
}

impl InitAnswers {
    /// `usv init --defaults`: a working, zero-interaction config. Every
    /// value here matches what an *absent* `usv.toml` would already
    /// resolve to (`Config::resolve`'s own defaults) — `--defaults`
    /// writes down the defaults explicitly rather than picking different
    /// ones, so the file it produces is a legible starting point to edit,
    /// not a second set of magic numbers to learn.
    pub fn defaults() -> InitAnswers {
        InitAnswers {
            hostname: "localhost".to_string(),
            lang: "en".to_string(),
            theme: theme::DEFAULT_THEME_NAME.to_string(),
            http_listen: None,
        }
    }
}

/// Why building or writing a config from [`InitAnswers`] failed.
#[derive(Debug)]
pub enum InitError {
    /// Not a usable hostname; carries [`crate::config::ConfigError`]'s
    /// own message so the wizard and the real loader explain a bad
    /// hostname identically.
    InvalidHostname(String),
    /// Not a plausible BCP 47 tag.
    InvalidLang(String),
    /// Not one of the bundled theme names.
    UnknownTheme(String),
    /// Present but not a parseable socket address.
    InvalidHttpListen(String),
    /// A file already exists at the destination — refused rather than
    /// overwritten, the same never-silently-overwrite rule this codebase
    /// applies everywhere a write could destroy an operator's existing
    /// material (identity minting, the content skeleton, `usv export`'s
    /// destination check).
    ConfigAlreadyExists(std::path::PathBuf),
    /// A filesystem operation failed.
    Io(std::io::Error),
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::InvalidHostname(e) => write!(f, "{e}"),
            InitError::InvalidLang(v) => write!(
                f,
                "{v:?} is not a BCP 47 language tag (e.g. \"en\", \"fr\", \"pt-BR\")"
            ),
            InitError::UnknownTheme(v) => {
                let known: Vec<&str> = theme::THEMES.iter().map(|t| t.name).collect();
                write!(
                    f,
                    "{v:?} is not a bundled theme (known: {})",
                    known.join(", ")
                )
            }
            InitError::InvalidHttpListen(v) => write!(
                f,
                "{v:?} is not a socket address (expected e.g. \"0.0.0.0:8080\")"
            ),
            InitError::ConfigAlreadyExists(p) => write!(
                f,
                "{} already exists; usv init never overwrites an existing config \
                 (edit it directly, or move it aside first)",
                p.display()
            ),
            InitError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for InitError {}

/// Validate raw wizard input into [`InitAnswers`]. Every rule here is the
/// same one `Config::resolve` applies to the file this produces — see the
/// module docs on why that must be true rather than merely similar.
pub fn validate(
    hostname: &str,
    lang: &str,
    theme_name: &str,
    http_listen: Option<&str>,
) -> Result<InitAnswers, InitError> {
    let hostname =
        validate_hostname(hostname).map_err(|e| InitError::InvalidHostname(e.to_string()))?;
    if !is_plausible_lang(lang) {
        return Err(InitError::InvalidLang(lang.to_string()));
    }
    let theme = theme::find(theme_name)
        .ok_or_else(|| InitError::UnknownTheme(theme_name.to_string()))?
        .name
        .to_string();
    let http_listen = match http_listen {
        None => None,
        Some(addr) => {
            addr.parse::<std::net::SocketAddr>()
                .map_err(|_| InitError::InvalidHttpListen(addr.to_string()))?;
            Some(addr.to_string())
        }
    };
    Ok(InitAnswers {
        hostname,
        lang: lang.to_string(),
        theme,
        http_listen,
    })
}

/// Render `answers` as `usv.toml` text. A plain, hand-written-looking
/// file — no generator preamble, no fields the operator didn't choose —
/// since this is meant to be a normal starting point for further manual
/// editing, not a machine-owned artifact.
pub fn render_toml(answers: &InitAnswers) -> String {
    let mut out = String::from("[server]\n");
    out.push_str(&format!("lang = {:?}\n", answers.lang));
    out.push_str(&format!("theme = {:?}\n", answers.theme));
    if let Some(addr) = &answers.http_listen {
        out.push_str(&format!("http_listen = {addr:?}\n"));
    }
    out.push('\n');
    out.push_str("[[host]]\n");
    out.push_str(&format!("name = {:?}\n", answers.hostname));
    out
}

/// Write `answers` to `path` as `usv.toml`, refusing to overwrite an
/// existing file. Parent directories are created as needed (matching
/// `IdentityStore::open`'s own `create_dir_all` on first run) — the whole
/// point of an init wizard is that the directory may not exist yet.
pub async fn write_config(path: &Path, answers: &InitAnswers) -> Result<(), InitError> {
    if tokio::fs::metadata(path).await.is_ok() {
        return Err(InitError::ConfigAlreadyExists(path.to_path_buf()));
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(InitError::Io)?;
    }
    tokio::fs::write(path, render_toml(answers))
        .await
        .map_err(InitError::Io)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("usv-init-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn defaults_validate_cleanly() {
        let d = InitAnswers::defaults();
        let validated = validate(&d.hostname, &d.lang, &d.theme, d.http_listen.as_deref()).unwrap();
        assert_eq!(validated.hostname, "localhost");
        assert_eq!(validated.lang, "en");
    }

    #[test]
    fn defaults_produce_a_config_that_actually_loads() {
        // The strongest proof this wizard can never drift from the real
        // loader: run its own output straight through Config::resolve.
        let toml = render_toml(&InitAnswers::defaults());
        let cfg =
            crate::config::Config::from_toml_str(&toml, &crate::config::EnvOverrides::default());
        assert!(
            cfg.is_ok(),
            "defaults must produce a config that loads: {cfg:?}"
        );
        let cfg = cfg.unwrap();
        assert_eq!(cfg.hosts.len(), 1);
        assert_eq!(cfg.hosts[0].name, "localhost");
    }

    #[test]
    fn validate_rejects_a_bad_hostname() {
        let err = validate("not a hostname!", "en", "daybreak", None).unwrap_err();
        assert!(matches!(err, InitError::InvalidHostname(_)));
    }

    #[test]
    fn validate_rejects_a_bad_lang() {
        let err = validate("example.org", "", "daybreak", None).unwrap_err();
        assert!(matches!(err, InitError::InvalidLang(_)));
        let err2 = validate("example.org", "café", "daybreak", None).unwrap_err();
        assert!(matches!(err2, InitError::InvalidLang(_)));
    }

    #[test]
    fn validate_rejects_an_unknown_theme() {
        let err = validate("example.org", "en", "not-a-theme", None).unwrap_err();
        assert!(matches!(err, InitError::UnknownTheme(_)));
    }

    #[test]
    fn validate_rejects_a_malformed_http_listen() {
        let err = validate("example.org", "en", "daybreak", Some("not-an-address")).unwrap_err();
        assert!(matches!(err, InitError::InvalidHttpListen(_)));
    }

    #[test]
    fn validate_accepts_a_well_formed_http_listen() {
        let a = validate("example.org", "en", "daybreak", Some("0.0.0.0:8080")).unwrap();
        assert_eq!(a.http_listen.as_deref(), Some("0.0.0.0:8080"));
    }

    #[test]
    fn render_toml_omits_http_listen_when_absent() {
        let answers = InitAnswers::defaults();
        let toml = render_toml(&answers);
        assert!(!toml.contains("http_listen"));
    }

    #[test]
    fn render_toml_includes_http_listen_when_present() {
        let mut answers = InitAnswers::defaults();
        answers.http_listen = Some("0.0.0.0:8080".to_string());
        let toml = render_toml(&answers);
        assert!(toml.contains("http_listen = \"0.0.0.0:8080\""));
    }

    #[tokio::test]
    async fn write_config_creates_a_loadable_file() {
        let dir = tmpdir("write-basic");
        let path = dir.join("usv.toml");
        write_config(&path, &InitAnswers::defaults()).await.unwrap();
        let text = tokio::fs::read_to_string(&path).await.unwrap();
        let cfg =
            crate::config::Config::from_toml_str(&text, &crate::config::EnvOverrides::default());
        assert!(cfg.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn write_config_creates_parent_directories() {
        let dir = tmpdir("write-nested");
        let path = dir.join("a/b/c/usv.toml");
        write_config(&path, &InitAnswers::defaults()).await.unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn write_config_never_overwrites_an_existing_file() {
        let dir = tmpdir("write-existing");
        let path = dir.join("usv.toml");
        std::fs::write(&path, "# operator's own config, do not touch\n").unwrap();

        let err = write_config(&path, &InitAnswers::defaults())
            .await
            .unwrap_err();
        assert!(matches!(err, InitError::ConfigAlreadyExists(_)));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# operator's own config, do not touch\n",
            "the existing file must be untouched"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_custom_hostname_and_theme_round_trip_through_config_loading() {
        let answers = validate(
            "my-capsule.example",
            "fr",
            "tokyo-night",
            Some("127.0.0.1:9000"),
        )
        .unwrap();
        let toml = render_toml(&answers);
        let cfg =
            crate::config::Config::from_toml_str(&toml, &crate::config::EnvOverrides::default())
                .unwrap();
        assert_eq!(cfg.hosts[0].name, "my-capsule.example");
        assert_eq!(cfg.lang, "fr");
        assert_eq!(cfg.theme.name, "tokyo-night");
        assert!(cfg.http_listen.is_some());
    }
}
