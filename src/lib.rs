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
//! Orientation path: `AGENTS.md` → `docs/BRIEF.md` → `docs/adr/README.md` →
//! `docs/ROADMAP.md` → `docs/BUILD-PLAN.md`. Nothing in this crate should
//! surprise a reader who has walked that path; if it does, that is a
//! documentation bug — fix the documents, not just the surprise.
//!
//! # Build-phase state
//!
//! Phase C0 (scaffold): only [`protocol`] exists, and only its framing layer.
//! The module map below is the intended shape; modules appear as their phase
//! arrives (docs/BUILD-PLAN.md):
//!
//! | Module | Phase | Owns |
//! |---|---|---|
//! | [`protocol`] | C1 | wire framing, URI validation, response emission |
//! | `config` | C1 | the single TOML file (ADR 0007) |
//! | `identity` | C1 | keys and certificates (ADR 0003) — sole holder of key material |
//! | `handler` | C2 | the request → response trait and its implementations (ADR 0005) |
//! | `render` | C3 | gemtext → HTML/Atom/feeds pipeline (ADR 0004) |
//! | `titan` | C4 | uploads (ADR 0006) |

pub mod protocol;
