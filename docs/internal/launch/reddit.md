---
title: "Reddit"
description: "DRAFT, not posted. r/geminiprotocol (and possibly r/selfhosted for the packaging angle, which is a different post: see the bottom)."
type: reference
status: decided
last_verified: 2026-08-30
---

DRAFT, not posted. r/geminiprotocol (and possibly r/selfhosted for the
packaging angle, which is a different post: see the bottom).

Reddit rewards posts that state their own limitations before a commenter
finds them, and punishes anything that reads like a launch. Every
weakness below is deliberately above the fold. Do not post unless
somebody will be around for a day to answer replies; an untended thread
reads worse than no thread.

---

**Title:** I built a Gemini server that also publishes the same content
as plain HTML: Unseen Servant

---

**Body:**

I have been writing a Gemini server in Rust called Unseen Servant (`usv`),
and it is ready for people to poke holes in.

**The idea:** you keep one folder of gemtext. `usv` renders it at write
time into both surfaces: served as-is to Gemini clients on 1965, and as
plain classless HTML for anyone on a browser. Not a per-request gateway:
the whole tree re-renders when a file changes and gets swapped in
atomically, so the web side is genuinely just static files. Save a page,
and a couple of seconds later it is live in both places from one source.
The rendered folder also stands alone, so you can copy it behind
OnionShare or onto any static host.

**Things it does:** Titan uploads on the same listener, gated per zone on
client-certificate fingerprints; certificate zones for private paths;
Atom and gemsub feeds; three themes; a site map on both surfaces; optional
Gopher, Spartan, Nex and Finger listeners over the same folder, off by
default; and
packaging for Cloudron, Debian, Fedora/RHEL, Arch, Nix and a small
container image.

**Before you get excited, the limitations:**

- **It is pre-1.0 and unaudited.** Agate and gmid have years of
  production hardening this does not.
- **The cleartext protocols carry less.** Gopher, Spartan, Nex and
  Finger have no TLS and no client identity, so anything you gate behind
  a certificate on the Gemini side is excluded from those trees at render
  time, structurally. They are off unless you turn them on.
- **No dynamic content at all**: no CGI, FastCGI, scripting, or plugin
  API, and that is a permanent design decision, not a todo. If you want
  to build something interactive, GmCapsule is the right tool and this
  is not.
- **It is not a web server.** The HTML side has no auth and no
  request-time logic. It exists so someone with a browser can read your
  capsule, nothing more.
- **On logs, since someone always asks:** no visitor addresses by
  default: the peer field is a dash, and query strings are redacted by
  construction. You can opt into verbatim addresses, or into a
  per-boot-salted digest that correlates repeat visits within one run
  and survives no restart. The default used to be the other way round
  and was changed deliberately.
- **It is written by an AI**, directed and reviewed by a human, with every
  design decision recorded as a written note before the code. I'm saying
  so up front because you are entitled to decide about that before running
  it, and because finding out later would rightly annoy you. If that is a
  dealbreaker, that is a reasonable position and I'm not going to argue
  you out of it in the comments.

**Where:** [capsule, served by usv
itself](gemini://unseenservant.dev/), and the same
thing [in a browser](https://unseenservant.dev/), which
is sort of the point.

Happy to answer anything, including the awkward questions.

---

## Notes for the poster

- Reddit will not linkify `gemini://`. Give the HTTPS mirror as the
  clickable one and mention the Gemini address in text: this is the one
  audience where the mirror genuinely helps.
- Do not cross-post the identical text to r/selfhosted. That audience
  cares about the packaging and the 20MB Cloudron image, not the
  protocol; write a separate, shorter post if it is worth doing at all.
- If the thread turns into an argument about AI authorship: answer once,
  honestly, and then let it be. Re-litigating it in twelve replies costs
  more than the original objection.
