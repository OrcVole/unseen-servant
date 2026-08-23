---
title: "Security"
description: "Usv is a network-facing service that terminates its own TLS (Transport Layer Security). This page describes what it does, and what it does not do."
type: reference
status: decided
last_verified: 2026-08-11
---

# Security

> Unseen Servant

`usv` is a network-facing service that terminates its own TLS (Transport
Layer Security). This page describes what it does, and what it does not do.

## Why the posture is shaped this way

A capsule is usually run by one person, on a machine they also use for other
things, and then left alone for months. The threat that matters is not a
targeted attack; it is the slow accumulation of things that can go wrong
unattended: an unpatched interpreter, a forgotten admin password, a
permissive default nobody revisited.

So the design removes categories rather than defending them. Almost every
decision below follows from three choices:

1. **Nothing is executed.** The one feature every server we studied
   regretted was the escape hatch beyond static serving; it produced most of
   their defect load. `usv` has none.
2. **There is nothing to log into.** No administrative web interface means
   no credential, no session, and no login page to find.
3. **Ambiguity fails closed.** An unknown configuration key, an empty upload
   allowlist, a mistyped logging mode: each is a startup error, because a
   setting that fails open is worse than one that fails loudly.

## Reporting a vulnerability

Please do not open a public issue. Email **<most+claude@alba.win>** with
"unseen-servant" in the subject. See [`../SECURITY.md`](../SECURITY.md) for
supported versions and disclosure handling.

`usv` is pre-1.0 and has not been independently audited.

## The posture

**Memory safety by construction.** `unsafe_code = "forbid"` is set
project-wide in `Cargo.toml`, so the crate cannot contain an `unsafe` block
, not "we avoid it", the compiler refuses it.

**Every parser is fuzzed.** Eight `cargo-fuzz` targets cover the request
line, URI (uniform resource identifier) validation, config parsing, static
path sanitisation, gemtext, HTML (HyperText Markup Language) rendering,
Titan framing, and gopher selectors. A bounded 60s-per-target smoke run
gates every push; extended campaigns run to hundreds of millions of
executions per target (most recently: 327M on `validate_uri`, 309M on
`static_path_sanitize`, zero crashes) with the minimised corpus committed as
regression material.

**No dynamic execution, anywhere.** No CGI (Common Gateway Interface), no
FastCGI, no SCGI (Simple Common Gateway Interface), no scripting, no
reverse-proxying, no plugin interface (ADR 0005, an architecture decision
record). Content is data, never code. Whole classes of vulnerability are
absent because the feature that carries them was refused.

**Single unprivileged process.** ADR 0002 chose one memory-safe process over
the multi-process privilege separation C servers need, and pays that back
through systemd hardening: `ProtectSystem=strict`, an *empty*
`CapabilityBoundingSet` (every listener defaults to a port above 1024, so no
capability is needed at all), `NoNewPrivileges`, `MemoryDenyWriteExecute`.
See [`deployment/source.md`](deployment/source.md).

**Supply chain.** `cargo deny check` gates every push: advisories fail the
build, yanked crates are denied, licences must be on an allowlist, and
crates.io is the only permitted source. Dependencies are pinned via
`Cargo.lock`, and every packaging format ships one binary built from it.

**Path traversal.** The static handler canonicalises and confines every
resolved path to the docroot, with a dedicated fuzz target and a regression
corpus of percent-encoded, double-encoded, backslash and NUL-injected
attempts, because the community `gemini-diagnostics` suite's own traversal
check has a documented false negative, and passing it proves very little.

**Uploads are closed by default.** Titan zones require an explicit
fingerprint or identity allowlist; an empty one is a *startup error*, not
"anyone" ([`configuration.md`](configuration.md)). Size is validated against
the declared length before the body is read, with a hard cap.

**Non-TLS probes get nothing.** The first byte is peeked before the TLS
acceptor runs; anything that is not a handshake record is dropped with no
response at all. This was a real fix: rustls was writing an alert record
before aborting, which confirmed a TLS stack was listening.

**Cleartext protocols are walled off structurally.** Gopher, Spartan, Nex
and Finger cannot authenticate a client at all, so content behind a
certificate zone or a Titan zone is excluded from their trees at the point
the tree is built, not by a check somewhere that has to remember. A
configuration that would publish gated content over one of them is a startup
error ([`smolnets.md`](smolnets.md)).

## Identity is TOFU, and that has consequences

The small internet uses TOFU (trust on first use): a reader's client pins
your certificate the first time it connects, and warns only if that
certificate later changes: the same model as SSH.

The certificate is minted once per hostname and never silently regenerated,
not on restart, update, restore, or migration. Corrupt or half-present key
material is a loud error, never a quiet regeneration, because silently
minting a new key is indistinguishable from an impersonation to a client
that pinned the old one.

Expiry is set to 4096-01-01, following Agate. Under TOFU, the expiry dates a
CA (certificate authority) issues add churn without adding security.

A hostname change is detected and gets a *fresh* identity rather than
reusing the old one on a new name.

## What is logged

One line per request, at `info`: the response status, the request path, and
a peer field whose contents you choose.

**Visitor addresses are not logged by default.** The peer field renders as
`-` unless you ask for more. Geminispace's norm is aggressive log minimalism
: operators routinely make a point of not retaining visitor addresses, and
the default follows that rather than the habit inherited from web servers.

`server.log_peer` takes three values:

| Value | Logs |
|---|---|
| `"off"` *(default)* | `-`. Nothing identifying at all. |
| `"hashed"` | A 48-bit digest of the address under a salt generated fresh at every start and never persisted. Repeat visits correlate within one run of the process; nothing survives a restart. |
| `"full"` | The address verbatim, for a conventional access log. |

The hashed mode digests the **address only**, never the ephemeral source
port: including the port would make every request from one visitor look
like a different visitor, which is the whole thing the mode exists to
provide.

**The query string is redacted by construction** in every mode. The log line
is built from the path alone, because Gemini's status `10`/`11` input flow
puts user-typed text in the query, up to and including passwords.

A mistyped `log_peer` value is a startup error, not a warning: failing open
would silently keep addresses an operator believed they had turned off.

`usv` writes only to stdout/stderr and keeps no log files. If you do enable
`full`, note that your platform's journal typically retains it for weeks
even though `usv` itself retains nothing.

## What `usv` deliberately does not protect you from

- **A writable Titan zone is a writable zone.** Anyone holding a listed
  fingerprint can change your capsule's content.
- **Client certificates are identity, not attestation.** The roster records
  *when* a key was enrolled, never who holds it. A fingerprint proves
  continuity with a previous visitor, nothing more.
- **The web mirror is public.** Certificate zones gate the *Gemini* surface.
  Content rendered to HTML is served to anyone who asks. Do not put a
  cert-gated path's content where the HTML renderer will publish it.
- **It is not a web server.** No HTTP (HyperText Transfer Protocol)
  authentication, no HTTP access control, no request-time logic of any kind.
