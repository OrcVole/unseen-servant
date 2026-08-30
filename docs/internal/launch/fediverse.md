---
title: "Fediverse"
description: "DRAFT, not posted. Fediverse, #gemini (and #smallweb / #smolnet)."
type: reference
status: decided
last_verified: 2026-08-30
---

DRAFT, not posted. Fediverse, `#gemini` (and `#smallweb` / `#smolnet`).

Short. One image. The image is `assets/mascot.png`: the dot-mesh
servant, and it **needs alt text**, which on the Fediverse is a norm
with teeth, not a nicety.

Link the HTTPS mirror, not the `gemini://` URL: most clients will not
linkify the latter, and half the audience cannot open it. The point of
having a mirror is that this is no longer a problem.

---

## Main post (fits comfortably under 500 characters)

```text
Unseen Servant: a Gemini server that publishes the same folder of
gemtext twice: natively on 1965, and as plain static HTML for
everyone else. One source, no build step.

Titan uploads, cert-gated zones, feeds; Gopher, Spartan, Nex and
Finger if you want them. Packaging for Cloudron, Debian, Fedora,
Arch, Nix.

Pre-1.0. AI-authored under human direction: said up front.

https://unseenservant.dev/

#gemini #smolnet
```

**Image:** `assets/mascot.png`

**Alt text** (write it in full; do not skip):

```yaml
A human figure drawn as a mesh of small glowing green characters and
dots on a black background, in the style of an old phosphor terminal
display: present, but only just visible.
```

---

## Optional follow-up in the same thread

Only if the first post gets traction. Keeps the disclosures visible to
anyone who boosts the top post.

```text
Two things worth saying plainly:

It logs no visitor addresses by default: the peer field is a dash,
queries are redacted by construction. Opt in if you want them.

And the cleartext protocols are off by default: no TLS, no client
identity, and nothing you cert-gated ever reaches them.
```

---

## Notes for the poster

- Content warnings are not needed here, but if your instance has norms
  about self-promotion, follow them.
- Expect the AI-authorship line to draw replies. Answer once, plainly.
- Do not boost your own post repeatedly. This audience notices.
