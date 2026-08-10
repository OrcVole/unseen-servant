## Your capsule is live

- **Gemini**: `gemini://$CLOUDRON-APP-FQDN/`
- **Web mirror**: $CLOUDRON-APP-ORIGIN/

The first connection to the Gemini address will show a certificate warning
in most clients — that's normal, and it's the point. Unseen Servant uses
TOFU (trust-on-first-use) identity, the same way SSH does: the client
remembers the certificate it saw the first time, and only warns again if it
ever changes. There is no certificate authority to trust in advance.

### Adding content

Use the file manager (the Files icon on this app's dashboard tile) to edit
the capsule under `content/` — gemtext (`.gmi`) files, one per page. Every
save re-renders both surfaces within a couple of seconds.

### If you ever move this capsule to a new domain

Unseen Servant detects the hostname change and will mint a fresh identity
for the new name rather than silently reusing the old one — readers who
pinned the old certificate would otherwise see it as a possible
impersonation. The old keypair is kept, untouched, alongside the new one.

### The command line

`cloudron exec` drops you into the container, where the `usv` binary itself
has a full CLI: `usv status`, `usv fingerprint`, `usv check`. None of them
touch your content — they only report on it.
