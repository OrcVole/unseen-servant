## Your capsule is live

One folder of writing, published to several places at once. **The
hostname never changes — you change the bit before the `://`, and that
alone decides which protocol you get and which port you land on.**

| Type this | You reach | On port |
|---|---|---|
| `gemini://$CLOUDRON-APP-FQDN/` | the Gemini capsule, in a client like Lagrange | 1965 |
| $CLOUDRON-APP-ORIGIN/ | the web mirror, in any browser | 443 |

So swapping `gemini://` for `https://` on the *same address* gets you
the same writing in a browser instead. Nothing else to set up — a
starter page is already written, and the identity your readers will
trust has already been generated.

*(More smolnet protocols are in development and work the same way. When
gopher ships, typing `gopher://$CLOUDRON-APP-FQDN/` instead of
`gemini://…` will send you to port 7070 rather than 1965, and give you
a gopher experience — menus and all — of the very same content.)*

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
