# Debugging

Grows with each build phase; a section that says "arrives at Cx" is a
placeholder on purpose, not an omission.

## Now (C0)

- `usv --version` / `usv --help` are the only behaviors. Smoke tests:
  `cargo test --test smoke`.
- Logging: `tracing` arrives in C1. From then on the server logs to
  stdout/stderr only, controlled by `RUST_LOG` (e.g. `RUST_LOG=usv=debug`);
  request logs are single-line and query-redacted by default.

## Arrives at C1–C2

- Reading a rejection: every 59/53 maps to a layer (framing → URI →
  authority; `src/protocol/mod.rs` documents the layering). The log line
  names the layer and the `FramingError`/validation variant.
- Certificate problems: `usv fingerprint` (C5) prints what's on disk; until
  then `openssl x509 -in <state>/certs/<host>/cert.pem -noout -fingerprint
  -sha256`.
- The regress suite (`cargo test`) spawns the real binary on a loopback
  port; failures print the exact wire bytes exchanged.
- gemini-diagnostics: run `gemini-diagnostics <host> <port>` against a local
  instance; all 27 checks and their meanings are enumerated in
  `docs/recon/prior-art.md` §6.

## Cloudron (arrives at C6)

Per `docs/recon/cloudron-fit.md`: `cloudron logs -f` streams the app;
`cloudron exec` opens a shell; the panel's file manager edits
`/app/data`. A "what each panel screen means for usv" table lands here with
the package. Until then that recon document is the reference.

## The disabled-port state (permanent fact)

If `GEMINI_PORT` is absent (Cloudron admin disabled the TCP port), usv runs
the HTTP surface only and stays healthy — by design (ADR 0008 /
cloudron-fit hard constraints). "Gemini unreachable but app green" is that
state, not a bug; the status page and logs both say so explicitly.
