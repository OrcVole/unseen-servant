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
