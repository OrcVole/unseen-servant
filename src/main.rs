//! `usv` — the Unseen Servant binary.
//!
//! Phase C1: zero-arg `usv` starts a working capsule (ADR 0008) — config
//! loaded per the ADR 0007 search order, identity minted on first run
//! (ADR 0003), Gemini served on the configured listeners.
//!
//! C5 (`docs/BUILD-PLAN.md`): `status`, `fingerprint`, `check`, `zones`,
//! `stats`, `render [--force]` are implemented — thin argument-parsing
//! wrappers in this file around business logic in [`unseen_servant::cli`],
//! which is where every format/lint function is actually tested. `init`
//! (the ratatui wizard) and `export` (OnionShare-ready folder) are still
//! reserved: naming either today is a loud, named error, never a silent
//! no-op or a fallthrough to "unknown argument".
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
use unseen_servant::http;
use unseen_servant::identity::IdentityStore;
use unseen_servant::render::{pipeline, watcher};
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
    "Subcommands (init, export) and Tor/I2P affordances arrive per\n",
    "docs/BUILD-PLAN.md C5. Nothing is announced or exposed publicly before\n",
    "the v1.0 gates pass (docs/ROADMAP.md).\n",
);

/// A subcommand not yet implemented — recognised and named rather than
/// falling through to the generic "unknown argument" error, so the
/// director gets "not yet, see BUILD-PLAN C5" instead of "typo?".
const RESERVED_SUBCOMMANDS: &[&str] = &["init", "export"];

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
            "status" | "fingerprint" | "check" | "zones" | "stats" | "render"
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
        Some(_) => unreachable!("only recognised subcommand strings are stored"),
    };
    if force && !matches!(command, Command::Render { .. }) {
        eprintln!("usv: --force only applies to 'render'");
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
    let web_base_url = config
        .http_listen
        .map(|_| format!("https://{primary_host}"))
        .unwrap_or_default();
    pipeline::RenderContext {
        theme_css: config.theme.css.to_string(),
        web_base_url,
        capsule_title: primary_host,
        lang: config.lang.clone(),
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
    // The primary host's name is the capsule identity for feeds. The
    // web base URL is only knowable if the HTTP surface is on — Atom
    // needs absolute links, so a Gemini-only deployment gets the gemsub
    // feed but no atom.xml (RenderContext handles the empty-base case).
    let primary_host = state
        .config
        .hosts
        .first()
        .map(|h| h.name.clone())
        .unwrap_or_default();
    let web_base_url = state
        .config
        .http_listen
        .map(|_| format!("https://{primary_host}"))
        .unwrap_or_default();
    let render_ctx = pipeline::RenderContext {
        theme_css: state.config.theme.css.to_string(),
        web_base_url,
        capsule_title: primary_host.clone(),
        lang: state.config.lang.clone(),
    };
    match pipeline::render_tree(&content_dir, &state.config.state_dir, &render_ctx).await {
        Ok(stats) => tracing::info!(
            pages = stats.pages_rendered,
            robots_mirrored = stats.robots_mirrored,
            feed_entries = stats.feed_entries,
            mapped_pages = stats.mapped_pages,
            "initial render complete"
        ),
        Err(e) => tracing::warn!(error = %e, "initial render failed; HTTP surface may be stale"),
    }
    let watcher_content_dir = content_dir.clone();
    let watcher_state_dir = state.config.state_dir.clone();
    let watcher_ctx = render_ctx.clone();
    let watcher_task = tokio::spawn(async move {
        let result = watcher::watch(
            watcher_content_dir,
            watcher_state_dir,
            watcher_ctx,
            watcher::DEFAULT_DEBOUNCE,
            |result| match result {
                Ok(stats) => tracing::info!(pages = stats.pages_rendered, "content re-rendered"),
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

    tracing::info!(
        hosts = ?state_rx.borrow().config.hosts.iter().map(|h| h.name.clone()).collect::<Vec<_>>(),
        "usv {} serving",
        env!("CARGO_PKG_VERSION")
    );

    signal_loop(&args, bound_port, &state_tx, &shutdown_tx).await;

    // Stop accepting, then drain: wait for every permit to come home, up to
    // a grace period, so in-flight responses finish with close_notify.
    for task in &accept_tasks {
        task.abort();
    }
    if let Some(task) = &http_task {
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
