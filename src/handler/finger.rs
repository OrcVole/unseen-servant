//! The finger response: a short profile that points at the capsule.
//!
//! Finger does not serve the content tree (ADR 0012 §4) — it answers
//! "what is this, and who runs it?" in a few lines of plain text. The
//! most useful thing those lines can do is **tell the reader where the
//! capsule actually is**, because finger itself carries no links and no
//! navigation: without an address, a successful query is a dead end.
//!
//! The operator supplies the text by dropping `finger.txt` in the state
//! directory. If they haven't, a default is generated that names the
//! capsule's addresses, so the protocol is useful the moment it is
//! switched on rather than after a documentation hunt.

use std::path::Path;

/// Addresses the profile advertises, so a reader who fingered the host
/// knows where to go next.
#[derive(Debug, Clone)]
pub struct Addresses {
    /// The capsule's hostname.
    pub host: String,
    /// The Gemini port, for the `gemini://` line.
    pub gemini_port: u16,
    /// The web mirror's base URL, if the HTTP surface is on.
    pub web_base_url: Option<String>,
    /// The gopher port, if that listener is enabled.
    pub gopher_port: Option<u16>,
}

/// Build the finger response.
///
/// `state_dir/finger.txt` wins if present — an operator's own words
/// should never be overwritten by a generated blurb. Its content is
/// served verbatim apart from line-ending normalisation.
pub async fn respond(state_dir: &Path, addr: &Addresses) -> Vec<u8> {
    if let Ok(text) = tokio::fs::read_to_string(state_dir.join("finger.txt")).await {
        return normalise(&text).into_bytes();
    }
    normalise(&default_profile(addr)).into_bytes()
}

/// The generated profile: what this is, and where to read it.
fn default_profile(addr: &Addresses) -> String {
    let mut s = String::with_capacity(512);
    s.push_str(&format!("{}\n", addr.host));
    s.push_str(&"-".repeat(addr.host.chars().count().max(1)));
    s.push('\n');
    s.push('\n');
    s.push_str("A capsule served by Unseen Servant.\n\n");

    s.push_str("Read it at:\n\n");
    if addr.gemini_port == crate::protocol::GEMINI_DEFAULT_PORT {
        s.push_str(&format!("  gemini://{}/\n", addr.host));
    } else {
        s.push_str(&format!("  gemini://{}:{}/\n", addr.host, addr.gemini_port));
    }
    if let Some(port) = addr.gopher_port {
        // Always with the port: gopher clients assume 70, and a capsule
        // on any other port is unreachable without it being written out.
        s.push_str(&format!("  gopher://{}:{}/\n", addr.host, port));
    }
    if let Some(web) = &addr.web_base_url {
        s.push_str(&format!("  {web}/\n"));
    }

    s.push('\n');
    s.push_str("Same words on every one of them — one folder, rendered to each.\n");
    s
}

/// Normalise to CRLF and guarantee a trailing newline.
///
/// Finger is a line protocol read by very old clients; a response
/// without a final newline runs into the shell prompt.
fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for line in text.split('\n') {
        out.push_str(line.strip_suffix('\r').unwrap_or(line));
        out.push_str("\r\n");
    }
    while out.ends_with("\r\n\r\n") {
        out.truncate(out.len() - 2);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "unwrap is idiomatic in tests")]
mod tests {
    use super::*;

    fn addr() -> Addresses {
        Addresses {
            host: "example.org".into(),
            gemini_port: 1965,
            web_base_url: Some("https://example.org".into()),
            gopher_port: Some(7070),
        }
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "usv-finger-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[tokio::test]
    async fn the_default_profile_points_at_the_main_page() {
        // The whole point: finger carries no links, so a query that does
        // not yield an address is a dead end.
        let d = tmp("default");
        let out = String::from_utf8(respond(&d, &addr()).await).unwrap();
        assert!(out.contains("gemini://example.org/"), "{out}");
        assert!(out.contains("https://example.org/"), "{out}");
        assert!(out.contains("gopher://example.org:7070/"), "{out}");
    }

    #[tokio::test]
    async fn a_non_default_gemini_port_is_written_out() {
        let d = tmp("port");
        let mut a = addr();
        a.gemini_port = 11965;
        let out = String::from_utf8(respond(&d, &a).await).unwrap();
        assert!(out.contains("gemini://example.org:11965/"), "{out}");
    }

    #[tokio::test]
    async fn surfaces_that_are_off_are_not_advertised() {
        let d = tmp("off");
        let a = Addresses {
            host: "example.org".into(),
            gemini_port: 1965,
            web_base_url: None,
            gopher_port: None,
        };
        let out = String::from_utf8(respond(&d, &a).await).unwrap();
        assert!(out.contains("gemini://"));
        assert!(!out.contains("gopher://"), "{out}");
        assert!(!out.contains("https://"), "{out}");
    }

    #[tokio::test]
    async fn an_operator_file_wins_over_the_generated_text() {
        let d = tmp("file");
        std::fs::write(d.join("finger.txt"), "my own words\nsecond line\n").unwrap();
        let out = String::from_utf8(respond(&d, &addr()).await).unwrap();
        assert!(out.starts_with("my own words\r\n"), "{out}");
        assert!(
            !out.contains("Unseen Servant"),
            "generated text leaked: {out}"
        );
    }

    #[tokio::test]
    async fn the_response_is_crlf_and_ends_with_a_newline() {
        let d = tmp("crlf");
        std::fs::write(d.join("finger.txt"), "no trailing newline").unwrap();
        let out = String::from_utf8(respond(&d, &addr()).await).unwrap();
        assert!(out.ends_with("\r\n"), "{out:?}");
        assert!(!out.contains('\n') || out.contains("\r\n"));
    }
}
