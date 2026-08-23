---
title: "ADR 0002: Process and privilege model: single process, capability-shaped module boundaries"
description: "Single process, multiple tokio tasks, capability-shaped module boundaries:."
type: explanation
status: decided
last_verified: 2026-08-11
---

# ADR 0002: Process and privilege model: single process, capability-shaped module boundaries

- Status: Accepted (pending director review)
- Date: 2026-08-09
- Evidence: docs/internal/recon/prior-art.md §2, docs/internal/recon/cloudron-fit.md §6

## Context

gmid, the field's security high-water mark, runs four OS processes
(main / logger / server / crypto) over imsg pipes so a compromised
request-parsing process cannot reach private keys or the log stream.
The brief's default is a single process, with keys loaded into a
dedicated task that has no filesystem write access after startup, and
asks this ADR to document why full privsep is or is not worth it.

What gmid's privsep defends against, and what replaces each defense
here:

| gmid defense | Threat | Our replacement |
|---|---|---|
| server process sandboxed (pledge/unveil) | parser memory-corruption → arbitrary code | Rust memory safety + `forbid(unsafe_code)` + fuzzing (ADR 0001); container seccomp + read-only rootfs as outer wall |
| crypto process holds keys | key exfiltration from compromised server process | keys enter rustls's `ServerConfig`/private-key provider at startup and are never held as raw bytes in request-handling scope; the type system, not a process boundary, denies access |
| logger process | log-stream tampering, privileged file writes | logs go only to stdout/stderr (Cloudron contract) or the journal; one `tracing` subscriber owns output |
| privileged main process | binding low ports, re-reading certs on SIGHUP | container/platform grants the port; reload is a task re-reading paths the process could already read |

A second full process tree inside a container also creates the PID-1
signal-forwarding and supervision problems that earned Agate+ its
Cloudron review objections (cloudron-fit.md §6-7); a single binary
sidesteps that entire review class. Standalone deployments (ADR 0008)
get the same benefit: one process is trivially run under systemd,
runit, or a plain shell.

## Decision

**Single process, multiple tokio tasks, capability-shaped module
boundaries:**

- A `listener` task per surface (Gemini :1965, HTTP) accepting
  connections; per-connection tasks with hard timeouts (header read,
  body write, idle) and a concurrent-connection cap.
- An `identity` module that loads/generates keys (ADR 0003) and hands
  rustls the resolver; no other module can name a private-key type.
  After startup the process needs no write access outside its content/
  state directories, and drops none of this to configuration.
- A `render` task owning the gemtext→HTML pipeline (ADR 0004), the
  only writer to the HTML output tree.
- A `control` task owning signals: SIGHUP = reload config + certs
  without dropping listeners (gmid's reload discipline); SIGTERM =
  graceful drain then exit (Agate had to retrofit this; we ship it in
  v1: Cloudron and systemd both stop with SIGTERM).
- One `tracing` subscriber writing to stdout/stderr; no log files.

Full gmid-style multi-process privsep is **refused** for v1: in a
memory-safe language inside a container (or under a hardened systemd
unit standalone), each process boundary buys marginal defense at real
supervision cost. The *goals* are kept; the *mechanism* is the type
system and the platform sandbox.

## Consequences

- The security argument rests on Rust's guarantees plus the fuzz gate;
  therefore `forbid(unsafe_code)` and parser fuzzing are load-bearing,
  not optional hygiene.
- Module boundaries must be enforced in review: any PR that moves raw
  key bytes or filesystem writes into request-handling scope violates
  this ADR by definition.
- For standalone hardening we ship a reference systemd unit
  (ProtectSystem=strict, ReadWritePaths=state dirs, NoNewPrivileges)
  so non-Cloudron deployments recover the container's outer wall.
