# ADR 0012: The smolnet protocols: one plaintext listener kind, opt-in, and a hard wall against leaking gated content

- Status: **Proposed** (v1.1 scope; ADR precedes code per docs/internal/ROADMAP.md)
- Date: 2026-08-10
- Evidence: docs/internal/recon/smolnet.md (wire formats, server/render requirements, effort ranking, venues); docs/internal/recon/ecosystem.md; ADR 0004 (dual surface), ADR 0005 (no dynamic content), ADR 0002 (process model); director 2026-08-09 ("include finger"), director 2026-08-10 ("we hope we will be supporting gopher etc when we eventually release")

## Context

v1.1 adds Gopher, Spartan, Nex and Finger. The recon establishes the
shape: all are one-shot, line-framed, plaintext TCP protocols: read a
line, optionally a body, write, close. None support keep-alive. None
support TLS (bar optional `gophers` sniffing). **None can authenticate
a client at all.**

That last fact is the one with teeth. `usv` today has exactly one
content tree, and parts of it can be gated behind client certificates
(ADR 0005's certificate zones; ADR 0006's Titan zones). Adding a
protocol that *cannot* express authentication, over a transport with no
confidentiality, to a server whose content tree contains gated material,
creates an obvious way to publish private content by accident.

Everything else here is engineering. That is a safety property.

## Decision

### 1. One `PlaintextListener` abstraction, parameterised

A single one-shot listener kind, parameterised by request parser and
response writer, serves all four. It reuses the Gemini listener's
existing machinery: accept loop, semaphore-based connection cap,
per-phase timeouts, request-line length caps, graceful drain. Protocols
differ in their grammar and their response framing, not in their
connection lifecycle.

Rejected: four bespoke listeners. The lifecycle is genuinely identical
and duplicating it would mean fixing every slowloris-class bug four
times.

### 2. Off by default. Always.

Every plaintext listener is opt-in per capsule, and a capsule that says
nothing gets none of them. A plaintext service is a decision an operator
must make deliberately: it changes the capsule's exposure, not just its
reach.

Enabling one logs a one-line trust disclaimer at startup, naming the
protocol and the fact that it is cleartext and unauthenticated. Not a
warning to be silenced: a statement of what was just switched on.

### 3. Non-privileged default ports

Ports 70 (gopher), 79 (finger) and 300 (spartan) are privileged; 1900
(nex) is not. Defaults are non-privileged: 7070, 7979, 3000, 1900, 
preserving ADR 0002's empty `CapabilityBoundingSet`, which is only
possible while every port is above 1024.

Reaching the canonical ports is documented rather than defaulted:
`CAP_NET_BIND_SERVICE`, systemd socket activation, a sysctl, a NAT
redirect, or on Cloudron a `tcpPorts` manifest entry. An operator who
wants port 70 can have it; they will know they asked, and what it cost.

### 4. Gopher is a render target; Spartan and Nex are not

Following ADR 0004's write-time rendering: the same metadata pass gains
emitters rather than growing request-time logic.

- **Gopher**, a full third output target: menus per directory/page, item
  typing, 70-column wrapping, `caps.txt`. This is where the effort is
  (recon ranks it 6-10× Nex).
- **Spartan**: serves the existing gemtext tree unchanged. Prefer
  relative links so no scheme rewriting is ever needed.
- **Nex**: serves the gemtext tree; optional nexification.
- **Finger**: serves a configured profile/status text. Not the content
  tree at all.

**Redirects are resolved at generation time** for the Gopher and Nex
targets, because neither protocol can express a redirect. That is a
pipeline requirement, not a listener one.

### 5. Spartan uploads are refused. Permanently.

Spartan's upload path is unauthenticated by construction. `usv` has a
Titan implementation that is cert-gated precisely because writable
means authenticated (ADR 0006). Accepting unauthenticated writes would
undo that on a second door. Spartan requests carrying a body get an
error; there is no configuration that enables them.

### 6. The wall: gated content may never reach a plaintext tree

**Certificate-zoned and Titan-zoned paths are excluded from every
plaintext render target, and this is enforced in code, not documented as
a caution.**

The render pipeline already knows each host's `cert_zones` and
`titan_zones`. A page whose path falls inside one is skipped when
emitting the Gopher/Spartan/Nex trees.

This is deliberately stricter than "document it plainly", which is what
the recon proposed. Documentation does not survive an operator adding a
`cert_zone` six months after enabling Gopher. The exclusion does.

### Amendment, 2026-08-10: made while implementing this section

This section originally required that a configuration which *would*
publish a gated path over a cleartext protocol be a **startup error**.
Implementing it showed that reading to be too strong, and the amendment
is recorded here rather than the code quietly diverging from the ADR.

Since exclusion already makes disclosure impossible, a blanket error
would mean a capsule with one small private area could not serve Gopher
**at all**. That does not make anyone safer; it forces a choice between
the cert zone and the protocol, and the likeliest casualty is the cert
zone: the safety measure, abandoned to get the feature. A rule that
pressures operators into removing their own gates is a bad rule however
strict it looks.

What the section now requires, and what is implemented in
`src/render/cleartext.rs`:

1. **Exclusion is unconditional and structural.** Gated prefixes are
   subtracted where the cleartext trees are *built*, not where they are
   served, so no request path, no later misconfiguration, and no
   protocol added in future can serve what was never emitted. Titan
   zones are excluded alongside certificate zones: a writable area is
   one whose contents are controlled by a specific key, and mirroring it
   somewhere anyone can read *and alter* is the same disclosure with an
   extra step.
2. **Every exclusion is announced at startup.** An operator who gated
   `/private/` and then enabled Gopher must not have to deduce why it is
   missing: silence is how a safety measure gets mistaken for a bug and
   worked around.
3. **A contradictory configuration is still a startup error.** A
   cleartext root pointing *into* a gated prefix can only mean "publish
   this gated thing in the clear". That is an instruction, not an
   oversight, and it is refused with a message naming both sides and
   saying what to do about it.

Prefix matching is on whole path segments in both directions: `/up`
must not gate `/uploads`, and must still gate `/up` itself as well as
everything beneath it. Getting that wrong leaks or hides the wrong
thing, so it is tested explicitly.

### 7. Honesty about the trust model, in the docs and on the wire

`docs/protocols.md` and each capsule's own documentation state plainly:
these listeners offer no confidentiality, no integrity, no server
authentication, and no client authentication. Content served over them
is world-readable in transit and trivially tamperable by anyone on path.
Serve only content whose integrity loss is acceptable.

The counterweight, also stated: this is the settled norm of these
communities: Gopher has run cleartext for 35 years, and mirroring
public static content is the one workload where cleartext is defensible.

### 8. Logging

The existing policy applies unchanged (`server.log_peer`, default
`off`: OQ-9). Plaintext ports attract scanners, so these logs will be
noisier; that is a reason for the default to stay where it is, not a
reason for a per-protocol exception.

## Consequences

- One listener abstraction to harden, four protocols served by it.
- The Gopher render target is the bulk of v1.1's implementation budget.
- A capsule mixing gated Gemini content with plaintext protocols will be
  *refused at startup* rather than quietly leaking. Some operators will
  find this annoying; the alternative is worse.
- `usv` gains no client-authenticated capability on any new protocol,
  and never will: cert zones and Titan stay Gemini-only by construction.
- Until every one of these ships and has been exercised against a real
  client, `docs/protocols.md` continues to list them as **Planned**, and
  no README, capsule page, store listing, or announcement may say
  otherwise.

## Alternatives considered

**Serve the same tree on every protocol, warn in docs.** The recon's
proposal. Rejected: a warning does not survive a later configuration
change, and the failure is silent disclosure of content the operator
believed was gated.

**TLS on the smolnet listeners (`gophers`).** Deferred, not refused. It
exists, adoption is thin, and it does not solve client authentication, 
so it changes none of the reasoning above. Revisit if it gains traction.

**Privileged ports by default.** Rejected: it would cost the empty
capability bounding set that ADR 0002's whole hardening story rests on,
for the convenience of not typing a port number.

**One listener per protocol.** Rejected: see §1.
