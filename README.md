# Unseen Servant (`usv`)

> **Pre-release. Unannounced.** Nothing links here yet, on purpose: the
> project is gated on its v1.0 quality bar (a clean pass of the community
> gemini-diagnostics torture suite, among others — see `docs/ROADMAP.md`).
> If you've found this early, please don't share it around yet.

A security-first [Gemini](https://geminiprotocol.net/) server in Rust that
publishes **one content tree to two worlds**: served natively as gemtext on
port 1965, and statically rendered to themed, classless HTML for the web.
Write a gemlog once; readers reach it from Lagrange or lynx or Chrome.

- **TOFU-native identity**: certificates auto-generated once, per hostname,
  and never silently touched again — your capsule's identity survives every
  update, restore, and migration (`UPGRADING.md`).
- **Titan uploads** (client-certificate-gated), certificate-gated private
  zones, gemsub + Atom feeds, visitor responses with moderation-first
  defaults — all opt-in, all designed before built (`docs/adr/`).
- **Runs anywhere**: a single binary with a TUI setup wizard standalone, or
  a first-class [Cloudron](https://www.cloudron.io/) package; the smolnet
  side-protocols (gopher, Spartan, Nex, Finger) arrive as opt-in listeners
  in v1.1.
- **AI Forward**: AI-authored end to end, human-directed, with the research
  and every decision on the record in `docs/`.

## Status

Phase C0 (scaffold) of the build plan — see `AGENTS.md` for the live phase
state and `docs/BUILD-PLAN.md` for the road from here to v1.0.

## License

MIT.
