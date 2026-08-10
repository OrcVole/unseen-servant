//! `usv` — the Unseen Servant binary.
//!
//! Phase C1: zero-arg `usv` starts a working capsule (ADR 0008) — config
//! loaded per the ADR 0007 search order, identity minted on first run
//! (ADR 0003), Gemini served on the configured listeners.
//!
//! C5 (`docs/BUILD-PLAN.md`): `status`, `fingerprint`, `check`, `zones`,
//! `stats`, `render [--force]`, `identity add/rotate/revoke`, `export`,
//! `init [--defaults]` are implemented — thin argument-parsing wrappers
//! in this file around business logic in [`unseen_servant::cli`]/
//! [`unseen_servant::init`]/[`unseen_servant::handler::admin`], which is
//! where every format/lint/export/validation function is actually
//! tested. Only the interactive `init` event loop itself lives here —
//! ratatui rendering has no meaningful unit test, so everything it could
//! get wrong (what a bad hostname looks like, what the file ends up
//! containing) is pushed into `init::validate`/`init::write_config`
//! instead, where it can be. Tor/I2P affordances (`server.advertised_host`,
//! onion-hostname cert slots, no-SNI tolerance) are implemented in
//! `config`/`identity`/`tls`, not here — see `INTEGRATIONS.md`.
//!
//! Signal discipline (ADR 0002): SIGHUP reloads config + certificates
//! without dropping listeners (an invalid file keeps the old config);
//! SIGTERM/SIGINT drain gracefully — Cloudron and systemd both stop with
//! SIGTERM.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use tokio::sync::{Semaphore, watch};

use unseen_servant::cli;
use unseen_servant::config::{Config, EnvOverrides};
use unseen_servant::handler;
use unseen_servant::http;
use unseen_servant::identity::IdentityStore;
use unseen_servant::init::{self, InitAnswers};
use unseen_servant::plaintext;
use unseen_servant::render::theme;
use unseen_servant::render::{pipeline, watcher};
use unseen_servant::runtime_state::{RenderSnapshot, RuntimeState};
use unseen_servant::server::{self, Shared};
use unseen_servant::tls;

const HELP: &str = concat!(
    "usv ",
    env!("CARGO_PKG_VERSION"),
    " — Unseen Servant, a security-first Gemini server (pre-release)\n",
    "\n",
    "USAGE:\n",
    "  usv [--config <path>]           start the server (zero-config default works)\n",
    "  usv status     [--config <p>]   config, fingerprints, roster, zones, published\n",
    "  usv fingerprint [--config <p>]  this capsule's server certificate fingerprint(s)\n",
    "  usv check      [--config <p>]   validate config + lint the content tree\n",
    "  usv zones      [--config <p>]   list certificate and Titan zones\n",
    "  usv stats      [--config <p>]   what's currently published (read-only)\n",
    "  usv render     [--config <p>] [--force]   render the content tree now\n",
    "  usv identity add    <label> <fingerprint> [--capability <c>]... [--enrolled <date>]\n",
    "  usv identity rotate <label> <new-fingerprint> --until <date>\n",
    "  usv identity revoke <label> (--capability <c>... | --all)\n",
    "  usv export     [--config <p>] <destination>   copy the rendered tree out\n",
    "  usv init       [--config <p>] [--defaults]   write a working usv.toml\n",
    "  usv --version | --help\n",
    "\n",
    "CONFIG:\n",
    "  One TOML file: --config, else $USV_CONFIG, else ${state_dir}/usv.toml,\n",
    "  else built-in defaults (localhost capsule, port 1965, auto-minted cert).\n",
    "  USV_STATE_DIR / USV_LISTEN / USV_ADVERTISED_PORT / USV_HOSTNAME /\n",
    "  USV_HTTP_LISTEN override platform-injected facts (ADR 0007). The HTTP\n",
    "  surface (rendered HTML tree) is off unless USV_HTTP_LISTEN or\n",
    "  server.http_listen names an address.\n",
    "\n",
    "SIGNALS:\n",
    "  SIGHUP  reload config and certificates without dropping listeners\n",
    "  SIGTERM graceful drain and exit\n",
    "\n",
    "`status`/`fingerprint`/`zones` open (and, on a fresh capsule, mint) the\n",
    "identity store — the same first-run behaviour starting the server has.\n",
    "`stats` is read-only and never renders; `render` always performs a real\n",
    "render (the same atomic staging-swap the server itself uses).\n",
    "\n",
    "`usv identity add/rotate/revoke` print a ready-to-paste [[identity]]\n",
    "block; they never write to usv.toml (director-confirmed 2026-08-10).\n",
    "Look the identity up first with `--config` pointed at the real file, so\n",
    "rotate/revoke can find the existing entry.\n",
    "\n",
    "`usv export` copies the already-rendered state_dir/html tree to\n",
    "<destination> verbatim (refuses a non-empty destination) — that folder\n",
    "is already a self-contained static site (ADR 0004), so this is\n",
    "\"copy it out for OnionShare or any other static host\", not a second\n",
    "render. Never renders anything new: run `usv render` first if needed.\n",
    "\n",
    "`usv init` writes a new usv.toml (refuses to overwrite an existing\n",
    "one) at --config's path, or the default search location. Interactive\n",
    "by default (a small terminal wizard); `--defaults` skips it and writes\n",
    "the same defaults an absent config already resolves to, written down\n",
    "explicitly as a starting point to edit.\n",
    "\n",
    "Tor/I2P onion capsules: set server.advertised_host and add a [[host]]\n",
    "for the onion address (see INTEGRATIONS.md for the verified recipe and\n",
    "a real gotcha around advertised_port). Nothing is announced or exposed\n",
    "publicly before the v1.0 gates pass (docs/ROADMAP.md).\n",
);

/// A subcommand not yet implemented — recognised and named rather than
/// falling through to the generic "unknown argument" error, so the
/// director gets "not yet, see BUILD-PLAN C5" instead of "typo?".
const RESERVED_SUBCOMMANDS: &[&str] = &[];

/// `usv identity <action>`'s own parsed action — kept separate from the
/// flat single-token subcommands, since it takes positional arguments and
/// repeatable flags the top-level parser has no shape for.
enum IdentityAction {
    Add {
        label: String,
        fingerprint: String,
        capabilities: Vec<String>,
        enrolled: Option<String>,
    },
    Rotate {
        label: String,
        new_fingerprint: String,
        until: String,
    },
    Revoke {
        label: String,
        capabilities: Vec<String>,
        all: bool,
    },
}

enum Command {
    Serve,
    Status,
    Fingerprint,
    Check,
    Zones,
    Stats,
    Render { force: bool },
    Identity(IdentityAction),
    Export { destination: PathBuf },
    Init { defaults: bool },
}

struct Args {
    config: Option<PathBuf>,
    command: Command,
}

enum Parsed {
    Run(Args),
    Exit(ExitCode),
}

fn parse_args() -> Parsed {
    let mut config = None;
    let mut command = None;
    let mut force = false;
    let mut defaults = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("usv {}", env!("CARGO_PKG_VERSION"));
                return Parsed::Exit(ExitCode::SUCCESS);
            }
            "--help" | "-h" => {
                print!("{HELP}");
                return Parsed::Exit(ExitCode::SUCCESS);
            }
            "--config" => match args.next() {
                Some(path) => config = Some(PathBuf::from(path)),
                None => {
                    eprintln!("usv: --config requires a path argument");
                    return Parsed::Exit(ExitCode::from(2));
                }
            },
            "--force" => force = true,
            "identity" if command.is_none() => {
                return match parse_identity_action(args) {
                    Ok(action) => Parsed::Run(Args {
                        config,
                        command: Command::Identity(action),
                    }),
                    Err(code) => Parsed::Exit(code),
                };
            }
            "export" if command.is_none() => {
                let destination = match args.next() {
                    Some(d) => PathBuf::from(d),
                    None => {
                        eprintln!("usv: export needs a destination directory (see --help)");
                        return Parsed::Exit(ExitCode::from(2));
                    }
                };
                if let Some(extra) = args.next() {
                    eprintln!("usv: export: unexpected argument '{extra}' (see --help)");
                    return Parsed::Exit(ExitCode::from(2));
                }
                return Parsed::Run(Args {
                    config,
                    command: Command::Export { destination },
                });
            }
            "--defaults" => defaults = true,
            "status" | "fingerprint" | "check" | "zones" | "stats" | "render" | "init"
                if command.is_none() =>
            {
                command = Some(arg);
            }
            reserved if RESERVED_SUBCOMMANDS.contains(&reserved) && command.is_none() => {
                eprintln!(
                    "usv: '{reserved}' is not implemented yet (docs/BUILD-PLAN.md C5). \
                     Nothing was run."
                );
                return Parsed::Exit(ExitCode::from(2));
            }
            other => {
                eprintln!("usv: unknown argument '{other}' (see --help)");
                return Parsed::Exit(ExitCode::from(2));
            }
        }
    }
    let command = match command.as_deref() {
        None => Command::Serve,
        Some("status") => Command::Status,
        Some("fingerprint") => Command::Fingerprint,
        Some("check") => Command::Check,
        Some("zones") => Command::Zones,
        Some("stats") => Command::Stats,
        Some("render") => Command::Render { force },
        Some("init") => Command::Init { defaults },
        Some(_) => unreachable!("only recognised subcommand strings are stored"),
    };
    if force && !matches!(command, Command::Render { .. }) {
        eprintln!("usv: --force only applies to 'render'");
        return Parsed::Exit(ExitCode::from(2));
    }
    if defaults && !matches!(command, Command::Init { .. }) {
        eprintln!("usv: --defaults only applies to 'init'");
        return Parsed::Exit(ExitCode::from(2));
    }
    Parsed::Run(Args { config, command })
}

/// Parse `identity <action> ...` — everything after the `identity` token.
/// `--config` is not accepted here (see the module docs/`--help`: it must
/// come before `identity`, same as every other subcommand); seeing one
/// here is a clear, named error rather than the flag being silently
/// swallowed as a positional label or fingerprint.
fn parse_identity_action(
    mut args: impl Iterator<Item = String>,
) -> Result<IdentityAction, ExitCode> {
    let action_word = args.next().ok_or_else(|| {
        eprintln!("usv: 'identity' needs an action: add, rotate, or revoke (see --help)");
        ExitCode::from(2)
    })?;
    match action_word.as_str() {
        "add" => parse_identity_add(args),
        "rotate" => parse_identity_rotate(args),
        "revoke" => parse_identity_revoke(args),
        other => {
            eprintln!("usv: unknown identity action '{other}' (expected add, rotate, or revoke)");
            Err(ExitCode::from(2))
        }
    }
}

fn misplaced_config_error() -> ExitCode {
    eprintln!("usv: --config must come before 'identity' (see --help)");
    ExitCode::from(2)
}

fn missing_positional(what: &str) -> ExitCode {
    eprintln!("usv: identity: missing {what} (see --help)");
    ExitCode::from(2)
}

fn parse_identity_add(mut args: impl Iterator<Item = String>) -> Result<IdentityAction, ExitCode> {
    let label = args.next().ok_or_else(|| missing_positional("<label>"))?;
    let fingerprint = args
        .next()
        .ok_or_else(|| missing_positional("<fingerprint>"))?;
    let mut capabilities = Vec::new();
    let mut enrolled = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--capability" => capabilities.push(
                args.next()
                    .ok_or_else(|| missing_positional("a value for --capability"))?,
            ),
            "--enrolled" => {
                enrolled = Some(
                    args.next()
                        .ok_or_else(|| missing_positional("a value for --enrolled"))?,
                );
            }
            "--config" => return Err(misplaced_config_error()),
            other => {
                eprintln!("usv: identity add: unknown argument '{other}' (see --help)");
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(IdentityAction::Add {
        label,
        fingerprint,
        capabilities,
        enrolled,
    })
}

fn parse_identity_rotate(
    mut args: impl Iterator<Item = String>,
) -> Result<IdentityAction, ExitCode> {
    let label = args.next().ok_or_else(|| missing_positional("<label>"))?;
    let new_fingerprint = args
        .next()
        .ok_or_else(|| missing_positional("<new-fingerprint>"))?;
    let mut until = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--until" => {
                until = Some(
                    args.next()
                        .ok_or_else(|| missing_positional("a value for --until"))?,
                );
            }
            "--config" => return Err(misplaced_config_error()),
            other => {
                eprintln!("usv: identity rotate: unknown argument '{other}' (see --help)");
                return Err(ExitCode::from(2));
            }
        }
    }
    let until = until.ok_or_else(|| missing_positional("--until <date>"))?;
    Ok(IdentityAction::Rotate {
        label,
        new_fingerprint,
        until,
    })
}

fn parse_identity_revoke(
    mut args: impl Iterator<Item = String>,
) -> Result<IdentityAction, ExitCode> {
    let label = args.next().ok_or_else(|| missing_positional("<label>"))?;
    let mut capabilities = Vec::new();
    let mut all = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--capability" => capabilities.push(
                args.next()
                    .ok_or_else(|| missing_positional("a value for --capability"))?,
            ),
            "--all" => all = true,
            "--config" => return Err(misplaced_config_error()),
            other => {
                eprintln!("usv: identity revoke: unknown argument '{other}' (see --help)");
                return Err(ExitCode::from(2));
            }
        }
    }
    if all && !capabilities.is_empty() {
        eprintln!("usv: identity revoke: --all and --capability are mutually exclusive");
        return Err(ExitCode::from(2));
    }
    if !all && capabilities.is_empty() {
        eprintln!(
            "usv: identity revoke: name at least one --capability to remove, or pass --all \
             to remove the whole identity"
        );
        return Err(ExitCode::from(2));
    }
    Ok(IdentityAction::Revoke {
        label,
        capabilities,
        all,
    })
}

/// The content directory rendering reads from: the primary host's
/// docroot, or `state_dir/content` for a config naming no hosts yet.
/// Shared between `serve()` and every CLI subcommand that touches
/// content, so "where is the content tree" is answered in exactly one
/// place (ADR 0004's one-content-tree model, applied to this binary's
/// own code, not just the capsule's data).
fn content_dir(config: &Config) -> PathBuf {
    config
        .hosts
        .first()
        .map(|h| h.docroot.clone())
        .unwrap_or_else(|| config.state_dir.join("content"))
}

/// The render context a full render needs, built from config alone —
/// shared by `serve()`'s initial render, its watcher, and `usv render`/
/// `usv check`, so the three can never construct it differently.
fn render_context(config: &Config) -> pipeline::RenderContext {
    let primary_host = config
        .hosts
        .first()
        .map(|h| h.name.clone())
        .unwrap_or_default();
    // `server.advertised_host` overrides only what generated absolute URLs
    // (feed self-link, sitemap.xml, /llms.txt, robots.txt) name — not
    // routing or SNI cert selection, which still key off `config.hosts`
    // (see the field's own docs). Unset: unchanged behavior, first host.
    let advertised_host = config.advertised_host.clone().unwrap_or(primary_host);
    let web_base_url = config
        .http_listen
        .map(|_| format!("https://{advertised_host}"))
        .unwrap_or_default();
    // The cleartext trees are built only when at least one cleartext
    // protocol is enabled, and they carry the gate that keeps gated
    // pages out of every one of them (ADR 0012 §6).
    let any_cleartext = config.gopher.is_some() || config.spartan.is_some() || config.nex.is_some();
    let cleartext = any_cleartext.then(|| {
        let gate = config
            .hosts
            .first()
            .map(unseen_servant::render::cleartext::Gate::for_host)
            .unwrap_or_default();
        pipeline::CleartextRender {
            gate,
            gopher: config
                .gopher
                .as_ref()
                .map(|g| unseen_servant::render::gopher::Context {
                    host: advertised_host.clone(),
                    port: g.advertised_port,
                }),
        }
    });
    pipeline::RenderContext {
        theme_css: config.theme.css.to_string(),
        web_base_url,
        capsule_title: advertised_host,
        lang: config.lang.clone(),
        cleartext,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod render_context_tests {
    use super::*;
    use unseen_servant::config::EnvOverrides;

    fn cfg(toml: &str) -> Config {
        Config::from_toml_str(toml, &EnvOverrides::default()).unwrap()
    }

    #[test]
    fn without_an_override_the_first_host_is_advertised() {
        let ctx = render_context(&cfg("[server]\nhttp_listen = \"127.0.0.1:8080\"\n\
             [[host]]\nname = \"real.example\"\n"));
        assert_eq!(ctx.web_base_url, "https://real.example");
        assert_eq!(ctx.capsule_title, "real.example");
    }

    #[test]
    fn advertised_host_overrides_the_render_time_base_url_and_title() {
        // The Tor case: the capsule answers Gemini for its real hostname
        // (routing/SNI still key off [[host]]) but wants its rendered
        // feeds/sitemap/llms.txt to name the onion mirror instead.
        let onion = "a".repeat(56) + ".onion";
        let ctx = render_context(&cfg(&format!(
            "[server]\nhttp_listen = \"127.0.0.1:8080\"\nadvertised_host = {onion:?}\n\
             [[host]]\nname = \"real.example\"\n"
        )));
        assert_eq!(ctx.web_base_url, format!("https://{onion}"));
        assert_eq!(ctx.capsule_title, onion);
    }

    #[test]
    fn advertised_host_is_moot_without_an_http_surface() {
        // web_base_url must stay empty (which disables the Atom feed) —
        // advertised_host names *how* to advertise the web surface, not
        // *whether* one exists.
        let onion = "a".repeat(56) + ".onion";
        let ctx = render_context(&cfg(&format!(
            "[server]\nadvertised_host = {onion:?}\n[[host]]\nname = \"real.example\"\n"
        )));
        assert_eq!(ctx.web_base_url, "");
    }
}

/// Load config the same way every subcommand does: env overrides, the
/// `--config` search order (ADR 0007), a plain error message on failure.
///
/// Takes the path directly rather than `&Args`, so callers can pull
/// `config` out of an owned `Args` (as `run_command` does, matching on
/// `args.command` by value) without fighting the borrow checker over a
/// struct one of whose other fields was just moved.
fn load_config(config_path: Option<&Path>) -> Result<Config, ExitCode> {
    let env = EnvOverrides::from_process_env();
    Config::load(config_path, &env).map_err(|e| {
        eprintln!("usv: {e}");
        ExitCode::FAILURE
    })
}

/// Open (and, on a fresh capsule, mint) the identity store for every
/// configured host — the same call `build_state` makes for the server
/// itself, reused so `status`/`fingerprint`/`zones` report the identity
/// the server would actually run with, not a second, divergent notion of
/// it.
fn open_identities(config: &Config) -> Result<IdentityStore, ExitCode> {
    let hostnames: Vec<String> = config.hosts.iter().map(|h| h.name.clone()).collect();
    IdentityStore::open(&config.certs_dir(), &hostnames).map_err(|e| {
        eprintln!("usv: {e}");
        ExitCode::FAILURE
    })
}

async fn run_command(args: Args) -> ExitCode {
    let Args { config, command } = args;
    let config_path = config.as_deref();
    match command {
        Command::Serve => unreachable!("Serve is dispatched by main() before reaching here"),
        Command::Status => cmd_status(config_path).await,
        Command::Fingerprint => cmd_fingerprint(config_path).await,
        Command::Check => cmd_check(config_path).await,
        Command::Zones => cmd_zones(config_path),
        Command::Stats => cmd_stats(config_path).await,
        Command::Render { force } => cmd_render(config_path, force).await,
        Command::Identity(action) => cmd_identity(config_path, action),
        Command::Export { destination } => cmd_export(config_path, &destination).await,
        Command::Init { defaults } => cmd_init(config_path, defaults).await,
    }
}

/// Where `usv init` writes, absent an explicit destination flag of its
/// own: the exact ADR 0007 search order `Config::load` itself uses for
/// "no `--config`" (`$USV_CONFIG`, else `${state_dir}/usv.toml`) — a
/// wizard that wrote somewhere the real loader wouldn't look would be
/// worse than no wizard at all.
fn init_target_path(config_path: Option<&Path>) -> PathBuf {
    if let Some(p) = config_path {
        return p.to_path_buf();
    }
    if let Some(p) = std::env::var_os(unseen_servant::config::env_keys::CONFIG) {
        return PathBuf::from(p);
    }
    unseen_servant::config::default_state_dir(&EnvOverrides::from_process_env()).join("usv.toml")
}

/// `usv init [--defaults]`.
async fn cmd_init(config_path: Option<&Path>, defaults: bool) -> ExitCode {
    let path = init_target_path(config_path);
    let answers = if defaults {
        InitAnswers::defaults()
    } else {
        match run_init_wizard(&path) {
            Ok(Some(a)) => a,
            Ok(None) => {
                println!("cancelled — nothing was written");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("usv: init wizard failed: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    match init::write_config(&path, &answers).await {
        Ok(()) => {
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("usv: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `usv export <destination>`: copy the rendered HTML tree out to a plain
/// folder — see `cli::export_html_tree`'s docs for why this is
/// deliberately almost the whole implementation.
async fn cmd_export(config_path: Option<&Path>, destination: &Path) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match cli::export_html_tree(&config.state_dir, destination).await {
        Ok(count) => {
            println!("exported {count} file(s) to {}", destination.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("usv: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `usv identity add/rotate/revoke`: load config for context (so
/// rotate/revoke can find the identity they're named against, and add
/// can refuse a colliding label), then print the snippet or the error —
/// never touch the config file (director-confirmed 2026-08-10: printing
/// is the whole design, not a stopgap for a future `--write`).
fn cmd_identity(config_path: Option<&Path>, action: IdentityAction) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let result = match action {
        IdentityAction::Add {
            label,
            fingerprint,
            capabilities,
            enrolled,
        } => cli::identity_add_snippet(
            &config.roster,
            &label,
            &fingerprint,
            &capabilities,
            enrolled.as_deref(),
        ),
        IdentityAction::Rotate {
            label,
            new_fingerprint,
            until,
        } => cli::identity_rotate_snippet(&config.roster, &label, &new_fingerprint, &until),
        IdentityAction::Revoke {
            label,
            capabilities,
            all,
        } => cli::identity_revoke_snippet(&config.roster, &label, &capabilities, all),
    };
    match result {
        Ok(snippet) => {
            print!("{snippet}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("usv: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn cmd_status(config_path: Option<&Path>) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let store = match open_identities(&config) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let published = cli::inspect_published(&config.state_dir).await;
    print!("{}", cli::format_status(&config, &store, &published));
    ExitCode::SUCCESS
}

async fn cmd_fingerprint(config_path: Option<&Path>) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let store = match open_identities(&config) {
        Ok(s) => s,
        Err(code) => return code,
    };
    print!("{}", cli::format_fingerprints(&store));
    ExitCode::SUCCESS
}

fn cmd_zones(config_path: Option<&Path>) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    print!("{}", cli::format_zones(&config));
    ExitCode::SUCCESS
}

async fn cmd_check(config_path: Option<&Path>) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let content = content_dir(&config);
    let lint = match cli::lint_content(&content).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "usv: could not read content directory {}: {e}",
                content.display()
            );
            return ExitCode::FAILURE;
        }
    };
    print!("{}", cli::format_check_report(&config, &lint));
    ExitCode::SUCCESS
}

async fn cmd_stats(config_path: Option<&Path>) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let published = cli::inspect_published(&config.state_dir).await;
    print!("{}", cli::format_published_stats(&published));
    ExitCode::SUCCESS
}

async fn cmd_render(config_path: Option<&Path>, _force: bool) -> ExitCode {
    // `render_tree` is already always a full rebuild (design brief §5.4:
    // "full-tree rebuild every time, not incremental"), so `--force` has
    // no distinct behaviour to select today. It is accepted and parsed
    // now — rather than added later as a breaking CLI change — because a
    // future incremental-render mode would need exactly this flag to
    // force a full one, and the surface should be stable before that
    // exists, not after.
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let content = content_dir(&config);
    let ctx = render_context(&config);
    match pipeline::render_tree(&content, &config.state_dir, &ctx).await {
        Ok(stats) => {
            println!(
                "rendered {} page(s) (feed entries: {}, mapped pages: {}, robots mirrored: {})",
                stats.pages_rendered, stats.feed_entries, stats.mapped_pages, stats.robots_mirrored
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("usv: render failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Parsed::Run(args) => args,
        Parsed::Exit(code) => return code,
    };

    // Logs to stderr only (ADR 0002: stdout/stderr, no log files; the
    // platform rotates). RUST_LOG filters; default level info. ANSI color
    // only when stderr is a real terminal: platform log collectors and the
    // regress suite read these lines as text.
    let color = {
        use std::io::IsTerminal;
        std::io::stderr().is_terminal()
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(color)
        .init();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "tokio runtime construction failed");
            return ExitCode::FAILURE;
        }
    };
    if matches!(args.command, Command::Serve) {
        match runtime.block_on(serve(args)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(()) => ExitCode::FAILURE,
        }
    } else {
        runtime.block_on(run_command(args))
    }
}

/// Build the full shared state from scratch: config → identity → TLS.
/// Used at startup and again on every SIGHUP. `bound_port` substitutes for
/// a configured `advertised_port` of 0 ("derive from the bound listener")
/// once the listeners exist.
fn build_state(args: &Args, bound_port: Option<u16>) -> Result<Shared, String> {
    let env = EnvOverrides::from_process_env();
    let mut config = Config::load(args.config.as_deref(), &env).map_err(|e| e.to_string())?;
    if config.advertised_port == 0
        && let Some(port) = bound_port
    {
        config.advertised_port = port;
    }
    let hostnames: Vec<String> = config.hosts.iter().map(|h| h.name.clone()).collect();
    let identities =
        IdentityStore::open(&config.certs_dir(), &hostnames).map_err(|e| e.to_string())?;
    let tls = tls::server_config(config.tls_min, Arc::new(identities))
        .map_err(|e| format!("TLS configuration failed: {e}"))?;
    Ok(Shared {
        config: Arc::new(config),
        tls,
    })
}

async fn serve(args: Args) -> Result<(), ()> {
    let mut state = match build_state(&args, None) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("{e}");
            return Err(());
        }
    };

    // Bind every listener before declaring victory; a bind failure at
    // startup is fatal (there is nothing to gracefully degrade to at C1).
    let mut listeners = Vec::new();
    let mut first_bound: Option<u16> = None;
    for addr in &state.config.listen {
        match server::bind(*addr) {
            Ok(l) => {
                match l.local_addr() {
                    Ok(bound) => {
                        first_bound.get_or_insert(bound.port());
                        tracing::info!(%bound, "gemini listener bound");
                    }
                    Err(_) => tracing::info!(%addr, "gemini listener bound"),
                }
                listeners.push(l);
            }
            Err(e) => {
                tracing::error!(%addr, error = %e, "cannot bind listener");
                return Err(());
            }
        }
    }
    // Ephemeral-port case: advertise what the OS actually granted.
    let bound_port = first_bound;
    if state.config.advertised_port == 0
        && let Some(port) = bound_port
    {
        let mut config = (*state.config).clone();
        config.advertised_port = port;
        state.config = Arc::new(config);
    }
    server::warn_if_nonstandard(&state.config);

    // C3: render once before declaring "serving" (so the first HTTP
    // request sees fresh content, not whatever happened to survive from
    // a previous run), then start the watcher for every edit after that.
    // The primary host's docroot is content_dir for rendering purposes —
    // ADR 0004's one-content-tree model, gemtext served natively from it
    // on Gemini, HTML rendered from it here. Per-host HTML rendering
    // (multi-host capsules) is a known v1 limitation, not yet wired.
    let content_dir = state
        .config
        .hosts
        .first()
        .map(|h| h.docroot.clone())
        .unwrap_or_else(|| state.config.state_dir.join("content"));
    // A capsule with no content at all gets the first-run skeleton — but
    // only ever when the content directory holds no gemtext whatsoever,
    // so an operator's own work is never overwritten on a later start.
    if let Err(e) = pipeline::seed_skeleton_if_empty(&content_dir, pipeline::DEFAULT_SKELETON).await
    {
        tracing::warn!(error = %e, "could not write the first-run skeleton");
    }
    // Built by the same function `usv render`/`usv check` use, so the live
    // server's initial render, its watcher, and the CLI tools can never
    // construct this differently (e.g. server.advertised_host silently
    // applying to one but not the other).
    let render_ctx = render_context(&state.config);
    // Created once, before any listener accepts a connection, and never
    // touched by SIGHUP reload (runtime_state's whole reason to exist —
    // see its module docs): an operator reloading config mid-incident
    // must see the activity log keep growing, not reset to empty.
    let runtime = Arc::new(RuntimeState::new(time::OffsetDateTime::now_utc()));

    match pipeline::render_tree(&content_dir, &state.config.state_dir, &render_ctx).await {
        Ok(stats) => {
            tracing::info!(
                pages = stats.pages_rendered,
                robots_mirrored = stats.robots_mirrored,
                feed_entries = stats.feed_entries,
                mapped_pages = stats.mapped_pages,
                "initial render complete"
            );
            runtime.record_render(RenderSnapshot {
                at: time::OffsetDateTime::now_utc(),
                pages_rendered: stats.pages_rendered,
                feed_entries: stats.feed_entries,
                mapped_pages: stats.mapped_pages,
                robots_mirrored: stats.robots_mirrored,
            });
        }
        Err(e) => tracing::warn!(error = %e, "initial render failed; HTTP surface may be stale"),
    }
    let watcher_content_dir = content_dir.clone();
    let watcher_state_dir = state.config.state_dir.clone();
    // A `watch` channel, not a plain clone: SIGHUP can change
    // server.advertised_host, the primary hostname, http_listen, the
    // theme, or the language, and the watcher's *next* edit-triggered
    // render must see that — not the value frozen at this moment, which
    // would otherwise persist until the next full restart (found as a
    // real bug tracing through this exact path — see watcher::watch's
    // docs on why it takes a receiver, not an owned RenderContext).
    // `render_ctx_tx` is threaded into `signal_loop`, the only place a
    // reload ever produces a new one.
    let (render_ctx_tx, render_ctx_rx) = watch::channel(render_ctx.clone());
    let watcher_runtime = runtime.clone();
    let watcher_task = tokio::spawn(async move {
        let result = watcher::watch(
            watcher_content_dir,
            watcher_state_dir,
            render_ctx_rx,
            watcher::DEFAULT_DEBOUNCE,
            |result| match result {
                Ok(stats) => {
                    tracing::info!(pages = stats.pages_rendered, "content re-rendered");
                    watcher_runtime.record_render(RenderSnapshot {
                        at: time::OffsetDateTime::now_utc(),
                        pages_rendered: stats.pages_rendered,
                        feed_entries: stats.feed_entries,
                        mapped_pages: stats.mapped_pages,
                        robots_mirrored: stats.robots_mirrored,
                    });
                }
                Err(e) => tracing::warn!(error = %e, "re-render failed"),
            },
        )
        .await;
        if let Err(e) = result {
            tracing::error!(error = %e, "content watcher stopped");
        }
    });

    let permits = Arc::new(Semaphore::new(state.config.max_connections));
    let max_permits = state.config.max_connections;
    let http_listen = state.config.http_listen;
    let (state_tx, state_rx) = watch::channel(state);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let mut accept_tasks = Vec::new();
    for listener in listeners {
        accept_tasks.push(tokio::spawn(server::accept_loop(
            listener,
            state_rx.clone(),
            permits.clone(),
            shutdown_rx.clone(),
            runtime.clone(),
        )));
    }

    // HTTP surface (C3; ADR 0008): optional standalone, always set by the
    // Cloudron profile. Starts unconditionally once enabled — independent
    // of whether the Gemini listeners above bound successfully, matching
    // the cloudron-fit.md hard constraint that the health check must
    // never depend on the Gemini port's state.
    let mut http_task = None;
    if let Some(addr) = http_listen {
        match http::bind(addr) {
            Ok(listener) => {
                let html_dir = state_rx.borrow().config.state_dir.join("html");
                let (http_state_tx, http_state_rx) = watch::channel(http::Shared { html_dir });
                std::mem::forget(http_state_tx); // html_dir is stable for process lifetime
                tracing::info!(%addr, "http listener bound");
                http_task = Some(tokio::spawn(http::accept_loop(
                    listener,
                    http_state_rx,
                    shutdown_rx.clone(),
                )));
            }
            Err(e) => {
                tracing::error!(%addr, error = %e, "cannot bind http listener");
                return Err(());
            }
        }
    }

    // Gopher (v1.1; ADR 0012): cleartext, and therefore only ever
    // present because the operator asked for it.
    let mut gopher_task = None;
    if let Some(gcfg) = state_rx.borrow().config.gopher.clone() {
        match server::bind(gcfg.listen) {
            Ok(listener) => {
                let bound = listener.local_addr().unwrap_or(gcfg.listen);
                let root = state_rx.borrow().config.state_dir.join("gopher");
                let gctx = unseen_servant::render::gopher::Context {
                    host: state_rx
                        .borrow()
                        .config
                        .advertised_host
                        .clone()
                        .or_else(|| {
                            state_rx
                                .borrow()
                                .config
                                .hosts
                                .first()
                                .map(|h| h.name.clone())
                        })
                        .unwrap_or_default(),
                    port: gcfg.advertised_port,
                };

                // Say plainly what was just switched on (ADR 0012 §2),
                // and what will not be in it (§6) — an operator who
                // gated a path must not have to deduce why it is missing.
                plaintext::log_trust_disclaimer("gopher", bound);
                if let Some(host) = state_rx.borrow().config.hosts.first() {
                    let gate = unseen_servant::render::cleartext::Gate::for_host(host);
                    unseen_servant::render::cleartext::announce("gopher", &gate);
                }

                let handler_root = root.clone();
                let handler_ctx = gctx.clone();
                let handler_addrs = unseen_servant::render::colophon::Addresses::from_config(
                    &state_rx.borrow().config.clone(),
                );
                let service = plaintext::Service {
                    name: "gopher",
                    max_request_bytes: unseen_servant::protocol::gopher::MAX_SELECTOR_BYTES,
                    request_timeout_secs: state_rx.borrow().config.request_timeout_secs,
                    response_timeout_secs: state_rx.borrow().config.response_timeout_secs,
                    handler: std::sync::Arc::new(move |line, _cfg| {
                        let root = handler_root.clone();
                        let ctx = handler_ctx.clone();
                        let addrs = handler_addrs.clone();
                        Box::pin(async move {
                            match unseen_servant::protocol::gopher::parse_selector_line(&line) {
                                Ok((req, _)) => {
                                    handler::gopher::serve(&req.selector, &root, &ctx, &addrs).await
                                }
                                // Gopher has no status codes: a refusal is
                                // an ordinary one-line type-3 menu.
                                Err(_) => {
                                    unseen_servant::protocol::gopher::error_menu("bad request")
                                        .into_bytes()
                                }
                            }
                        })
                    }),
                };
                let (gopher_cfg_tx, gopher_cfg_rx) =
                    watch::channel(state_rx.borrow().config.clone());
                std::mem::forget(gopher_cfg_tx);
                gopher_task = Some(tokio::spawn(plaintext::accept_loop(
                    listener,
                    service,
                    gopher_cfg_rx,
                    std::sync::Arc::new(tokio::sync::Semaphore::new(
                        state_rx.borrow().config.max_connections,
                    )),
                    shutdown_rx.clone(),
                )));
            }
            Err(e) => {
                tracing::error!(addr = %gcfg.listen, error = %e, "cannot bind gopher listener");
                return Err(());
            }
        }
    }

    // Finger (v1.1; ADR 0012): a person's status, not the content tree.
    let mut finger_task = None;
    if let Some(fcfg) = state_rx.borrow().config.finger.clone() {
        match server::bind(fcfg.listen) {
            Ok(listener) => {
                let bound = listener.local_addr().unwrap_or(fcfg.listen);
                plaintext::log_trust_disclaimer("finger", bound);
                let cfg_now = state_rx.borrow().config.clone();
                let state_dir = cfg_now.state_dir.clone();
                let addresses = handler::finger::Addresses::from_config(&cfg_now);
                let service = plaintext::Service {
                    name: "finger",
                    max_request_bytes: unseen_servant::protocol::finger::MAX_REQUEST_BYTES,
                    request_timeout_secs: cfg_now.request_timeout_secs,
                    response_timeout_secs: cfg_now.response_timeout_secs,
                    handler: std::sync::Arc::new(move |line, _cfg| {
                        let state_dir = state_dir.clone();
                        let addresses = addresses.clone();
                        Box::pin(async move {
                            match unseen_servant::protocol::finger::parse(&line) {
                                Ok(_) => handler::finger::respond(&state_dir, &addresses).await,
                                Err(
                                    unseen_servant::protocol::finger::RequestError::ForwardingRefused,
                                ) => b"finger forwarding is not supported here\r\n".to_vec(),
                                Err(_) => b"bad request\r\n".to_vec(),
                            }
                        })
                    }),
                };
                let (finger_cfg_tx, finger_cfg_rx) = watch::channel(cfg_now.clone());
                std::mem::forget(finger_cfg_tx);
                finger_task = Some(tokio::spawn(plaintext::accept_loop(
                    listener,
                    service,
                    finger_cfg_rx,
                    std::sync::Arc::new(tokio::sync::Semaphore::new(cfg_now.max_connections)),
                    shutdown_rx.clone(),
                )));
            }
            Err(e) => {
                tracing::error!(addr = %fcfg.listen, error = %e, "cannot bind finger listener");
                return Err(());
            }
        }
    }

    // Spartan and Nex (v1.1; ADR 0012). Both serve the cleartext tree
    // the gopher target already builds — no third and fourth render
    // pass, and the wall holds for free because gated pages were never
    // written into it.
    // Spartan and Nex serve GEMTEXT, so they read the cleartext tree —
    // not the gopher tree, which holds menus. Pointing them at the
    // wrong one is exactly the bug live testing caught.
    let cleartext_root = state_rx.borrow().config.state_dir.join("cleartext");

    let mut spartan_task = None;
    if let Some(scfg) = state_rx.borrow().config.spartan.clone() {
        match server::bind(scfg.listen) {
            Ok(listener) => {
                plaintext::log_trust_disclaimer(
                    "spartan",
                    listener.local_addr().unwrap_or(scfg.listen),
                );
                let root = cleartext_root.clone();
                let cfg_now = state_rx.borrow().config.clone();
                let service = plaintext::Service {
                    name: "spartan",
                    max_request_bytes: unseen_servant::protocol::spartan::MAX_REQUEST_BYTES,
                    request_timeout_secs: cfg_now.request_timeout_secs,
                    response_timeout_secs: cfg_now.response_timeout_secs,
                    handler: {
                        let addrs =
                            unseen_servant::render::colophon::Addresses::from_config(&cfg_now);
                        std::sync::Arc::new(move |line, _cfg| {
                            let root = root.clone();
                            let addrs = addrs.clone();
                            Box::pin(async move {
                                match unseen_servant::protocol::spartan::parse(&line) {
                                    Ok((req, _)) => {
                                        handler::spartan::serve(&req, &root, &addrs).await
                                    }
                                    Err(_) => unseen_servant::protocol::spartan::client_error(
                                        "bad request",
                                    )
                                    .into_bytes(),
                                }
                            })
                        })
                    },
                };
                let (tx, rx) = watch::channel(cfg_now.clone());
                std::mem::forget(tx);
                spartan_task = Some(tokio::spawn(plaintext::accept_loop(
                    listener,
                    service,
                    rx,
                    std::sync::Arc::new(tokio::sync::Semaphore::new(cfg_now.max_connections)),
                    shutdown_rx.clone(),
                )));
            }
            Err(e) => {
                tracing::error!(addr = %scfg.listen, error = %e, "cannot bind spartan listener");
                return Err(());
            }
        }
    }

    let mut nex_task = None;
    if let Some(ncfg) = state_rx.borrow().config.nex.clone() {
        match server::bind(ncfg.listen) {
            Ok(listener) => {
                plaintext::log_trust_disclaimer(
                    "nex",
                    listener.local_addr().unwrap_or(ncfg.listen),
                );
                let root = cleartext_root.clone();
                let cfg_now = state_rx.borrow().config.clone();
                let service = plaintext::Service {
                    name: "nex",
                    max_request_bytes: handler::nex::MAX_REQUEST_BYTES,
                    request_timeout_secs: cfg_now.request_timeout_secs,
                    response_timeout_secs: cfg_now.response_timeout_secs,
                    handler: {
                        let addrs =
                            unseen_servant::render::colophon::Addresses::from_config(&cfg_now);
                        std::sync::Arc::new(move |line, _cfg| {
                            let root = root.clone();
                            let addrs = addrs.clone();
                            Box::pin(async move { handler::nex::serve(&line, &root, &addrs).await })
                        })
                    },
                };
                let (tx, rx) = watch::channel(cfg_now.clone());
                std::mem::forget(tx);
                nex_task = Some(tokio::spawn(plaintext::accept_loop(
                    listener,
                    service,
                    rx,
                    std::sync::Arc::new(tokio::sync::Semaphore::new(cfg_now.max_connections)),
                    shutdown_rx.clone(),
                )));
            }
            Err(e) => {
                tracing::error!(addr = %ncfg.listen, error = %e, "cannot bind nex listener");
                return Err(());
            }
        }
    }

    tracing::info!(
        hosts = ?state_rx.borrow().config.hosts.iter().map(|h| h.name.clone()).collect::<Vec<_>>(),
        "usv {} serving",
        env!("CARGO_PKG_VERSION")
    );

    signal_loop(&args, bound_port, &state_tx, &shutdown_tx, &render_ctx_tx).await;

    // Stop accepting, then drain: wait for every permit to come home, up to
    // a grace period, so in-flight responses finish with close_notify.
    for task in &accept_tasks {
        task.abort();
    }
    if let Some(task) = &http_task {
        task.abort();
    }
    if let Some(task) = &gopher_task {
        task.abort();
    }
    if let Some(task) = &finger_task {
        task.abort();
    }
    if let Some(task) = &spartan_task {
        task.abort();
    }
    if let Some(task) = &nex_task {
        task.abort();
    }
    watcher_task.abort();
    let grace = std::time::Duration::from_secs(15);
    let drained = tokio::time::timeout(grace, async {
        let _ = permits.acquire_many(max_permits as u32).await;
    })
    .await
    .is_ok();
    if drained {
        tracing::info!("drained cleanly; exiting");
    } else {
        tracing::warn!(
            grace_secs = grace.as_secs(),
            "grace period expired with connections still open; exiting anyway"
        );
    }
    Ok(())
}

/// Wait for signals: SIGHUP → rebuild state and swap; SIGTERM/SIGINT →
/// return (the caller drains). Reload failures keep the old state — a
/// running server is never taken down by a bad config edit (ADR 0007).
async fn signal_loop(
    args: &Args,
    bound_port: Option<u16>,
    state_tx: &watch::Sender<Shared>,
    shutdown_tx: &watch::Sender<bool>,
    render_ctx_tx: &watch::Sender<pipeline::RenderContext>,
) {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sighup = match signal(SignalKind::hangup()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "cannot install SIGHUP handler");
            return;
        }
    };
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "cannot install SIGTERM handler");
            return;
        }
    };
    loop {
        tokio::select! {
            _ = sighup.recv() => {
                tracing::info!("SIGHUP: reloading configuration and certificates");
                match build_state(args, bound_port) {
                    Ok(new_state) => {
                        let old_listen = state_tx.borrow().config.listen.clone();
                        if new_state.config.listen != old_listen {
                            tracing::warn!(
                                "listen addresses changed in config; a restart is required \
                                 for that change (reload never drops live listeners)"
                            );
                        }
                        // Every edit-triggered render after this point must
                        // see the new config (advertised_host, hostname,
                        // http_listen, theme, lang) — not the value the
                        // watcher was started with (see watcher::watch's
                        // docs on why it reads a receiver, not an owned
                        // context).
                        let _ = render_ctx_tx.send(render_context(&new_state.config));
                        let _ = state_tx.send(new_state);
                        tracing::info!("reload complete");
                    }
                    Err(e) => {
                        tracing::error!(
                            "reload REJECTED, keeping the running configuration: {e}"
                        );
                    }
                }
            }
            _ = sigterm.recv() => {
                tracing::info!("SIGTERM: graceful shutdown");
                let _ = shutdown_tx.send(true);
                return;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("SIGINT: graceful shutdown");
                let _ = shutdown_tx.send(true);
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------
// `usv init`'s interactive wizard. Rendering has no meaningful unit test
// (see the module docs) — every decision that *can* be tested lives in
// `unseen_servant::init` instead, and is. What's here is the minimum
// event-loop shell around it.
// ---------------------------------------------------------------------

/// One field of the wizard, in the order the operator fills them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Hostname,
    Lang,
    Theme,
    HttpEnabled,
    HttpAddress,
    Confirm,
}

/// The step after `current`, given whether the HTTP surface is enabled —
/// `HttpAddress` is skipped entirely when it's off, since there is
/// nothing to type. A pure function so the skip logic is checkable
/// without a terminal.
fn wizard_next(current: WizardStep, http_enabled: bool) -> WizardStep {
    match current {
        WizardStep::Hostname => WizardStep::Lang,
        WizardStep::Lang => WizardStep::Theme,
        WizardStep::Theme => WizardStep::HttpEnabled,
        WizardStep::HttpEnabled if http_enabled => WizardStep::HttpAddress,
        WizardStep::HttpEnabled => WizardStep::Confirm,
        WizardStep::HttpAddress => WizardStep::Confirm,
        WizardStep::Confirm => WizardStep::Confirm,
    }
}

/// The step before `current` — the mirror of [`wizard_next`], used by
/// Backspace-to-go-back. `Hostname` has no predecessor; it is its own.
fn wizard_prev(current: WizardStep, http_enabled: bool) -> WizardStep {
    match current {
        WizardStep::Hostname => WizardStep::Hostname,
        WizardStep::Lang => WizardStep::Hostname,
        WizardStep::Theme => WizardStep::Lang,
        WizardStep::HttpEnabled => WizardStep::Theme,
        WizardStep::HttpAddress => WizardStep::HttpEnabled,
        WizardStep::Confirm if http_enabled => WizardStep::HttpAddress,
        WizardStep::Confirm => WizardStep::HttpEnabled,
    }
}

/// Everything the wizard's UI needs to hold between frames.
struct WizardState {
    step: WizardStep,
    hostname: String,
    lang: String,
    theme_idx: usize,
    http_enabled: bool,
    http_address: String,
    error: Option<String>,
}

impl WizardState {
    fn new() -> WizardState {
        let defaults = InitAnswers::defaults();
        WizardState {
            step: WizardStep::Hostname,
            hostname: defaults.hostname,
            lang: defaults.lang,
            theme_idx: theme::THEMES
                .iter()
                .position(|t| t.name == defaults.theme)
                .unwrap_or(0),
            http_enabled: false,
            http_address: "0.0.0.0:8080".to_string(),
            error: None,
        }
    }

    /// Validate the collected answers via `init::validate` — the single
    /// source of truth this wizard defers to rather than re-implementing
    /// any check.
    fn validate(&self) -> Result<InitAnswers, init::InitError> {
        let http = self.http_enabled.then_some(self.http_address.as_str());
        init::validate(
            &self.hostname,
            &self.lang,
            theme::THEMES[self.theme_idx].name,
            http,
        )
    }
}

/// Run the interactive wizard. `Ok(None)` means the operator cancelled
/// (Esc/Ctrl-C); `path` is shown in the confirm screen so the operator
/// knows exactly what they're about to write, and is never touched here
/// — writing is `cmd_init`'s job, after this returns.
fn run_init_wizard(path: &Path) -> std::io::Result<Option<InitAnswers>> {
    let mut terminal = ratatui::init();
    let result = run_wizard_loop(&mut terminal, path);
    ratatui::restore();
    result
}

fn run_wizard_loop(
    terminal: &mut ratatui::DefaultTerminal,
    path: &Path,
) -> std::io::Result<Option<InitAnswers>> {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers};
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

    let mut state = WizardState::new();
    loop {
        terminal.draw(|frame| {
            let area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(6),
                    Constraint::Length(2),
                ])
                .split(frame.area());

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "usv init",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        format!("writing to {}", path.display()),
                        Style::default().fg(Color::DarkGray),
                    )),
                ]),
                area[0],
            );

            let body: Vec<Line> = match state.step {
                WizardStep::Hostname => vec![
                    Line::from("Hostname"),
                    Line::from(Span::styled(
                        format!("> {}_", state.hostname),
                        Style::default().fg(Color::Yellow),
                    )),
                ],
                WizardStep::Lang => vec![
                    Line::from("Language (BCP 47, e.g. en, fr, pt-BR)"),
                    Line::from(Span::styled(
                        format!("> {}_", state.lang),
                        Style::default().fg(Color::Yellow),
                    )),
                ],
                WizardStep::Theme => {
                    let mut lines = vec![Line::from("Theme (↑↓ to choose, Enter to accept)")];
                    for (i, t) in theme::THEMES.iter().enumerate() {
                        let marker = if i == state.theme_idx { "> " } else { "  " };
                        let style = if i == state.theme_idx {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        lines.push(Line::from(Span::styled(
                            format!("{marker}{} — {}", t.name, t.description),
                            style,
                        )));
                    }
                    lines
                }
                WizardStep::HttpEnabled => vec![
                    Line::from("Enable the HTTP (web mirror) surface? (y/n)"),
                    Line::from(Span::styled(
                        if state.http_enabled { "> yes" } else { "> no" },
                        Style::default().fg(Color::Yellow),
                    )),
                ],
                WizardStep::HttpAddress => vec![
                    Line::from("HTTP listen address (e.g. 0.0.0.0:8080)"),
                    Line::from(Span::styled(
                        format!("> {}_", state.http_address),
                        Style::default().fg(Color::Yellow),
                    )),
                ],
                WizardStep::Confirm => {
                    let mut lines = vec![Line::from("Review — Enter to write, Backspace to edit")];
                    match state.validate() {
                        Ok(answers) => {
                            for line in init::render_toml(&answers).lines() {
                                lines.push(Line::from(line.to_string()));
                            }
                        }
                        Err(e) => {
                            lines.push(Line::from(Span::styled(
                                format!("Cannot write yet: {e}"),
                                Style::default().fg(Color::Red),
                            )));
                        }
                    }
                    lines
                }
            };
            let mut all_lines = body;
            if let Some(err) = &state.error {
                all_lines.push(Line::from(""));
                all_lines.push(Line::from(Span::styled(
                    err.clone(),
                    Style::default().fg(Color::Red),
                )));
            }
            frame.render_widget(
                List::new(all_lines.into_iter().map(ListItem::new).collect::<Vec<_>>())
                    .block(Block::default().borders(Borders::TOP)),
                area[1],
            );

            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "Enter: next · Backspace: back · Esc/Ctrl-C: cancel",
                    Style::default().fg(Color::DarkGray),
                ))),
                area[2],
            );
        })?;

        let Event::Key(key) = event::read()? else {
            continue;
        };
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_c || key.code == KeyCode::Esc {
            return Ok(None);
        }
        state.error = None;
        match state.step {
            WizardStep::Hostname | WizardStep::Lang | WizardStep::HttpAddress => {
                let buf = match state.step {
                    WizardStep::Hostname => &mut state.hostname,
                    WizardStep::Lang => &mut state.lang,
                    _ => &mut state.http_address,
                };
                match key.code {
                    KeyCode::Char(c) => buf.push(c),
                    KeyCode::Backspace if buf.is_empty() => {
                        state.step = wizard_prev(state.step, state.http_enabled);
                    }
                    KeyCode::Backspace => {
                        buf.pop();
                    }
                    KeyCode::Enter => {
                        state.step = wizard_next(state.step, state.http_enabled);
                    }
                    _ => {}
                }
            }
            WizardStep::Theme => match key.code {
                KeyCode::Up => {
                    state.theme_idx = state.theme_idx.saturating_sub(1);
                }
                KeyCode::Down => {
                    state.theme_idx = (state.theme_idx + 1).min(theme::THEMES.len() - 1);
                }
                KeyCode::Enter => state.step = wizard_next(state.step, state.http_enabled),
                KeyCode::Backspace => state.step = wizard_prev(state.step, state.http_enabled),
                _ => {}
            },
            WizardStep::HttpEnabled => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => state.http_enabled = true,
                KeyCode::Char('n') | KeyCode::Char('N') => state.http_enabled = false,
                KeyCode::Enter => state.step = wizard_next(state.step, state.http_enabled),
                KeyCode::Backspace => state.step = wizard_prev(state.step, state.http_enabled),
                _ => {}
            },
            WizardStep::Confirm => match key.code {
                KeyCode::Enter => match state.validate() {
                    Ok(answers) => return Ok(Some(answers)),
                    Err(e) => state.error = Some(e.to_string()),
                },
                KeyCode::Backspace => {
                    state.step = wizard_prev(state.step, state.http_enabled);
                }
                _ => {}
            },
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod wizard_tests {
    use super::*;

    #[test]
    fn http_address_is_skipped_when_the_surface_is_off() {
        assert_eq!(
            wizard_next(WizardStep::HttpEnabled, false),
            WizardStep::Confirm
        );
        assert_eq!(
            wizard_next(WizardStep::HttpEnabled, true),
            WizardStep::HttpAddress
        );
    }

    #[test]
    fn forward_and_back_are_mirror_images_off_http() {
        for step in [
            WizardStep::Hostname,
            WizardStep::Lang,
            WizardStep::Theme,
            WizardStep::HttpEnabled,
        ] {
            let forward = wizard_next(step, false);
            assert_eq!(
                wizard_prev(forward, false),
                step,
                "{step:?} -> {forward:?} -> back"
            );
        }
    }

    #[test]
    fn forward_and_back_are_mirror_images_with_http_on() {
        for step in [WizardStep::HttpEnabled, WizardStep::HttpAddress] {
            let forward = wizard_next(step, true);
            assert_eq!(
                wizard_prev(forward, true),
                step,
                "{step:?} -> {forward:?} -> back"
            );
        }
    }

    #[test]
    fn hostname_has_no_predecessor() {
        assert_eq!(
            wizard_prev(WizardStep::Hostname, false),
            WizardStep::Hostname
        );
        assert_eq!(
            wizard_prev(WizardStep::Hostname, true),
            WizardStep::Hostname
        );
    }

    #[test]
    fn confirm_is_the_terminal_step() {
        assert_eq!(wizard_next(WizardStep::Confirm, false), WizardStep::Confirm);
        assert_eq!(wizard_next(WizardStep::Confirm, true), WizardStep::Confirm);
    }

    #[test]
    fn wizard_state_defaults_match_init_answers_defaults() {
        let state = WizardState::new();
        let defaults = InitAnswers::defaults();
        assert_eq!(state.hostname, defaults.hostname);
        assert_eq!(state.lang, defaults.lang);
        assert_eq!(theme::THEMES[state.theme_idx].name, defaults.theme);
        assert!(!state.http_enabled);
    }

    #[test]
    fn wizard_state_validate_matches_init_validate() {
        let state = WizardState::new();
        let validated = state.validate().unwrap();
        let direct = init::validate(
            &state.hostname,
            &state.lang,
            theme::THEMES[state.theme_idx].name,
            None,
        )
        .unwrap();
        assert_eq!(validated.hostname, direct.hostname);
        assert_eq!(validated.theme, direct.theme);
    }
}
