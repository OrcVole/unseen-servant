# Contributing

Unseen Servant is developed **AI Forward**: the AI is the primary author of
code, configuration, documentation, and maintenance; a human director reviews
and directs. This shapes what contribution means here.

## Before v1.0

The project is pre-release and unannounced. External contributions are not
being solicited yet; if you found this repository, welcome — please don't
publicise it (see `docs/ROADMAP.md` for why the announcement is gated).

## What always applies

- **Decisions live in ADRs** (`docs/adr/`). A change that contradicts an
  accepted ADR needs an ADR amendment in the same change — never a silent
  overturn. New non-obvious decisions get new ADRs.
- **Invariants** are listed in `AGENTS.md` and are not up for casual
  renegotiation: no unsafe code, every parser fuzzed, the
  gemini-diagnostics gate, dynamic-write/static-read for interaction, no
  content execution.
- **Tests arrive with the change**, in the same commit: unit tests beside
  the module, regress coverage where behavior is wire-visible, a fuzz
  target for any new parser.
- **Docs are part of the change.** If behavior moves, the relevant document
  (ADR, DEBUGGING.md, UPGRADING.md) moves in the same commit. The codebase
  must stay legible to a fresh session without archaeology.
- Style: `cargo fmt`, clippy clean at `-D warnings`, commit subjects as
  `<area>: <what>` with a why-focused body.

## Licensing

MIT (see `LICENSE`). By contributing you agree your contribution is
MIT-licensed. Where mechanisms are adapted from studied prior art (Agate,
gmid, Molly Brown — see `docs/recon/prior-art.md`), attribution lives in the
relevant module's doc comment.
