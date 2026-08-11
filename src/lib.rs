//! Unseen Servant — the library behind the `usv` binary.
//!
//! A security-first [Gemini protocol](https://geminiprotocol.net/) server that
//! publishes one content tree to Geminispace (gemtext, port 1965) and the web
//! (statically rendered HTML). Cloudron is a first-class deployment profile,
//! never a requirement (ADR 0008).
//!
//! # Read order for maintainers (human or AI)
//!
//! This codebase is AI-authored and optimised for machine legibility: small
//! modules, exhaustive doc comments, and an ADR for every non-obvious decision.
//! Orientation path: `AGENTS.md` → `docs/internal/BRIEF.md` → `docs/adr/README.md` →
//! `docs/internal/ROADMAP.md` → `docs/internal/BUILD-PLAN.md`. Nothing in this crate should
//! surprise a reader who has walked that path; if it does, that is a
//! documentation bug — fix the documents, not just the surprise.
//!
//! # Build-phase state
//!
//! C1-C5 of `docs/internal/BUILD-PLAN.md` are underway; the module map below is the
//! intended shape, updated as each module lands:
//!
//! | Module | Phase | Owns |
//! |---|---|---|
//! | [`protocol`] | C1 | wire framing, URI validation, authority check, response emission |
//! | [`config`] | C1 | the single TOML file (ADR 0007) |
//! | [`identity`] | C1 | keys and certificates (ADR 0003) — sole holder of key material |
//! | [`tls`] | C1 | rustls server policy: versions, SNI, client-cert capture, no tickets |
//! | [`server`] | C1 | listener, per-connection lifecycle, timeouts, drain |
//! | [`handler`] | C2 | the request → response trait and its implementations (ADR 0005); Titan uploads (ADR 0006) land here in C4 |
//! | [`render`] | C3 | gemtext → HTML/Atom/feeds pipeline (ADR 0004) |
//! | [`roster`] | C4 | client-cert fingerprint → capability lookups (ADR 0011) |
//! | [`runtime_state`] | C5 | in-process state that must survive a SIGHUP config reload: activity log, last render snapshot |
//! | [`cli`] | C5 | business logic behind every `usv` subcommand except `init`'s interactive wizard |
//! | [`init`] | C5 | `usv init`'s validation and file-writing — the wizard's own event loop lives in the binary |
//! | [`json`] | C7 | JSON emission for the `--json` CLI output (`docs/agents.md`) |

pub mod cli;
pub mod config;
pub mod handler;
pub mod http;
pub mod identity;
pub mod init;
pub mod json;
pub mod plaintext;
pub mod protocol;
pub mod render;
pub mod roster;
pub mod runtime_state;
pub mod server;
pub mod tls;
