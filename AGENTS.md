# AGENTS.md — operating manual for maintainers (AI or human)

Unseen Servant (`usv`): a security-first Gemini server in Rust that publishes
one content tree to Geminispace and the web. AI-authored under the AI Forward
principle — the AI writes and proves, the director (n) reviews and directs.
This file is the entry point; it must always be enough to start work without
archaeology.

## Read order

| # | Document | Why |
|---|---|---|
| 1 | `docs/BRIEF.md` | The founding directive, verbatim, with amendments |
| 2 | `docs/adr/README.md` | Every decision, its status, its evidence |
| 3 | `docs/ROADMAP.md` | What ships in which release; the announcement gate |
| 4 | `docs/BUILD-PLAN.md` | Phases C0–C7, exit gates, proving-grounds protocol |
| 5 | `docs/recon/README.md` | Phase 0 research synthesis (8 documents, all dated) |
| 6 | `docs/OPEN-QUESTIONS.md` | What awaits the director; never re-ask answered ones |
| 7 | `docs/notes/integration-ideas.md` | Design notes not yet hardened into ADRs |

## Phase state

**C1 (wire core) exit gate passed — next: C2 (static serving).** Config,
protocol layers, identity, TLS, listener all land with 81 tests green;
gemini-diagnostics 26/27 clean (27th is a verified tool bug under Python
≥3.13, see DEBUGGING.md); fuzz targets (frame_request_line, validate_uri,
config_parse) wired into CI alongside cargo-deny. Update this line when a
phase's exit gate passes; the gates are defined in `docs/BUILD-PLAN.md`.

## Invariants (violating any of these needs an ADR amendment first)

- `unsafe_code = "forbid"` — the ADR 0002 security argument rests on it.
- Every parser is fuzzed (`fuzz/`); a new parser lands with its fuzz target
  in the same change.
- The gemini-diagnostics suite (all 27 checks, enumerated in
  `docs/recon/prior-art.md` §6) must pass clean before ANY public exposure.
  Hard gate, not advice.
- **Nothing is announced, linked, or listed publicly before v1.0 passes its
  gates** (`docs/ROADMAP.md` M6). This includes forum posts, store
  submissions, and awesome-list PRs.
- ADR discipline: decisions are recorded before or with their code; director
  defaults are never overturned silently; superseded ADRs are amended in
  place, never renumbered or deleted.
- One TOML config file (ADR 0007). Unknown keys are startup errors.
- Interaction features follow dynamic-write/static-read (ADR 0009); usv
  never executes content (ADR 0005 — CGI refused permanently).
- Keys and certificates: never silently regenerated (ADR 0003); private-key
  material is only ever held by the `identity` module.
- Commits: `<area>: <what>` subject, body explains why. **No
  Co-Authored-By or AI-attribution trailers — house policy** (the AI
  Forward authorship story lives in the docs, not in commit trailers).
  Push to Forgejo origin.

## Commands

```sh
cargo test                                   # unit + binary smoke tests
cargo clippy --all-targets -- -D warnings    # warnings are errors
cargo fmt --check
cargo +nightly fuzz run frame_request_line   # needs cargo-fuzz; see fuzz/Cargo.toml
```

## House context (the estate)

This repo lives INSIDE the Cloudron packaging estate at
`…/Code/cloudron/packages/unseen-servant/unseen-servant/`. Estate facts that
bear on this project:

- The workspace folder above this repo is for round material (phase notes,
  drafts) per the estate naming rule; the repo itself stays publishable. A
  sibling `unseen-servant-cloudron/` packaging repo may be created at C6 —
  that decision is recorded in `docs/OPEN-QUESTIONS.md` territory, not made
  silently.
- Estate doctrine root: `../../../estate/` (README, `doctrine/`, `starter/`).
  Round workflow: `starter/START-HERE.md`; experiments go in
  `doctrine/cloudron-packaging-experiments.md` — named, vivid, never left in
  scratchpads.
- Hosts: `the build workstation` = build workstation (this machine; hosts the Proving
  Ground VM — labs Cloudron, `a private address`, RFC1918, never
  public). `production` = production Cloudron (~69 apps, load-saturated —
  **gates run on the proving grounds, never on production**; doctrine:
  `proving-ground-design.md`). Both are known to the `cloudron` CLI.
- Secrets: the `secret` helper reads OpenBao (folders: github, forgejo, ai,
  apps, dns, infra); capability matrix at
  `../../meilisearch/phase-notes/credential-capability-inventory.md`.
  GitHub mirror pushes: `GH_TOKEN_ORCVOLE` (repo+workflow).
- Base image truth: `cloudron/base:5.1.0` per live docs (2026-08-09). The
  estate skeleton and the global skill both still pin 5.0.0 — this repo's
  recon (`docs/recon/cloudron-fit.md`) overrides them.
- Test domain offered by the director: `usv.wanderingmonster.dev`.

## Session mechanics

This file is the ONLY agent-facing instruction file — house policy is
AGENTS.md, never CLAUDE.md.

- Director preferences: explain jargon in plain words on first use;
  Lagrange is the reference client; record directives in the repo docs, not
  just in replies; check `docs/OPEN-QUESTIONS.md` before asking anything.
- Model/effort tiering (estate finding: effort dominates token cost more
  than model choice): security-sensitive parsing/TLS/ADR work wants a
  frontier model at high effort; mechanical phases (boilerplate, themes,
  doc formatting, packaging plumbing) run fine at lower effort or a smaller
  model. Max effort only for genuinely hard, novel decisions.
- The repo was relocated into the estate on 2026-08-09; sessions started
  before the move have an invalidated cwd — anchor shell commands with
  absolute paths.

## Layout

| Path | Holds |
|---|---|
| `src/` | The crate (module map in `src/lib.rs`) |
| `tests/` | Integration/regress tests (real binary, real sockets from C1) |
| `fuzz/` | cargo-fuzz workspace, one target per parser |
| `docs/` | Brief, ADRs, roadmap, build plan, recon, notes |
| `.forgejo/workflows/` | CI gates (runner facts documented in the file) |
