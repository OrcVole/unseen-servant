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

### C1 exit gate: gemini-diagnostics run (2026-08-09)

26/27 clean against a local instance (Python 3.14.6, tool from
`michael-lazar/gemini-diagnostics`, commit at time of run). Two items are not
usv defects; recorded here so a future run isn't re-litigated:

- **`TLSClaims` fails on this tool under Python ≥3.13**: the script calls
  `ssl.match_hostname`, removed from the stdlib in 3.13. The certificate
  claims it's trying to check (notBefore/notAfter, CN/SAN) were verified
  independently via `openssl x509 -noout -subject -dates -ext
  subjectAltName` and a modern `ssl.SSLContext` — both confirm the cert is
  correct. Not actionable on our side; re-check if the tool patches this.
- **`URLByIPAddress` reports `None`, not a failure.** The check's own
  design accepts either serving the request or refusing with 53; we refuse
  (the request's authority doesn't match a configured host), which is
  spec-legitimate. The tool reports the choice, not a verdict.
- **Real defect found and fixed**: `TLSRequired` initially failed because
  rustls writes a TLS alert record to the socket before aborting a garbled
  handshake — a real response, just not a Gemini one, and worse than
  silence (confirms a TLS stack lives on the port before ever refusing the
  request). Fixed in `src/server.rs` (`peek_looks_like_tls`): the first
  byte is peeked before the TLS acceptor runs; anything that isn't a TLS
  handshake record (0x16) is dropped with no response at all.

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
