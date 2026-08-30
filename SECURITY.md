# Security policy

## Reporting a vulnerability

**Please do not open a public issue.**

Email **<most+claude@alba.win>** with `unseen-servant` in the subject line.
Include what you did, what happened, and what you expected. A proof of
concept helps but is not required to make contact.

You will get an acknowledgement. If a report turns out to be a real
issue, you will be credited in the fix unless you would rather not be.

## Supported versions

| Version | Supported |
|---|---|
| 1.0.x | yes |
| < 1.0 | there was no < 1.0; v1.0.0 is the first release |

Fixes land on `main` and ship in the next patch release. `usv` follows
semantic versioning: configuration semantics and the certificate
lifecycle can only change in a MAJOR release, and `UPGRADING.md` states
what that promise covers.

## Verifying a release

Release artefacts are signed with minisign. The public key is
[`assets/unseen-servant.pub`](assets/unseen-servant.pub) in this
repository, and its raw value is:

```text
RWQ8yH+Afj6YnCB5dOP+vbhvFT6DQhHBzmkC5oAY9gIrv0+vyAP+qQAw
```

```sh
minisign -Vm SHA256SUMS -P RWQ8yH+Afj6YnCB5dOP+vbhvFT6DQhHBzmkC5oAY9gIrv0+vyAP+qQAw
sha256sum -c SHA256SUMS
```

`rsign` (the Rust implementation) verifies the same signatures:
`rsign verify -P <key> -x SHA256SUMS.minisig SHA256SUMS`.

A key published in the repository it signs proves less than one you
obtained separately: if you can, note the key on first download and
check it has not changed on the next, which is the same trust-on-first-use
reasoning `usv` applies to its own certificates.

## Scope

In scope: anything reachable over the Gemini, Titan, or web-mirror
surfaces: request parsing, TLS handling, path traversal, certificate
zone and Titan authorisation, identity handling, the render pipeline.
Also in scope, since they shipped: the four cleartext listeners
(Gopher, Spartan, Nex, Finger), their parsers, and above all the rule
that certificate-gated or Titan-gated content must never reach a
cleartext tree (ADR 0012 §6). A gated file appearing on any cleartext
surface is a security bug, not a rendering bug.
Also in scope: the packaging (a `.deb`/RPM/AUR/Nix/container package that
installs something unsafe is a security bug).

Out of scope, because they are documented behaviour rather than defects
, see [`docs/security.md`](docs/security.md) for the reasoning:

- The web mirror publishes rendered content to anyone. Certificate zones
  gate the *Gemini* surface only.
- A writable Titan zone is writable by every fingerprint listed in it.
- Gopher, Spartan, Nex and Finger are cleartext by design: no
  confidentiality, no server or client authentication. They are off by
  default and documented as such.
- Client certificates prove continuity, not who someone is.
- Visitor addresses are **not** logged by default (`server.log_peer`
  defaults to `off`). An operator who opts into `full` has chosen a
  conventional access log; that is configuration, not a defect.

## What we do already

Details in [`docs/security.md`](docs/security.md): `unsafe_code =
"forbid"` project-wide, eight fuzzed parsers with committed regression
corpora, `cargo deny check` on every push, no dynamic execution anywhere
by design, a single unprivileged process with an empty capability
bounding set, and TOFU identity that is never silently regenerated.

`usv` has **not** been independently audited.
