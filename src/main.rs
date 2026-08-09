//! `usv` — the Unseen Servant binary.
//!
//! Phase C0: argument surface only. The server itself arrives in C1
//! (docs/BUILD-PLAN.md); the subcommands listed in the help text are the
//! committed surface from docs/ROADMAP.md M4, stubbed here so the CLI shape
//! is stable from the first release of anything.

use std::process::ExitCode;

const HELP: &str = concat!(
    "usv ",
    env!("CARGO_PKG_VERSION"),
    " — Unseen Servant, a security-first Gemini server (pre-release)\n",
    "\n",
    "USAGE:\n",
    "  usv [--version | --help]\n",
    "\n",
    "The server and its subcommands (init, status, fingerprint, check, zones,\n",
    "render, stats, export) arrive per docs/BUILD-PLAN.md phases C1–C5; until\n",
    "then this binary only identifies itself. Nothing is announced or exposed\n",
    "publicly before the v1.0 gates pass (docs/ROADMAP.md).\n",
);

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version" | "-V") => {
            println!("usv {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        None | Some("--help" | "-h") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "usv: unknown argument '{other}' (this is pre-release scaffolding; see --help)"
            );
            ExitCode::from(2)
        }
    }
}
