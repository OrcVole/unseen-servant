//! `usv` — the Unseen Servant binary.
//!
//! Phase C1: zero-arg `usv` starts a working capsule (ADR 0008) — config
//! loaded per the ADR 0007 search order, identity minted on first run
//! (ADR 0003), Gemini served on the configured listeners. The M4 subcommands
//! (init, status, fingerprint, check, zones, render, stats, export) arrive
//! in C5; naming one today is a loud error, not a silent no-op.
//!
//! Signal discipline (ADR 0002): SIGHUP reloads config + certificates
//! without dropping listeners (an invalid file keeps the old config);
//! SIGTERM/SIGINT drain gracefully — Cloudron and systemd both stop with
//! SIGTERM.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use tokio::sync::{Semaphore, watch};

use unseen_servant::config::{Config, EnvOverrides};
use unseen_servant::identity::IdentityStore;
use unseen_servant::server::{self, Shared};
use unseen_servant::tls;

const HELP: &str = concat!(
    "usv ",
    env!("CARGO_PKG_VERSION"),
    " — Unseen Servant, a security-first Gemini server (pre-release)\n",
    "\n",
    "USAGE:\n",
    "  usv [--config <path>]     start the server (zero-config default works)\n",
    "  usv --version | --help\n",
    "\n",
    "CONFIG:\n",
    "  One TOML file: --config, else $USV_CONFIG, else ${state_dir}/usv.toml,\n",
    "  else built-in defaults (localhost capsule, port 1965, auto-minted cert).\n",
    "  USV_STATE_DIR / USV_LISTEN / USV_ADVERTISED_PORT / USV_HOSTNAME override\n",
    "  platform-injected facts (ADR 0007).\n",
    "\n",
    "SIGNALS:\n",
    "  SIGHUP  reload config and certificates without dropping listeners\n",
    "  SIGTERM graceful drain and exit\n",
    "\n",
    "Subcommands (init, status, fingerprint, check, zones, render, stats,\n",
    "export) arrive per docs/BUILD-PLAN.md phases C2–C5. Nothing is announced\n",
    "or exposed publicly before the v1.0 gates pass (docs/ROADMAP.md).\n",
);

struct Args {
    config: Option<PathBuf>,
}

enum Parsed {
    Run(Args),
    Exit(ExitCode),
}

fn parse_args() -> Parsed {
    let mut config = None;
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
            other => {
                eprintln!("usv: unknown argument '{other}' (see --help)");
                return Parsed::Exit(ExitCode::from(2));
            }
        }
    }
    Parsed::Run(Args { config })
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
    match runtime.block_on(serve(args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
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

    let permits = Arc::new(Semaphore::new(state.config.max_connections));
    let max_permits = state.config.max_connections;
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
