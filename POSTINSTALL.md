## Your capsule is live

- **Gemini**: `gemini://$CLOUDRON-APP-FQDN/`
- **Web mirror**: $CLOUDRON-APP-ORIGIN/

Both addresses serve the same content. There is nothing else to set up —
a starter page is already written, and the identity your readers will
trust has already been generated.

### The first visit will show a warning

That is expected. Gemini uses TOFU (trust-on-first-use), like SSH: your
reader's client remembers the certificate it saw the first time and only
warns again if it *changes*. No certificate authority is involved.

To confirm readers are seeing the right one, run `usv fingerprint` (see
below) and compare it with what your client shows.

### Adding content

Open the **Files** icon on this app's tile and edit `content/` — one
gemtext (`.gmi`) file per page. Saving re-renders both surfaces within
seconds. No build step, no deploy.

### The command line

`cloudron exec` gives you a shell. `usv status`, `usv fingerprint`, and
`usv check` report on the capsule; none of them modify your content.

### Moving to another domain

Unseen Servant notices the new hostname and mints a fresh identity for it
rather than silently reusing the old one — reusing it would look like
impersonation to anyone who had pinned it. The old keypair is kept.
