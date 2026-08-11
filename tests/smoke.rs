//! Binary smoke tests: the CLI surface and process discipline.
//!
//! Uses `CARGO_BIN_EXE_usv` (built into cargo, zero extra dependencies) to
//! run the real binary. The wire-level regress suite (real sockets, real
//! TLS) lives in `tests/wire.rs`.

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
    for word in [
        "init",
        "fingerprint",
        "export",
        "pre-release",
        "SIGHUP",
        "--config",
    ] {
        assert!(stdout.contains(word), "help should mention {word:?}");
    }
}

#[test]
fn unknown_argument_exits_2() {
    let out = usv().arg("--frobnicate").output().expect("binary runs");
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stderr).is_empty());
}

#[test]
fn config_flag_without_path_exits_2() {
    let out = usv().arg("--config").output().expect("binary runs");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn nonexistent_explicit_config_is_a_startup_error() {
    let out = usv()
        .args(["--config", "/nonexistent/usv.toml"])
        .output()
        .expect("binary runs");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot be read"), "stderr was: {stderr}");
}

#[test]
fn zero_arg_usv_serves_and_drains_on_sigterm() {
    // ADR 0008: zero-arg `usv` starts a working capsule. Ephemeral state
    // dir and port keep the test hermetic.
    let dir = std::env::temp_dir().join(format!("usv-smoke-serve-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut child = usv()
        .env("USV_STATE_DIR", &dir)
        .env("USV_LISTEN", "127.0.0.1:0")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary spawns");

    // Wait (bounded) for the serving line on stderr.
    let stderr = child.stderr.take().expect("piped stderr");
    let mut reader = std::io::BufReader::new(stderr);
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut line = String::new();
    let mut seen_serving = false;
    while Instant::now() < deadline {
        line.clear();
        if reader.read_line(&mut line).expect("read stderr") == 0 {
            break; // EOF — process died early; the assert below reports it.
        }
        if line.contains("serving") {
            seen_serving = true;
            break;
        }
    }
    assert!(seen_serving, "server never reported serving state");

    // Graceful SIGTERM (Cloudron and systemd both stop this way).
    let pid = child.id().to_string();
    let killed = Command::new("kill")
        .args(["-TERM", &pid])
        .status()
        .expect("kill runs");
    assert!(killed.success());

    // Drain the rest of stderr so the child can never block on the pipe,
    // then require a clean exit within the deadline.
    let drain = std::thread::spawn(move || {
        let mut rest = String::new();
        use std::io::Read;
        let _ = reader.read_to_string(&mut rest);
        rest
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait().expect("wait works") {
            Some(status) => {
                assert!(status.success(), "clean exit after SIGTERM, got {status:?}");
                break;
            }
            None if Instant::now() > deadline => {
                let _ = child.kill();
                panic!("server did not exit within 20s of SIGTERM");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let _ = drain.join();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_writable_titan_zone_without_fingerprints_refuses_to_start() {
    // C4 (ADR 0006): the `[titan]` section is live now, but a writable
    // zone with no fingerprint allowlist would let anyone who can mint a
    // self-signed certificate write to the capsule. That is a startup
    // error, not a warning — the server must not come up in that state.
    let dir = std::env::temp_dir().join(format!("usv-smoke-titan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let cfg = dir.join("usv.toml");
    std::fs::write(
        &cfg,
        "[[host]]\nname = \"localhost\"\n\
         [[host.titan_zone]]\npath_prefix = \"/uploads/\"\n",
    )
    .expect("write config");
    let out = usv()
        .arg("--config")
        .arg(&cfg)
        .output()
        .expect("binary runs");
    assert!(!out.status.success(), "must refuse to start");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("fingerprint"),
        "the error must say what is missing; stderr was: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_key_in_the_titan_section_is_still_a_startup_error() {
    // The section is live, but it is not a free-for-all: unknown keys are
    // startup errors so a typo can never be silently ignored (ADR 0007).
    let dir = std::env::temp_dir().join(format!("usv-smoke-titan-typo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let cfg = dir.join("usv.toml");
    std::fs::write(&cfg, "[titan]\nenabled = true\n").expect("write config");
    let out = usv()
        .arg("--config")
        .arg(&cfg)
        .output()
        .expect("binary runs");
    assert!(!out.status.success());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sighup_reload_reaches_the_watcher_not_just_the_next_restart() {
    // Regression test for a real bug: the file watcher used to be spawned
    // once with a RenderContext frozen at startup, so a SIGHUP reload that
    // changed server.advertised_host (or the primary hostname, http_listen,
    // theme, lang) never reached the *next edit-triggered* render — only a
    // full restart would pick it up. watcher::watch now reads a live
    // tokio::sync::watch::Receiver instead, and signal_loop pushes a fresh
    // context on every successful reload; this proves it end to end against
    // the real binary, not just the unit-level watcher tests.
    let dir = std::env::temp_dir().join(format!("usv-smoke-sighup-reload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("content")).expect("mkdir");
    let cfg = dir.join("usv.toml");
    std::fs::write(
        &cfg,
        "[server]\nlisten = [\"127.0.0.1:0\"]\nhttp_listen = \"127.0.0.1:0\"\n\
         [[host]]\nname = \"before.example\"\n",
    )
    .expect("write config");

    let mut child = usv()
        .arg("--config")
        .arg(&cfg)
        .env("USV_STATE_DIR", &dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary spawns");

    let stderr = child.stderr.take().expect("piped stderr");
    let mut reader = std::io::BufReader::new(stderr);
    let mut line = String::new();
    let wait_for = |reader: &mut std::io::BufReader<std::process::ChildStderr>,
                    line: &mut String,
                    needle: &str,
                    secs: u64| {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            line.clear();
            if reader.read_line(line).expect("read stderr") == 0 {
                return false; // EOF: process died
            }
            if line.contains(needle) {
                return true;
            }
        }
        false
    };

    assert!(
        wait_for(&mut reader, &mut line, "serving", 15),
        "server never reported serving state"
    );

    // sitemap.xml, not atom.xml: the Atom feed only exists when the
    // content has dated gemsub entries (the auto-generated skeleton page
    // has none), but sitemap.xml unconditionally lists every page
    // prefixed with the advertised base URL, so it always exists.
    let sitemap_path = dir.join("html/sitemap.xml");
    let initial = std::fs::read_to_string(&sitemap_path).expect("initial sitemap.xml exists");
    assert!(
        initial.contains("before.example"),
        "initial sitemap should advertise the original hostname: {initial:?}"
    );

    // Change the advertised hostname and reload — a live config edit an
    // operator would make with the server already running.
    std::fs::write(
        &cfg,
        "[server]\nlisten = [\"127.0.0.1:0\"]\nhttp_listen = \"127.0.0.1:0\"\n\
         advertised_host = \"after.example\"\n\
         [[host]]\nname = \"before.example\"\n",
    )
    .expect("rewrite config");
    let pid = child.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-HUP", &pid])
            .status()
            .expect("kill runs")
            .success()
    );
    assert!(
        wait_for(&mut reader, &mut line, "reload complete", 15),
        "reload never completed"
    );

    // The watcher's *next* edit-triggered render — not a restart — must
    // already use the new context.
    std::fs::write(dir.join("content/new.gmi"), "# New page\n").expect("write new content");
    assert!(
        wait_for(&mut reader, &mut line, "content re-rendered", 15),
        "edit-triggered re-render never happened after reload"
    );

    let after = std::fs::read_to_string(&sitemap_path).expect("sitemap.xml exists after re-render");
    assert!(
        after.contains("after.example") && !after.contains("before.example"),
        "post-reload re-render must advertise the new hostname, not the frozen one: {after:?}"
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// The machine-readable surface (docs/agents.md)
//
// These tests exist because the whole value of `--json`, the exit codes
// and `USV_LOG_FORMAT` is that something *depends* on them. A contract
// nothing checks is a contract that will be broken by an unrelated
// change to a print statement.
// ---------------------------------------------------------------------

/// An isolated state directory, so a test never reads or mints into a
/// real capsule. Named per test to keep the suite parallel-safe.
fn scratch_state(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("usv-smoke-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Run a read-only subcommand against a scratch state dir and return
/// (exit code, stdout).
fn run_reporting(name: &str, args: &[&str]) -> (Option<i32>, String) {
    let dir = scratch_state(name);
    let out = usv()
        .args(args)
        .env("USV_STATE_DIR", &dir)
        .output()
        .expect("binary runs");
    let _ = std::fs::remove_dir_all(&dir);
    (
        out.status.code(),
        String::from_utf8(out.stdout).expect("utf8 stdout"),
    )
}

/// The smallest check that keeps the suite dependency-free while still
/// being a real one: balanced braces/brackets outside of strings, and a
/// leading `{`. It catches every way the hand-written emitter could
/// actually go wrong (a missing comma is caught by the field assertions
/// each caller makes; an unterminated string or object is caught here).
fn looks_like_one_json_object(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return false;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for c in s.chars() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            // A raw control character outside a string is never valid.
            c if (c as u32) < 0x20 && c != '\n' => return false,
            _ => {}
        }
    }
    depth == 0 && !in_string
}

#[test]
fn every_reporting_subcommand_emits_one_json_object_on_stdout() {
    for (name, args) in [
        ("json-status", &["status", "--json"][..]),
        ("json-fingerprint", &["fingerprint", "--json"][..]),
        ("json-check", &["check", "--json"][..]),
        ("json-zones", &["zones", "--json"][..]),
        ("json-stats", &["stats", "--json"][..]),
    ] {
        let (code, stdout) = run_reporting(name, args);
        assert_eq!(code, Some(0), "{args:?} should succeed");
        assert!(
            looks_like_one_json_object(&stdout),
            "{args:?} stdout was not one JSON object: {stdout:?}"
        );
        // One line, so a caller may read a single line — and several
        // invocations concatenate into valid JSON Lines.
        assert_eq!(
            stdout.trim_end().lines().count(),
            1,
            "{args:?} should emit exactly one line"
        );
    }
}

#[test]
fn status_json_carries_the_fields_an_agent_branches_on() {
    let (code, stdout) = run_reporting("json-status-fields", &["status", "--json"]);
    assert_eq!(code, Some(0));
    for key in [
        "\"capsule\"",
        "\"server_fingerprints\"",
        "\"roster\"",
        "\"zones\"",
        "\"published\"",
        "\"http_listen\"",
    ] {
        assert!(
            stdout.contains(key),
            "status --json missing {key}: {stdout}"
        );
    }
    // An absent HTTP surface is null, never an empty string — the two
    // must stay distinguishable at the point a caller branches.
    assert!(
        stdout.contains("\"http_listen\":null"),
        "an unconfigured http surface must be null: {stdout}"
    );
}

#[test]
fn json_and_prose_report_the_same_facts() {
    // The guarantee that makes `--json` safe to depend on: it is a second
    // rendering of one answer, not a second answer.
    let (_, prose) = run_reporting("parity-prose", &["fingerprint"]);
    let (_, json) = run_reporting("parity-json", &["fingerprint", "--json"]);
    let host = prose
        .split_whitespace()
        .next()
        .expect("prose names a host")
        .to_string();
    assert!(
        json.contains(&format!("\"host\":\"{host}\"")),
        "prose named {host:?} but json was {json}"
    );
}

#[test]
fn json_is_refused_where_it_has_no_meaning() {
    // Refused, not silently ignored: a caller must never believe it asked
    // for machine-readable output and receive prose.
    for subcommand in ["render", "init", "identity"] {
        let out = usv()
            .args([subcommand, "--json"])
            .output()
            .expect("binary runs");
        assert_eq!(
            out.status.code(),
            Some(2),
            "--json on {subcommand} should be a usage error"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("--json"),
            "the error for {subcommand} should name the flag: {stderr}"
        );
    }
}

#[test]
fn the_exit_code_contract_holds() {
    // 0 success / 1 ran-and-failed / 2 bad command line. Documented in
    // `--help` and docs/agents.md; scripts and agents may depend on it.
    let (ok, _) = run_reporting("exit-ok", &["stats"]);
    assert_eq!(ok, Some(0), "a successful command exits 0");

    let usage = usv().arg("--frobnicate").output().expect("binary runs");
    assert_eq!(usage.status.code(), Some(2), "a bad flag exits 2");

    let failure = usv()
        .args(["--config", "/nonexistent/usv.toml", "stats"])
        .output()
        .expect("binary runs");
    assert_eq!(
        failure.status.code(),
        Some(1),
        "a command that ran and failed exits 1"
    );
}

#[test]
fn usv_log_format_json_emits_structured_lines_on_stderr() {
    let dir = scratch_state("log-json");
    let out = usv()
        .arg("stats")
        .env("USV_STATE_DIR", &dir)
        .env("USV_LOG_FORMAT", "json")
        .output()
        .expect("binary runs");
    let _ = std::fs::remove_dir_all(&dir);
    let stderr = String::from_utf8(out.stderr).expect("utf8 stderr");
    let first = stderr.lines().next().expect("at least one log line");
    assert!(
        looks_like_one_json_object(first),
        "log line was not JSON: {first:?}"
    );
    for key in ["\"level\"", "\"timestamp\"", "\"target\""] {
        assert!(first.contains(key), "log line missing {key}: {first}");
    }
}

#[test]
fn the_default_log_format_is_unchanged_by_the_json_option() {
    let dir = scratch_state("log-text");
    let out = usv()
        .arg("stats")
        .env("USV_STATE_DIR", &dir)
        .output()
        .expect("binary runs");
    let _ = std::fs::remove_dir_all(&dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let first = stderr.lines().next().expect("at least one log line");
    assert!(
        !first.trim_start().starts_with('{'),
        "the default format must stay human-readable: {first:?}"
    );
    assert!(first.contains("INFO"), "log line was: {first:?}");
}
