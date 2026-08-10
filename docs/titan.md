# Titan uploads

Titan is the community's companion protocol for *writing* to a capsule.
Same TLS, same port, same client certificates as Gemini — only the URL
scheme differs. In `usv` it is handled on the same listener, dispatched
by scheme, and it is off until you configure a zone.

Spec: `gemini://transjovian.org/titan` (the canonical copy is
Gemini-only; `docs/recon/titan.md` has a full reading with citations).

## Turning it on

```toml
[[host]]
name = "example.org"

[[host.titan_zone]]
path_prefix = "/uploads/"
identities  = ["laptop"]          # or fingerprints = ["sha256-hex…"]
max_upload_bytes = 1048576
mime = ["text/gemini"]
allow_delete = false

[[identity]]
label = "laptop"
fingerprint = "sha256-hex…"
capabilities = ["titan"]
```

Get the fingerprint of the client certificate you want to authorise with
`usv identity add`, which prints a ready-made config snippet rather than
making you assemble one.

**An empty allowlist is a startup error.** Not "allow anyone" — `usv`
refuses to start. Anyone can mint a self-signed certificate, so a
writable zone with no allowlist is a writable zone with no protection.
This is deliberately the opposite of `cert_zone`, where an empty list
sensibly means "any valid certificate", because reading and writing do
not deserve the same default.

## Writing a page

From Lagrange: open the page, *Edit Page with Titan*, save. From the
command line, `titan` (shipped with gmid) or any Titan-capable client.

```
titan://example.org/uploads/hello.gmi;mime=text/gemini;size=42
```

## What happens on the server

1. TLS completes, so the client certificate is already known.
2. **Authorisation runs before any payload is read** — wrong or missing
   certificate gets `60`/`61` immediately, and the connection is drained
   rather than left to dribble a rejected body at you.
3. The declared `size` is checked against the zone's cap *before*
   reading, and the read is hard-capped regardless of what was declared.
4. The body is buffered fully, then written atomically into the content
   tree. A partial upload never becomes a partial page.
5. The render pipeline re-enters and both surfaces update.

Path traversal is refused at the same canonicalisation boundary the
static handler uses, with its own fuzz target — an upload can never
escape the content tree.

## Things worth knowing

**Uploads are content, not code.** An uploaded file is served as data.
There is nothing to execute, whatever it claims to be.

**`token` is weak.** It rides in the URL. It exists because the spec
defines it, and it may add a second factor alongside a fingerprint
allowlist. It is never a substitute for one.

**`allow_delete` defaults to false**, and deletion is the one operation
that cannot be undone by re-uploading.

**Rotation.** `superseded` + `superseded_until` on a roster identity
keeps an old fingerprint working for a fixed window while you move to a
new key. The window closes itself; `superseded` without a date is a
startup error, so there is no way to leave an old key valid forever by
forgetting about it.

**The web mirror publishes what you upload.** A Titan zone under a
cert-gated path still renders to HTML, and the HTML surface has no
access control. If content must stay private, keep it out of the
rendered tree entirely.

## Checking your setup

```sh
usv zones          # every cert and Titan zone as usv understands it
usv identity list  # the roster, with rotation windows
usv check          # config validation + content lint
```

## Why native rather than delegated

gmid validates Titan requests and forwards them to a FastCGI backend —
reasonable for a server whose extension model is FastCGI, but `usv` has
no dynamic-content tier to delegate to (ADR 0005) and adding one purely
for uploads would reintroduce the whole class of risk that decision
removed. GmCapsule handles Titan natively, buffering the full payload
before dispatch and requiring a client certificate by default; `usv`
follows that shape and hardens the default from *by default* to
*always*.
