# Integrations

How usv composes with the world around it. Sections marked with a phase are
committed but not yet built; design detail lives in
`docs/notes/integration-ideas.md` until each hardens here.

## Cloudron (C6)

The primary deployment profile: tcpPort 1965 (pinned), proxied HTTPS web
surface, `/app/data` state, panel file manager as the content-authoring UI,
web terminal for the `usv` CLI. Full constraint set:
`docs/recon/cloudron-fit.md`.

## Tor and I2P (C5)

Onion/eepsite capsules via ordinary tunnel configuration plus three usv
affordances: `advertised_host` override, a cert slot for the onion
hostname, and graceful no-SNI handling. Recipes (torrc, I2P server tunnel)
land here with C5.

## OnionShare (C5)

`usv export` emits the rendered HTML tree as a drop-in folder for
OnionShare's website mode — a zero-infrastructure onion mirror of your
capsule.

## Feeds and aggregators (C3)

Generated indexes carry gemsub dated links; `atom.xml` is emitted for both
surfaces. CAPCOM/Antenna consume the Gemini side natively; web feed readers
take the Atom. Submission/announcement mechanics: `docs/ROADMAP.md` M6.

## Contact addresses (documentation only)

usv is a content server, not a mailbox: for a capsule contact address the
smolnet answer is a `misfin://` link served beside your content (one-click
in Lagrange ≥1.18) with a standalone misfin server handling delivery, or a
plain `mailto:`. See `docs/recon/smolnet.md` §5.

## Smolnet side-protocols (v1.1)

Gopher, Spartan, Nex, and Finger as opt-in listeners over the same content
tree — all plaintext, all off by default, trust model documented plainly.
Design source: `docs/recon/smolnet.md`.
