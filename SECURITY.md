# Security policy

## Reporting a vulnerability

**Please do not open a public issue.**

Email **<most+claude@alba.win>** with `unseen-servant` in the subject line.
Include what you did, what happened, and what you expected. A proof of
concept helps but is not required to make contact.

You will get an acknowledgement. If a report turns out to be a real
issue, you will be credited in the fix unless you would rather not be.

## Supported versions

`usv` is **pre-1.0 and pre-release**. There are no tagged releases, no
published packages, and no announcement yet. Only `main` is supported;
fixes land there.

This will be replaced with a real support table when v1.0 ships.

## Scope

In scope: anything reachable over the Gemini, Titan, or web-mirror
surfaces: request parsing, TLS handling, path traversal, certificate
zone and Titan authorisation, identity handling, the render pipeline.
Also in scope: the packaging (a `.deb`/RPM/AUR/Nix/container package that
installs something unsafe is a security bug).

Out of scope, because they are documented behaviour rather than defects
, see [`docs/security.md`](docs/security.md) for the reasoning:

- The web mirror publishes rendered content to anyone. Certificate zones
  gate the *Gemini* surface only.
- A writable Titan zone is writable by every fingerprint listed in it.
- Client certificates prove continuity, not who someone is.
- Visitor addresses are **not** logged by default (`server.log_peer`
  defaults to `off`). An operator who opts into `full` has chosen a
  conventional access log; that is configuration, not a defect.

## What we do already

Details in [`docs/security.md`](docs/security.md): `unsafe_code =
"forbid"` project-wide, seven fuzzed parsers with committed regression
corpora, `cargo deny check` on every push, no dynamic execution anywhere
by design, a single unprivileged process with an empty capability
bounding set, and TOFU identity that is never silently regenerated.

`usv` has **not** been independently audited.
