DRAFT, not posted. Cloudron forum (forum.cloudron.io).

Different audience from every other venue here: they do not care about
Geminispace. They care whether the package is well-made, whether it
follows platform conventions, and whether it will still work after the
next platform update. Write to that.

**A thread already exists**: the one asking whether a minimal base
image instead of `cloudron/base` is a reasonable approach. Its outcome
should shape this post: if the minimal base turns out to be a problem,
that gets fixed *before* announcing a package built on it, and this
draft's size claims change.

There is also standing prior art to be courteous about. Agate+ (Tim
Considine) is the closest existing Gemini package on this forum and its
author is the person most invested in this niche here. His thread
documented real problems: health checks, process supervision: that
directly informed usv's design. Say so, genuinely, rather than arriving
as a replacement.

---

**Title:** Unseen Servant: a Gemini/Titan server packaged for Cloudron

---

**Body:**

I have packaged a Gemini server for Cloudron and would appreciate review
before I take it any further.

**What it is:** Unseen Servant (`usv`) is a Gemini server written in
Rust. Gemini is a small internet protocol: think of a capsule as a
personal site made of plain text files, served over TLS on port 1965 to
its own clients. The relevant part for this forum is that `usv` also
renders the same content to static HTML, so the app's dashboard tile
opens a real web page rather than a dead link.

**How it fits the platform:**

- Single process: no supervisor needed. (This is directly downstream of
  the Agate+ thread here: multi-process packaging was the friction that
  came up in review, and a one-binary design sidesteps it.)
- `tcpPorts` for 1965, pinned `readOnly`: Gemini clients assume that
  port, and a capsule on another one is effectively undiscoverable.
- `httpPort` 8000 behind the platform's nginx, with `healthCheckPath`
  returning 2xx unconditionally: including when the Gemini service has
  been disabled by the admin, so the app can never be stuck "Starting…"
  because of protocol-side configuration.
- `localstorage` for `/app/data`; the server's TOFU keypair lives there
  and therefore survives backup, restore, update and migration.
- `multiDomain`: alias domains are served by SNI from the one instance,
  each with its own certificate.
- Runs as the `cloudron` user, drops privileges in `start.sh`, logs only
  to stdout/stderr.

**On image size:** the package is a ~20MB image. Built the conventional
way on `cloudron/base` it was 2.46GB, for an 8.85MB binary, and installs
on a busy host were taking around 26 minutes almost entirely in the
image pull: versus about 1m40s now. That is the subject of my other
thread, and if the conclusion there is that the minimal base causes
problems I'd rather find out now than after anyone installs it.

**Status:** pre-1.0 and not submitted to the App Store. Posting for
review rather than announcing availability.

**One disclosure:** `usv` is AI-authored under human direction. Design
decisions are recorded as written notes before implementation and the
research behind them is in the repo, so the reasoning can be judged
rather than taken on trust. Stating it up front rather than leaving it
to be discovered.

Happy to share the `Dockerfile`, manifest and `start.sh` if that is
useful: the packaging critique is the feedback I actually want.

---

## Notes for the poster

- Do not post this while the base-image thread is unresolved.
- Explain Gemini in one sentence, once. This audience is technical but
  has no reason to know the protocol.
- Lead with platform-fit, not features. The question in a reviewer's
  head is "will this package cause me support tickets?"
