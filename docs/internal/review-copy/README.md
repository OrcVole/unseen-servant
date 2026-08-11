# Review copy: every page a person actually sees

One folder holding the first thing a reader meets on each protocol, plus
the two pages an operator meets on Cloudron, so they can be reviewed
together for tone and accuracy instead of hunted down one protocol at a
time.

**Regenerate with:** `scripts/collect-review-copy.sh [host] [out-dir]`

Everything here except the two Cloudron files is **fetched from a
running server**, not rendered from source. That is deliberate: it is the
served bytes that get reviewed, so a formatting fault that only appears
on the wire cannot hide behind a tidy-looking source file. These are
snapshots: the source of truth is the code and the Markdown they were
copied from, and edits made *here* are lost on the next run.

| File | What it is | Where it comes from |
|---|---|---|
| `colophon-gemini.gmi` | The colophon over Gemini | generated, `render/colophon.rs` |
| `colophon-gopher.txt` | The colophon over Gopher, markup flattened | generated |
| `colophon-spartan.gmi` | The colophon over Spartan | generated |
| `colophon-nex.gmi` | The colophon over Nex | generated |
| `finger-profile.txt` | What `finger @host` answers | generated, `handler/finger.rs` |
| `*.header.txt` | The response header, kept so the MIME is reviewable | the wire |
| `cloudron-landing-page.md` | The app's store/landing copy | `DESCRIPTION.md` |
| `cloudron-admin-panel.md` | What an operator reads after install | `POSTINSTALL.md` |

## What to look for

- **The name.** Every page should make `usv` guessable: *UnSeen serVant*.
  A reader arriving cold on any protocol should not have to search.
- **Right protocol, right words.** A Nex page must say Nex, quote a
  `nex://` address, and list only clients that actually speak Nex.
- **Addresses.** These come from live config, so they are true for the
  capsule that produced them. Check the *shape*: every cleartext address
  carries its port, because those clients assume 70/300/1900/79.
- **Client lists.** Checked August 2026 and stated as such on the page.
  Support drifts; treat a stale list as a bug.
- **Tone.** These pages greet strangers. They should explain without
  condescending and sell nothing.

## Known gaps

- **Finger has no colophon of its own.** Its page is a *profile*, not a
  document, so the protocol introduction is folded into the profile in
  four lines rather than served separately. Judge it as a profile.
- **The two Cloudron files are copies**, not fetched from a running
  Cloudron: the landing page as rendered by the store, and the admin
  panel with its placeholders substituted, will differ in presentation.
- **Gemini's colophon is not in the site map**, since it is generated at
  request time rather than rendered into the tree. It is reachable and
  linked from nowhere; whether it *should* be linked from the capsule
  root is an open question worth deciding during this review.
