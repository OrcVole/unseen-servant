//! C5 CLI subcommands (`docs/BUILD-PLAN.md` C5): `status`, `fingerprint`,
//! `check`, `zones`, `stats`, `render`, `export`. `usv init` (the ratatui
//! wizard) lives in `main.rs`; Tor/I2P affordances (`server.advertised_host`,
//! onion-hostname cert slots, no-SNI tolerance) needed no CLI surface —
//! they're `config`/`identity`/`tls` behavior, documented in
//! `INTEGRATIONS.md`.
//!
//! Kept in the library, not `main.rs`, on the same principle every other
//! phase has followed: business logic goes where it can be unit-tested
//! without a subprocess. This module owns *formatting and read-only
//! inspection*; `main.rs` owns argument parsing and the I/O each
//! subcommand needs before calling in here (loading config, opening the
//! identity store, invoking the render pipeline).
//!
//! One deliberate split worth stating up front: **`stats` reads the
//! already-rendered tree; `render` performs a fresh one.** They sound
//! adjacent but differ in the property that matters most for a command an
//! operator might run against a *live* capsule — `stats` never touches
//! `state_dir/html`, `render` always does (the same atomic staging-swap
//! the server itself uses). An operator asking "what's currently
//! published" should never have to accept "and also, it just got
//! rebuilt" as the price of asking.

use std::path::Path;

use crate::config::Config;
use crate::identity::IdentityStore;
use crate::roster::{Capability, Roster};

/// `usv fingerprint`: every configured hostname's server certificate
/// fingerprint, one per line — what an operator publishes out-of-band for
/// a TOFU client to verify against on first connection.
pub fn format_fingerprints(store: &IdentityStore) -> String {
    let mut out = String::new();
    for (host, fp) in store.fingerprints() {
        out.push_str(host);
        out.push_str("  ");
        out.push_str(&fp);
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str("(no hosts configured)\n");
    }
    out
}

/// `usv zones`: every certificate zone and Titan zone, per host, in the
/// order they'll actually be matched (longest-prefix-wins, but listed in
/// config order here — the *matching* order is a request-time property of
/// the path, not a listing property). Fingerprint values are the
/// operator's own config; there is no confidentiality boundary being
/// crossed by printing them back.
pub fn format_zones(config: &Config) -> String {
    let mut out = String::new();
    for host in &config.hosts {
        out.push_str(&host.name);
        out.push('\n');
        if host.cert_zones.is_empty() && host.titan_zones.is_empty() {
            out.push_str("  (no zones configured)\n");
            continue;
        }
        for zone in &host.cert_zones {
            out.push_str("  read   ");
            out.push_str(&zone.path_prefix);
            if zone.allowed_fingerprints.is_empty() {
                out.push_str("  (any certificate)\n");
            } else {
                out.push_str(&format!(
                    "  ({} fingerprint(s))\n",
                    zone.allowed_fingerprints.len()
                ));
            }
        }
        for zone in &host.titan_zones {
            out.push_str("  titan  ");
            out.push_str(&zone.path_prefix);
            out.push_str(&format!(
                "  ({} fingerprint(s), {} identity(ies), max {} bytes{})\n",
                zone.allowed_fingerprints.len(),
                zone.allowed_identities.len(),
                zone.max_upload_bytes,
                if zone.allow_delete {
                    ", delete allowed"
                } else {
                    ""
                }
            ));
        }
    }
    if out.is_empty() {
        out.push_str("(no hosts configured)\n");
    }
    out
}

/// `usv fingerprint`/`zones`/`stats`: the identity roster, one line per
/// configured `[[identity]]`. Never prints a fingerprint's superseded
/// keys' status beyond whether the rotation window is open — that
/// decision depends on the clock, which is `usv status`'s business, not a
/// static listing's.
pub fn format_roster(config: &Config) -> String {
    let mut out = String::new();
    for identity in config.roster.identities() {
        out.push_str(&identity.label);
        out.push_str("  ");
        out.push_str(&identity.fingerprint);
        if !identity.capabilities.is_empty() {
            out.push_str("  [");
            let caps: Vec<&str> = identity.capabilities.iter().map(|c| c.as_str()).collect();
            out.push_str(&caps.join(", "));
            out.push(']');
        }
        if !identity.superseded.is_empty() {
            out.push_str(&format!(
                "  (rotating, {} old key(s), window closes {})",
                identity.superseded.len(),
                identity
                    .superseded_until
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "?".to_string())
            ));
        }
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str(
            "(no [[identity]] entries configured — zones may still list raw fingerprints)\n",
        );
    }
    out
}

/// Why an `identity add`/`rotate`/`revoke` snippet request was refused.
/// Every variant is caught *before* any TOML is generated — the whole
/// point of validating here is that a mistake shows up as an immediate,
/// specific CLI error, not as usv refusing to start after the operator
/// has already pasted a bad snippet into their config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentitySnippetError {
    /// The label was empty or all whitespace.
    EmptyLabel,
    /// Not 64 lowercase-able hex characters — the same rule
    /// [`crate::roster::Roster`] enforces at config-load time.
    MalformedFingerprint(String),
    /// Not one of the closed set of capability names.
    UnknownCapability(String),
    /// `add` was asked to create a label that's already in the loaded
    /// roster — generating a colliding snippet would silently shadow the
    /// existing entry once pasted in, which usv's config loader refuses
    /// (`RosterError::DuplicateLabel`) but a snippet-only command should
    /// catch first, before the operator gets that far.
    LabelAlreadyExists(String),
    /// `rotate`/`revoke` named a label the loaded roster doesn't have —
    /// there's nothing to build a snippet *for*.
    LabelNotFound(String),
    /// Not a `YYYY-MM-DD` date.
    BadDate(String),
}

impl std::fmt::Display for IdentitySnippetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentitySnippetError::EmptyLabel => f.write_str("label must not be empty"),
            IdentitySnippetError::MalformedFingerprint(v) => {
                write!(f, "{v:?} is not a SHA-256 fingerprint (64 hex characters)")
            }
            IdentitySnippetError::UnknownCapability(v) => {
                let known: Vec<&str> = Capability::ALL.iter().map(|c| c.as_str()).collect();
                write!(f, "unknown capability {v:?}; known: {}", known.join(", "))
            }
            IdentitySnippetError::LabelAlreadyExists(l) => write!(
                f,
                "identity {l:?} already exists in the loaded config; pick a different \
                 label, or use 'usv identity rotate' if you mean to change its key"
            ),
            IdentitySnippetError::LabelNotFound(l) => {
                write!(f, "no identity labelled {l:?} in the loaded config")
            }
            IdentitySnippetError::BadDate(v) => write!(f, "{v:?} is not a YYYY-MM-DD date"),
        }
    }
}

impl std::error::Error for IdentitySnippetError {}

fn parse_snippet_date(s: &str) -> Result<time::Date, IdentitySnippetError> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    time::Date::parse(s, &format).map_err(|_| IdentitySnippetError::BadDate(s.to_string()))
}

fn parse_capabilities(names: &[String]) -> Result<Vec<Capability>, IdentitySnippetError> {
    names
        .iter()
        .map(|n| {
            Capability::parse(n).ok_or_else(|| IdentitySnippetError::UnknownCapability(n.clone()))
        })
        .collect()
}

/// `usv identity add`: a ready-to-paste `[[identity]]` block. Never
/// touches the config file (see the module docs on `check`/`stats`'
/// read-only stance — the director confirmed the same posture applies
/// here 2026-08-10: printing a snippet for the operator to paste in
/// themselves is the whole design, not a stopgap for a future `--write`).
pub fn identity_add_snippet(
    roster: &Roster,
    label: &str,
    fingerprint: &str,
    capabilities: &[String],
    enrolled: Option<&str>,
) -> Result<String, IdentitySnippetError> {
    if label.trim().is_empty() {
        return Err(IdentitySnippetError::EmptyLabel);
    }
    if !crate::roster::is_sha256_hex(fingerprint) {
        return Err(IdentitySnippetError::MalformedFingerprint(
            fingerprint.to_string(),
        ));
    }
    if roster.by_label(label).is_some() {
        return Err(IdentitySnippetError::LabelAlreadyExists(label.to_string()));
    }
    let caps = parse_capabilities(capabilities)?;
    let enrolled_date = enrolled.map(parse_snippet_date).transpose()?;

    let mut out = String::from(
        "[[identity]]
",
    );
    out.push_str(&format!(
        "label = {label:?}
"
    ));
    out.push_str(&format!(
        "fingerprint = {fingerprint:?}
"
    ));
    if !caps.is_empty() {
        let list: Vec<String> = caps.iter().map(|c| format!("{:?}", c.as_str())).collect();
        out.push_str(&format!(
            "capabilities = [{}]
",
            list.join(", ")
        ));
    }
    if let Some(d) = enrolled_date {
        out.push_str(&format!(
            "enrolled = {:?}
",
            d.to_string()
        ));
    }
    Ok(out)
}

/// `usv identity rotate`: a snippet updating an *existing* label to a new
/// current fingerprint, moving its old one to `superseded` with the given
/// deadline. Requires the label to already be in the loaded roster —
/// rotation is meaningless for an identity that doesn't exist yet.
pub fn identity_rotate_snippet(
    roster: &Roster,
    label: &str,
    new_fingerprint: &str,
    until: &str,
) -> Result<String, IdentitySnippetError> {
    let existing = roster
        .by_label(label)
        .ok_or_else(|| IdentitySnippetError::LabelNotFound(label.to_string()))?;
    if !crate::roster::is_sha256_hex(new_fingerprint) {
        return Err(IdentitySnippetError::MalformedFingerprint(
            new_fingerprint.to_string(),
        ));
    }
    let until_date = parse_snippet_date(until)?;

    let mut out = String::from(
        "[[identity]]
",
    );
    out.push_str(&format!(
        "label = {label:?}
"
    ));
    out.push_str(&format!(
        "fingerprint = {new_fingerprint:?}
"
    ));
    out.push_str(&format!(
        "superseded = [{:?}]
",
        existing.fingerprint
    ));
    out.push_str(&format!(
        "superseded_until = {:?}
",
        until_date.to_string()
    ));
    if !existing.capabilities.is_empty() {
        let list: Vec<String> = existing
            .capabilities
            .iter()
            .map(|c| format!("{:?}", c.as_str()))
            .collect();
        out.push_str(&format!(
            "capabilities = [{}]
",
            list.join(", ")
        ));
    }
    Ok(out)
}

/// `usv identity revoke`: either a snippet with named capabilities
/// removed, or (when `all` is set) a plain instruction to delete the
/// whole block — there is no "empty but present" form of an identity
/// worth generating TOML for.
pub fn identity_revoke_snippet(
    roster: &Roster,
    label: &str,
    capabilities: &[String],
    all: bool,
) -> Result<String, IdentitySnippetError> {
    let existing = roster
        .by_label(label)
        .ok_or_else(|| IdentitySnippetError::LabelNotFound(label.to_string()))?;
    if all {
        return Ok(format!(
            "Remove the entire [[identity]] block labelled {label:?} from usv.toml.
"
        ));
    }
    let to_remove = parse_capabilities(capabilities)?;
    let remaining: Vec<&Capability> = existing
        .capabilities
        .iter()
        .filter(|c| !to_remove.contains(c))
        .collect();

    let mut out = String::from(
        "[[identity]]
",
    );
    out.push_str(&format!(
        "label = {label:?}
"
    ));
    out.push_str(&format!(
        "fingerprint = {:?}
",
        existing.fingerprint
    ));
    if remaining.is_empty() {
        out.push_str(
            "capabilities = []
",
        );
    } else {
        let list: Vec<String> = remaining
            .iter()
            .map(|c| format!("{:?}", c.as_str()))
            .collect();
        out.push_str(&format!(
            "capabilities = [{}]
",
            list.join(", ")
        ));
    }
    Ok(out)
}

/// What `lint_content` found. Never an error by itself — a lint surfaces
/// things worth an operator's attention, it does not fail the command;
/// only genuine I/O trouble (an unreadable content directory) does that.
#[derive(Debug, Default)]
pub struct ContentLint {
    /// `.gmi` files found under the content directory.
    pub pages_found: usize,
    /// Human-readable notes: nothing here is fatal, all of it is
    /// information an operator would want before publishing.
    pub notes: Vec<String>,
}

/// Read-only content-tree lint (`usv check`'s content half): walk every
/// `.gmi` file, parse it (the parser never fails — `gemtext::parse`'s
/// whole contract, proven by fuzzing), and note anything worth a second
/// look. Never writes anything, never invokes the render pipeline — safe
/// to run against a live capsule's content directory at any time.
pub async fn lint_content(content_dir: &Path) -> std::io::Result<ContentLint> {
    let mut lint = ContentLint::default();
    if tokio::fs::metadata(content_dir).await.is_err() {
        lint.notes.push(format!(
            "content directory {} does not exist yet (a fresh capsule writes its \
             skeleton here on first start)",
            content_dir.display()
        ));
        return Ok(lint);
    }
    walk(content_dir, content_dir, &mut lint).await?;
    Ok(lint)
}

fn walk<'a>(
    root: &'a Path,
    dir: &'a Path,
    lint: &'a mut ContentLint,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                walk(root, &path, lint).await?;
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if ext != "gmi" {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let name = relative.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == crate::render::pipeline::GENERATED_FEED_NAME
                || name == crate::render::pipeline::GENERATED_MAP_NAME
            {
                lint.notes.push(format!(
                    "{} is a generated filename (usv rewrites it on every render; \
                     authored content here would be silently overwritten)",
                    relative.display()
                ));
            }
            match tokio::fs::read_to_string(&path).await {
                Ok(text) => {
                    let lines = crate::render::gemtext::parse(&text);
                    if lines.is_empty() {
                        lint.notes.push(format!("{} is empty", relative.display()));
                    }
                    lint.pages_found += 1;
                }
                Err(e) => {
                    lint.notes
                        .push(format!("{} could not be read: {e}", relative.display()));
                }
            }
        }
        Ok(())
    })
}

/// `usv check`'s combined report: a config summary (config already loaded
/// successfully by the time this runs — that half of "check" is `Config::
/// load`'s own validation) plus the content lint.
pub fn format_check_report(config: &Config, lint: &ContentLint) -> String {
    let mut out = String::new();
    out.push_str("config: valid\n");
    out.push_str(&format!("  hosts: {}\n", config.hosts.len()));
    out.push_str(&format!("  lang: {}\n", config.lang));
    out.push_str(&format!("  theme: {}\n", config.theme.name));
    out.push_str(&format!(
        "  identities: {}\n",
        config.roster.identities().len()
    ));
    out.push_str(&format!("content: {} page(s) found\n", lint.pages_found));
    if lint.notes.is_empty() {
        out.push_str("  no issues noted\n");
    } else {
        for note in &lint.notes {
            out.push_str("  - ");
            out.push_str(note);
            out.push('\n');
        }
    }
    out
}

/// A snapshot of what's currently in the *rendered* tree, without
/// triggering a render. `None` fields mean "not present in the tree
/// right now" — distinct from an error, since a fresh or Gemini-only
/// capsule legitimately has no `html` directory or no `atom.xml` yet.
#[derive(Debug, Default)]
pub struct PublishedStats {
    /// Whether `state_dir/html` exists at all.
    pub html_tree_present: bool,
    /// `.html` files found under the rendered tree, if present.
    pub html_pages: usize,
    /// Whether `robots.txt` is present in the rendered tree.
    pub has_robots: bool,
    /// Whether `sitemap.xml` is present in the rendered tree.
    pub has_sitemap: bool,
    /// Whether `llms.txt` is present in the rendered tree.
    pub has_llms_txt: bool,
    /// Whether `atom.xml` is present in the rendered tree.
    pub has_atom: bool,
}

/// Inspect `state_dir/html` without rendering anything. Read-only by
/// construction: every check here is a `tokio::fs::metadata` call, never
/// a write.
pub async fn inspect_published(state_dir: &Path) -> PublishedStats {
    let html_dir = state_dir.join("html");
    let mut stats = PublishedStats {
        html_tree_present: tokio::fs::metadata(&html_dir).await.is_ok(),
        ..Default::default()
    };
    if !stats.html_tree_present {
        return stats;
    }
    stats.has_robots = tokio::fs::metadata(html_dir.join("robots.txt"))
        .await
        .is_ok();
    stats.has_sitemap = tokio::fs::metadata(html_dir.join("sitemap.xml"))
        .await
        .is_ok();
    stats.has_llms_txt = tokio::fs::metadata(html_dir.join("llms.txt")).await.is_ok();
    stats.has_atom = tokio::fs::metadata(html_dir.join("atom.xml")).await.is_ok();
    let mut count = 0usize;
    let _ = count_html(&html_dir, &mut count).await;
    stats.html_pages = count;
    stats
}

fn count_html<'a>(
    dir: &'a Path,
    count: &'a mut usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if entry.file_type().await?.is_dir() {
                count_html(&path, count).await?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
                *count += 1;
            }
        }
        Ok(())
    })
}

/// `usv stats`: format an [`PublishedStats`] snapshot for the terminal.
pub fn format_published_stats(stats: &PublishedStats) -> String {
    if !stats.html_tree_present {
        return "no rendered HTML tree yet — run `usv render` or start the server once\n"
            .to_string();
    }
    let mut out = String::new();
    out.push_str(&format!("pages: {}\n", stats.html_pages));
    out.push_str(&format!("robots.txt: {}\n", present(stats.has_robots)));
    out.push_str(&format!("sitemap.xml: {}\n", present(stats.has_sitemap)));
    out.push_str(&format!("llms.txt: {}\n", present(stats.has_llms_txt)));
    out.push_str(&format!("atom.xml: {}\n", present(stats.has_atom)));
    out
}

fn present(b: bool) -> &'static str {
    if b { "present" } else { "absent" }
}

/// Why `usv export` refused to run.
#[derive(Debug)]
pub enum ExportError {
    /// `state_dir/html` doesn't exist — nothing to export yet.
    NothingRendered,
    /// The destination already exists and isn't empty. Refused rather
    /// than merged or overwritten, matching the house rule this codebase
    /// applies everywhere a write could destroy existing material
    /// (identity minting, the content skeleton): silently overwriting
    /// whatever the operator has at that path is exactly the failure
    /// mode worth a named error instead of a surprise.
    DestinationNotEmpty(std::path::PathBuf),
    /// A filesystem operation failed partway through the copy.
    Io(std::io::Error),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::NothingRendered => f.write_str(
                "no rendered HTML tree yet — run `usv render` (or start the server once) first",
            ),
            ExportError::DestinationNotEmpty(p) => write!(
                f,
                "{} already exists and is not empty; export refuses to overwrite it \
                 (remove it first, or choose an empty destination)",
                p.display()
            ),
            ExportError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ExportError {}

/// `usv export`: copy the already-rendered `state_dir/html` tree to
/// `destination`, verbatim.
///
/// This is deliberately almost the whole implementation — a validated
/// recursive copy, nothing more. usv's rendered HTML tree already *is* a
/// self-contained static site (ADR 0004; no build step, no server-side
/// includes, no absolute-path assumptions beyond `/style.css` and the
/// other root-relative links every page already uses); the point of
/// `export` is to hand an operator that folder without them needing to
/// know usv's internal `state_dir` layout, not to transform it into
/// something new. "Your capsule's web mirror as an OnionShare site" is
/// documented (`docs/notes/integration-ideas.md`) as a copy-the-folder
/// recipe with zero new code — this function is that recipe, made
/// discoverable and given a destination-safety check.
///
/// Never renders (matching `stats`, not `render`): exporting is a read of
/// whatever's already published, not a trigger to publish something new.
/// Returns the number of files copied.
pub async fn export_html_tree(state_dir: &Path, destination: &Path) -> Result<usize, ExportError> {
    let html_dir = state_dir.join("html");
    if tokio::fs::metadata(&html_dir).await.is_err() {
        return Err(ExportError::NothingRendered);
    }
    match tokio::fs::metadata(destination).await {
        Ok(meta) if meta.is_dir() => {
            let mut entries = tokio::fs::read_dir(destination)
                .await
                .map_err(ExportError::Io)?;
            if entries
                .next_entry()
                .await
                .map_err(ExportError::Io)?
                .is_some()
            {
                return Err(ExportError::DestinationNotEmpty(destination.to_path_buf()));
            }
        }
        Ok(_) => {
            // Exists and is not a directory (a plain file, say) — same
            // refusal, for the same reason.
            return Err(ExportError::DestinationNotEmpty(destination.to_path_buf()));
        }
        Err(_) => {} // doesn't exist yet: created below, the normal case.
    }
    tokio::fs::create_dir_all(destination)
        .await
        .map_err(ExportError::Io)?;
    let mut copied = 0usize;
    copy_tree(&html_dir, destination, &mut copied)
        .await
        .map_err(ExportError::Io)?;
    Ok(copied)
}

fn copy_tree<'a>(
    src: &'a Path,
    dst: &'a Path,
    copied: &'a mut usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if entry.file_type().await?.is_dir() {
                tokio::fs::create_dir_all(&dst_path).await?;
                copy_tree(&src_path, &dst_path, copied).await?;
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
                *copied += 1;
            }
        }
        Ok(())
    })
}

/// `usv status`: the at-a-glance dashboard — config summary, server
/// fingerprints, the identity roster, zones, and what's currently
/// published, one command instead of five. Each section reuses the same
/// formatter its own subcommand does, so `status`'s output can never
/// drift from what `fingerprint`/`zones`/`stats` report individually.
pub fn format_status(config: &Config, store: &IdentityStore, published: &PublishedStats) -> String {
    let mut out = String::new();
    out.push_str("== capsule ==\n");
    out.push_str(&format!("hosts: {}\n", config.hosts.len()));
    out.push_str(&format!("lang: {}\n", config.lang));
    out.push_str(&format!("theme: {}\n", config.theme.name));
    out.push_str(&format!(
        "gemini listen: {}\n",
        config
            .listen
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!(
        "http surface: {}\n",
        config
            .http_listen
            .map(|a| a.to_string())
            .unwrap_or_else(|| "off".to_string())
    ));
    out.push_str(&format!(
        "advertised host: {}\n",
        config
            .advertised_host
            .as_deref()
            .unwrap_or("(default: first configured host)")
    ));
    out.push_str("\n== server fingerprints ==\n");
    out.push_str(&format_fingerprints(store));
    out.push_str("\n== identity roster ==\n");
    out.push_str(&format_roster(config));
    out.push_str("\n== zones ==\n");
    out.push_str(&format_zones(config));
    out.push_str("\n== published ==\n");
    out.push_str(&format_published_stats(published));
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unwrap/unwrap_err are idiomatic in tests"
)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("usv-cli-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[tokio::test]
    async fn lint_reports_page_count_and_no_notes_for_clean_content() {
        let dir = tmpdir("lint-clean");
        std::fs::write(dir.join("index.gmi"), "# Home\n\nHello.\n").unwrap();
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        std::fs::write(dir.join("notes/a.gmi"), "# A\n").unwrap();
        let lint = lint_content(&dir).await.unwrap();
        assert_eq!(lint.pages_found, 2);
        assert!(lint.notes.is_empty(), "{:?}", lint.notes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn lint_flags_a_generated_filename_present_as_authored_content() {
        let dir = tmpdir("lint-generated-name");
        std::fs::write(dir.join("map.gmi"), "# not actually generated yet\n").unwrap();
        let lint = lint_content(&dir).await.unwrap();
        assert!(lint.notes.iter().any(|n| n.contains("map.gmi")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn lint_flags_an_empty_page() {
        let dir = tmpdir("lint-empty");
        std::fs::write(dir.join("blank.gmi"), "").unwrap();
        let lint = lint_content(&dir).await.unwrap();
        assert!(lint.notes.iter().any(|n| n.contains("empty")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn lint_of_a_missing_content_dir_is_a_note_not_an_error() {
        let dir = tmpdir("lint-missing");
        let missing = dir.join("does-not-exist");
        let lint = lint_content(&missing).await.unwrap();
        assert_eq!(lint.pages_found, 0);
        assert!(!lint.notes.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn lint_ignores_non_gmi_files() {
        let dir = tmpdir("lint-nongmi");
        std::fs::write(dir.join("index.gmi"), "# Home\n").unwrap();
        std::fs::write(dir.join("photo.png"), b"\x89PNG").unwrap();
        std::fs::write(dir.join("notes.txt"), "not gemtext").unwrap();
        let lint = lint_content(&dir).await.unwrap();
        assert_eq!(lint.pages_found, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inspect_published_of_a_fresh_state_dir_shows_nothing_present() {
        let dir = tmpdir("inspect-fresh");
        let stats = inspect_published(&dir).await;
        assert!(!stats.html_tree_present);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inspect_published_counts_pages_and_finds_generated_files() {
        let dir = tmpdir("inspect-real");
        let html = dir.join("html");
        std::fs::create_dir_all(html.join("notes")).unwrap();
        std::fs::write(html.join("index.html"), "<html></html>").unwrap();
        std::fs::write(html.join("notes/a.html"), "<html></html>").unwrap();
        std::fs::write(html.join("robots.txt"), "User-agent: *\n").unwrap();
        std::fs::write(html.join("sitemap.xml"), "<urlset></urlset>").unwrap();

        let stats = inspect_published(&dir).await;
        assert!(stats.html_tree_present);
        assert_eq!(stats.html_pages, 2);
        assert!(stats.has_robots);
        assert!(stats.has_sitemap);
        assert!(!stats.has_llms_txt, "was not written in this fixture");
        assert!(!stats.has_atom, "was not written in this fixture");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn inspecting_never_writes_anything() {
        // The whole point of `stats` vs `render`: this must be provably
        // read-only. Snapshot the directory's mtime-relevant listing
        // before and after; nothing should change.
        let dir = tmpdir("inspect-readonly");
        let html = dir.join("html");
        std::fs::create_dir_all(&html).unwrap();
        std::fs::write(html.join("index.html"), "x").unwrap();
        let before: Vec<_> = std::fs::read_dir(&html)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();

        let _ = inspect_published(&dir).await;

        let after: Vec<_> = std::fs::read_dir(&html)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            before, after,
            "inspect_published must not create/remove files"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_published_stats_of_an_absent_tree_says_so_plainly() {
        let stats = PublishedStats::default();
        let out = format_published_stats(&stats);
        assert!(out.contains("no rendered HTML tree"));
    }

    #[test]
    fn format_published_stats_reports_every_field() {
        let stats = PublishedStats {
            html_tree_present: true,
            html_pages: 3,
            has_robots: true,
            has_sitemap: false,
            has_llms_txt: true,
            has_atom: false,
        };
        let out = format_published_stats(&stats);
        assert!(out.contains("pages: 3"));
        assert!(out.contains("robots.txt: present"));
        assert!(out.contains("sitemap.xml: absent"));
        assert!(out.contains("llms.txt: present"));
        assert!(out.contains("atom.xml: absent"));
    }

    fn roster_with_one(label: &str, fp: &str, caps: &[Capability]) -> Roster {
        Roster::new(vec![crate::roster::Identity {
            label: label.to_string(),
            fingerprint: fp.to_string(),
            superseded: Vec::new(),
            superseded_until: None,
            capabilities: caps.to_vec(),
            enrolled: None,
        }])
        .unwrap()
    }

    #[test]
    fn identity_add_produces_a_pasteable_block() {
        let roster = Roster::new(Vec::new()).unwrap();
        let fp = "a".repeat(64);
        let out = identity_add_snippet(
            &roster,
            "scribe",
            &fp,
            &["titan-write".to_string()],
            Some("2026-08-10"),
        )
        .unwrap();
        assert!(out.contains("[[identity]]"));
        assert!(out.contains("label = \"scribe\""));
        assert!(out.contains(&fp));
        assert!(out.contains("capabilities = [\"titan-write\"]"));
        assert!(out.contains("enrolled = \"2026-08-10\""));
    }

    #[test]
    fn identity_add_rejects_a_malformed_fingerprint() {
        let roster = Roster::new(Vec::new()).unwrap();
        let err = identity_add_snippet(&roster, "scribe", "not-hex", &[], None).unwrap_err();
        assert!(matches!(err, IdentitySnippetError::MalformedFingerprint(_)));
    }

    #[test]
    fn identity_add_rejects_an_empty_label() {
        let roster = Roster::new(Vec::new()).unwrap();
        let fp = "a".repeat(64);
        let err = identity_add_snippet(&roster, "  ", &fp, &[], None).unwrap_err();
        assert_eq!(err, IdentitySnippetError::EmptyLabel);
    }

    #[test]
    fn identity_add_rejects_an_unknown_capability() {
        let roster = Roster::new(Vec::new()).unwrap();
        let fp = "a".repeat(64);
        let err =
            identity_add_snippet(&roster, "scribe", &fp, &["write".to_string()], None).unwrap_err();
        assert!(matches!(err, IdentitySnippetError::UnknownCapability(_)));
    }

    #[test]
    fn identity_add_refuses_a_label_already_in_the_roster() {
        let roster = roster_with_one("scribe", &"a".repeat(64), &[]);
        let err = identity_add_snippet(&roster, "scribe", &"b".repeat(64), &[], None).unwrap_err();
        assert!(matches!(err, IdentitySnippetError::LabelAlreadyExists(_)));
    }

    #[test]
    fn identity_add_rejects_a_bad_date() {
        let roster = Roster::new(Vec::new()).unwrap();
        let fp = "a".repeat(64);
        let err =
            identity_add_snippet(&roster, "scribe", &fp, &[], Some("not-a-date")).unwrap_err();
        assert!(matches!(err, IdentitySnippetError::BadDate(_)));
    }

    #[test]
    fn identity_rotate_moves_the_old_key_to_superseded() {
        let old_fp = "a".repeat(64);
        let new_fp = "b".repeat(64);
        let roster = roster_with_one("scribe", &old_fp, &[Capability::TitanWrite]);
        let out = identity_rotate_snippet(&roster, "scribe", &new_fp, "2099-01-01").unwrap();
        assert!(out.contains(&format!("fingerprint = {new_fp:?}")));
        assert!(out.contains(&format!("superseded = [{old_fp:?}]")));
        assert!(out.contains("superseded_until = \"2099-01-01\""));
        assert!(
            out.contains("capabilities = [\"titan-write\"]"),
            "rotation must preserve existing capabilities: {out}"
        );
    }

    #[test]
    fn identity_rotate_of_an_unknown_label_is_refused() {
        let roster = Roster::new(Vec::new()).unwrap();
        let err =
            identity_rotate_snippet(&roster, "nobody", &"a".repeat(64), "2099-01-01").unwrap_err();
        assert!(matches!(err, IdentitySnippetError::LabelNotFound(_)));
    }

    #[test]
    fn identity_rotate_rejects_a_malformed_new_fingerprint() {
        let roster = roster_with_one("scribe", &"a".repeat(64), &[]);
        let err = identity_rotate_snippet(&roster, "scribe", "not-hex", "2099-01-01").unwrap_err();
        assert!(matches!(err, IdentitySnippetError::MalformedFingerprint(_)));
    }

    #[test]
    fn identity_revoke_all_says_to_delete_the_block() {
        let roster = roster_with_one("scribe", &"a".repeat(64), &[Capability::TitanWrite]);
        let out = identity_revoke_snippet(&roster, "scribe", &[], true).unwrap();
        assert!(out.to_lowercase().contains("remove"));
        assert!(out.contains("scribe"));
        // It's an instruction referring to the block by name, not a
        // generated snippet — no other TOML field appears alongside it.
        assert!(!out.contains("fingerprint ="));
        assert!(!out.contains("capabilities ="));
    }

    #[test]
    fn identity_revoke_one_capability_keeps_the_others() {
        let roster = roster_with_one(
            "scribe",
            &"a".repeat(64),
            &[Capability::TitanWrite, Capability::Read],
        );
        let out = identity_revoke_snippet(&roster, "scribe", &["titan-write".to_string()], false)
            .unwrap();
        assert!(out.contains("capabilities = [\"read\"]"));
        assert!(!out.contains("titan-write"));
    }

    #[test]
    fn identity_revoke_every_capability_leaves_an_empty_list() {
        let roster = roster_with_one("scribe", &"a".repeat(64), &[Capability::TitanWrite]);
        let out = identity_revoke_snippet(&roster, "scribe", &["titan-write".to_string()], false)
            .unwrap();
        assert!(out.contains("capabilities = []"));
    }

    #[test]
    fn identity_revoke_of_an_unknown_label_is_refused() {
        let roster = Roster::new(Vec::new()).unwrap();
        let err = identity_revoke_snippet(&roster, "nobody", &[], true).unwrap_err();
        assert!(matches!(err, IdentitySnippetError::LabelNotFound(_)));
    }

    #[test]
    fn format_check_report_includes_config_summary_and_lint() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"a.example\"\n",
            &crate::config::EnvOverrides::default(),
        )
        .unwrap();
        let lint = ContentLint {
            pages_found: 2,
            notes: vec!["blank.gmi is empty".to_string()],
        };
        let out = format_check_report(&cfg, &lint);
        assert!(out.contains("hosts: 1"));
        assert!(out.contains("2 page(s) found"));
        assert!(out.contains("blank.gmi is empty"));
    }

    #[test]
    fn format_check_report_of_clean_content_says_no_issues() {
        let cfg = Config::from_toml_str("", &crate::config::EnvOverrides::default()).unwrap();
        let lint = ContentLint {
            pages_found: 1,
            notes: Vec::new(),
        };
        let out = format_check_report(&cfg, &lint);
        assert!(out.contains("no issues noted"));
    }

    #[test]
    fn format_zones_of_an_empty_config_says_so() {
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"a.example\"\n",
            &crate::config::EnvOverrides::default(),
        )
        .unwrap();
        let out = format_zones(&cfg);
        assert!(out.contains("a.example"));
        assert!(out.contains("no zones configured"));
    }

    #[test]
    fn format_zones_lists_read_and_titan_zones_distinctly() {
        let hex = "a".repeat(64);
        let cfg = Config::from_toml_str(
            &format!(
                "[[host]]\nname = \"a.example\"\n\
                 [[host.cert_zone]]\npath_prefix = \"/private/\"\nfingerprints = [\"{hex}\"]\n\
                 [[host.titan_zone]]\npath_prefix = \"/uploads/\"\nfingerprints = [\"{hex}\"]\n"
            ),
            &crate::config::EnvOverrides::default(),
        )
        .unwrap();
        let out = format_zones(&cfg);
        assert!(out.contains("read   /private/"));
        assert!(out.contains("titan  /uploads/"));
        assert!(out.contains("1 fingerprint(s)"));
    }

    #[test]
    fn format_roster_of_an_empty_roster_says_so() {
        let cfg = Config::from_toml_str("", &crate::config::EnvOverrides::default()).unwrap();
        let out = format_roster(&cfg);
        assert!(out.contains("no [[identity]] entries"));
    }

    #[test]
    fn format_status_combines_every_section() {
        let dir = tmpdir("status-combine");
        let cfg = Config::from_toml_str(
            "[[host]]\nname = \"a.example\"\n",
            &crate::config::EnvOverrides::default(),
        )
        .unwrap();
        let store = IdentityStore::open(&dir, &["a.example".to_string()]).unwrap();
        let published = PublishedStats::default();
        let out = format_status(&cfg, &store, &published);
        assert!(out.contains("== capsule =="));
        assert!(out.contains("== server fingerprints =="));
        assert!(out.contains("== identity roster =="));
        assert!(out.contains("== zones =="));
        assert!(out.contains("== published =="));
        assert!(out.contains("a.example"));
        assert!(out.contains("advertised host: (default: first configured host)"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_status_shows_an_explicit_advertised_host() {
        let dir = tmpdir("status-advertised-host");
        let cfg = Config::from_toml_str(
            "[server]\nadvertised_host = \"mirror.example\"\n[[host]]\nname = \"a.example\"\n",
            &crate::config::EnvOverrides::default(),
        )
        .unwrap();
        let store = IdentityStore::open(&dir, &["a.example".to_string()]).unwrap();
        let published = PublishedStats::default();
        let out = format_status(&cfg, &store, &published);
        assert!(out.contains("advertised host: mirror.example"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_roster_lists_labels_capabilities_and_rotation() {
        let hex_a = "a".repeat(64);
        let hex_b = "b".repeat(64);
        let cfg = Config::from_toml_str(
            &format!(
                "[[identity]]\nlabel = \"scribe\"\nfingerprint = \"{hex_a}\"\n\
                 capabilities = [\"titan-write\"]\n\
                 superseded = [\"{hex_b}\"]\nsuperseded_until = \"2099-01-01\"\n"
            ),
            &crate::config::EnvOverrides::default(),
        )
        .unwrap();
        let out = format_roster(&cfg);
        assert!(out.contains("scribe"));
        assert!(out.contains(&hex_a));
        assert!(out.contains("titan-write"));
        assert!(out.contains("rotating"));
        assert!(out.contains("2099-01-01"));
    }

    // --- usv export ---

    #[tokio::test]
    async fn export_copies_every_file_preserving_structure() {
        let base = tmpdir("export-basic");
        let html = base.join("html");
        std::fs::create_dir_all(html.join("notes")).unwrap();
        std::fs::write(html.join("index.html"), "<html>home</html>").unwrap();
        std::fs::write(html.join("style.css"), "body{}").unwrap();
        std::fs::write(html.join("notes/a.html"), "<html>a</html>").unwrap();
        let dest = base.join("export-out");

        let count = export_html_tree(&base, &dest).await.unwrap();
        assert_eq!(count, 3);
        assert_eq!(
            std::fs::read_to_string(dest.join("index.html")).unwrap(),
            "<html>home</html>"
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("notes/a.html")).unwrap(),
            "<html>a</html>"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn export_without_a_render_yet_is_a_named_error() {
        let base = tmpdir("export-norender");
        let dest = base.join("out");
        let err = export_html_tree(&base, &dest).await.unwrap_err();
        assert!(matches!(err, ExportError::NothingRendered));
        assert!(!dest.exists(), "must not create the destination on failure");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn export_refuses_a_non_empty_destination() {
        let base = tmpdir("export-nonempty-dest");
        std::fs::create_dir_all(base.join("html")).unwrap();
        std::fs::write(base.join("html/index.html"), "x").unwrap();
        let dest = base.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("already-here.txt"), "pre-existing").unwrap();

        let err = export_html_tree(&base, &dest).await.unwrap_err();
        assert!(matches!(err, ExportError::DestinationNotEmpty(_)));
        // The pre-existing file must survive untouched.
        assert_eq!(
            std::fs::read_to_string(dest.join("already-here.txt")).unwrap(),
            "pre-existing"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn export_refuses_a_destination_that_is_a_plain_file() {
        let base = tmpdir("export-dest-is-file");
        std::fs::create_dir_all(base.join("html")).unwrap();
        std::fs::write(base.join("html/index.html"), "x").unwrap();
        let dest = base.join("out");
        std::fs::write(&dest, "not a directory").unwrap();

        let err = export_html_tree(&base, &dest).await.unwrap_err();
        assert!(matches!(err, ExportError::DestinationNotEmpty(_)));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn export_into_an_existing_empty_directory_succeeds() {
        let base = tmpdir("export-empty-dest");
        std::fs::create_dir_all(base.join("html")).unwrap();
        std::fs::write(base.join("html/index.html"), "hi").unwrap();
        let dest = base.join("out");
        std::fs::create_dir_all(&dest).unwrap(); // exists, but empty

        let count = export_html_tree(&base, &dest).await.unwrap();
        assert_eq!(count, 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn export_never_renders_anything_new() {
        // The same read-only-of-what's-published discipline `stats` and
        // `inspect_published` follow: export must not write into
        // state_dir/html itself, only read from it.
        let base = tmpdir("export-readonly-source");
        let html = base.join("html");
        std::fs::create_dir_all(&html).unwrap();
        std::fs::write(html.join("index.html"), "original").unwrap();
        let before: Vec<_> = std::fs::read_dir(&html)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();

        let dest = base.join("out");
        export_html_tree(&base, &dest).await.unwrap();

        let after: Vec<_> = std::fs::read_dir(&html)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(before, after, "export must not modify the source tree");
        let _ = std::fs::remove_dir_all(&base);
    }
}
