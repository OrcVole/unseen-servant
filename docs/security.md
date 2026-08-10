# Security

`usv` is a network-facing service that terminates its own TLS. This page
describes what it does to earn trust, and — as importantly — what it
does not do.

## Reporting a vulnerability

Please do not open a public issue. Email **most+claude@alba.win** with
"unseen-servant" in the subject. See [`../SECURITY.md`](../SECURITY.md)
for supported versions and disclosure handling.

`usv` is pre-1.0 and has not been independently audited.

## The posture

**Memory safety by construction.** `unsafe_code = "forbid"` is set
project-wide in `Cargo.toml`, so the crate cannot contain an `unsafe`
block — not "we avoid it", the compiler refuses it.

**Every parser is fuzzed.** Seven `cargo-fuzz` targets cover the request
line, URI validation, config parsing, static path sanitisation, gemtext,
HTML rendering, and Titan framing. A bounded 60s-per-target smoke run
gates every push; extended campaigns run to hundreds of millions of
executions per target (most recently: 327M on `validate_uri`, 309M on
`static_path_sanitize`, zero crashes) with the minimised corpus committed
as regression material.

**No dynamic execution, anywhere.** No CGI, no FastCGI, no SCGI, no
scripting, no reverse-proxying, no plugin API (ADR 0005). Content is
data, never code. Whole classes of vulnerability are absent because the
feature that carries them was refused.

**Single unprivileged process.** ADR 0002 chose one memory-safe process
over the multi-process privilege separation C servers need, and pays that
back through systemd hardening: `ProtectSystem=strict`, an *empty*
`CapabilityBoundingSet` (both ports are >1024, so no capability is
needed at all), `NoNewPrivileges`, `MemoryDenyWriteExecute`. See
[`deployment/source.md`](deployment/source.md).

**Supply chain.** `cargo deny check` gates every push: advisories fail
the build, yanked crates are denied, licences must be on an allowlist,
and crates.io is the only permitted source. Dependencies are pinned via
`Cargo.lock`, and every packaging format ships one binary built from it.

**Path traversal.** The static handler canonicalises and confines every
resolved path to the docroot, with a dedicated fuzz target and a
regression corpus of percent-encoded, double-encoded, backslash and
NUL-injected attempts — because the community `gemini-diagnostics` suite's
own traversal check has a documented false negative, and passing it
proves very little.

**Uploads are closed by default.** Titan zones require an explicit
fingerprint or identity allowlist; an empty one is a *startup error*, not
"anyone" ([`configuration.md`](configuration.md)). Size is validated
against the declared length before the body is read, with a hard cap.

**Non-TLS probes get nothing.** The first byte is peeked before the TLS
acceptor runs; anything that is not a handshake record is dropped with no
response at all. This was a real fix — rustls was writing an alert record
before aborting, which confirmed a TLS stack was listening.

## Identity is TOFU, and that has consequences

The certificate is minted once per hostname and never silently
regenerated — not on restart, update, restore, or migration. Corrupt or
half-present key material is a loud error, never a quiet regeneration,
because silently minting a new key is indistinguishable from an
impersonation to a client that pinned the old one.

Expiry is set to 4096-01-01, following Agate. Under TOFU, CA-style expiry
adds churn without adding security.

A hostname change is detected and gets a *fresh* identity rather than
reusing the old one on a new name.

## What is logged — read this before deploying

One line per request, at `info`: **the client's IP address**, the
response status, and the request path.

The query string is redacted by construction — the log line is built
from the path only, never the query — because Gemini's status `10`/`11`
input flow puts user-typed text in the query, including passwords.

**The peer IP is not redacted.** That is a deliberate operational
default, but it sits against a strong community norm: Geminispace
operators frequently make a point of *not* retaining visitor IPs, and
some map them to ephemeral identifiers discarded within the hour. If you
run a capsule where that matters, set the log level to `warn` (which
drops the per-request line entirely) or filter at the collector. There
is currently **no built-in option to log requests without the IP**;
`docs/OPEN-QUESTIONS.md` carries this as a decision to make before v1.0.

`usv` writes only to stdout/stderr and keeps no log files.

## What `usv` deliberately does not protect you from

- **A writable Titan zone is a writable zone.** Anyone holding a listed
  fingerprint can change your capsule's content.
- **Client certificates are identity, not attestation.** The roster
  records *when* a key was enrolled, never who holds it. A fingerprint
  proves continuity with a previous visitor, nothing more.
- **The web mirror is public.** Certificate zones gate the *Gemini*
  surface. Content rendered to HTML is served to anyone who asks. Do not
  put a cert-gated path's content where the HTML renderer will publish it.
- **It is not a web server.** No HTTP authentication, no HTTP access
  control, no request-time logic of any kind.
