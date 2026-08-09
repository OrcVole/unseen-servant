//! Binary smoke tests — the C0 sliver of the regress suite.
//!
//! Uses `CARGO_BIN_EXE_usv` (built into cargo, zero extra dependencies) to run
//! the real binary. The full regress runner (real sockets, gmid-style) arrives
//! in C1 and absorbs these.

use std::process::Command;

fn usv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_usv"))
}

#[test]
fn version_prints_name_and_semver() {
    let out = usv().arg("--version").output().expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(stdout.trim(), format!("usv {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_mentions_the_planned_surface_and_exits_zero() {
    let out = usv().arg("--help").output().expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    for word in ["init", "fingerprint", "export", "pre-release"] {
        assert!(stdout.contains(word), "help should mention {word:?}");
    }
}

#[test]
fn no_args_prints_help_not_a_server() {
    // ADR 0008 promises zero-arg `usv` starts a capsule — from C1 on. At C0 it
    // must NOT pretend: help text, success, nothing listening.
    let out = usv().output().expect("binary runs");
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).is_empty());
}

#[test]
fn unknown_argument_exits_2() {
    let out = usv().arg("--frobnicate").output().expect("binary runs");
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stderr).is_empty());
}
