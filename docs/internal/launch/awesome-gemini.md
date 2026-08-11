DRAFT — not submitted. `awesome-gemini` (github.com/kr1sp1n/awesome-gemini).

A list entry, not an announcement. This is the highest-value, lowest-effort
item in the wave and the one most likely to still be sending people in five
years — a thread dies in a week, a list entry doesn't.

**Do not submit until the repository is public.** A PR linking a private
repo will be closed, and correctly.

## Where it goes

Under the servers section, in whatever order that section uses
(alphabetical at time of writing — check, don't assume). Match the
surrounding entries' format exactly rather than the format below;
the house style wins.

## The entry

```markdown
- [Unseen Servant](https://forgejo.wanderingmonster.dev/WanderingMonster/unseen-servant) - Gemini and Titan server in Rust that also renders the same content tree to static HTML for the web.
```

One line. No adjectives that can't be checked, no "blazing fast", no
feature list. The reader is scanning a list of thirty servers and wants
the distinguishing fact, which here is the dual surface.

## PR description

Keep it to a couple of sentences — maintainers of list repos review a
lot of these:

```
Adds Unseen Servant, a Gemini + Titan server written in Rust. Its
distinguishing feature is that it renders the same gemtext tree to
static HTML at write time, so a capsule is readable both natively and
in a browser from one source.

MIT licensed. Note for transparency: the project is AI-authored under
human direction, which is stated prominently in its README.
```

Include the AI disclosure here. A list maintainer may reasonably have a
policy about it, and finding out after merging is worse for everyone
than a decision made up front.

## Before submitting

- [ ] Repository public
- [ ] README renders correctly for a first-time visitor
- [ ] Link resolves without a login
- [ ] Entry format matches its neighbours
- [ ] Claims in the one-liner match `../protocols.md`
