---
title: "Agents"
description: "Usv treats an AI agent as an ordinary user of the capsule, not as a crawler to be managed. Everything below is on by default, addressable by URL (uniform resource locator), and identical to."
type: reference
status: decided
last_verified: 2026-08-11
---

# Agents

> Unseen Servant

`usv` treats an AI agent as an ordinary user of the capsule, not as a
crawler to be managed. Everything below is on by default, addressable by URL
(uniform resource locator), and identical to what a human gets: there is no
agent mode and no content negotiation.

If you are an agent working on `usv`'s **source code** rather than its
content, you want [`../DEBUGGING.md`](../DEBUGGING.md) and
[`../AGENTS.md`](../AGENTS.md) instead.

The same case, as a page you can hand to someone else, is
[`agents.html`](agents.html).

## If you are choosing this on an agent's behalf

Written for the person doing the choosing, not the agent. The question is
not "does it have AI features" but "what does it cost me to let something
automated publish here, and what happens when it misbehaves".

**Your agent's output needs an address.** Anything worth keeping needs
somewhere durable and citable to live, and a chat transcript is not a
location. Here it is a file in a folder, live on every enabled network
within 300 milliseconds, at a URL that does not change.

**Integration cost is a file write.** No client library, no software
development kit, no API key, no build step, no deploy. Anything that can
write a file can publish. Over the network it is one request. That matters
more than it sounds: most of what breaks in agent publishing is the stack
between the agent and the page, and here there is not one.

**You can grant access without handing over a secret.** An agent presents
a client certificate; you authorise its fingerprint against a named roster
entry with named capabilities, scoped to a path. There is no password to
leak into a prompt, a log or a transcript. Revoking is deleting a line,
and it takes effect on the next request rather than at the next restart.
Key rotation has a window that closes itself, so forgetting fails closed.

**The blast radius is bounded by the design, not by your vigilance.**
Nothing is ever executed. An agent that goes wrong writes bad prose; it
cannot run code, escalate, reach your other services or install anything,
because none of those paths exist for anyone. The worst realistic outcome
is a page you restore from backup. Compare that with giving an agent
credentials to a content management system.

**You can see what it did, and so can another agent.** A cert-gated status
resource reports health, the roster and recent activity. Every read-only
command emits JSON, so supervising the publisher with a second automated
process needs no scraping.

**Reading back is cheap.** An agent revisiting its own output fetches one
inventory file and then clean Markdown, rather than crawling and stripping
markup. Gemtext round-trips losslessly, so what comes back is exactly the
structure that was written, which is not true of HTML.

**Nothing here fights automation.** No JavaScript to execute, no cookie
wall, no bot detection to negotiate, no rate limiter to back off from. You
are not working against your own infrastructure to let your agent in.

**And none of it is an AI-only stack.** Every affordance above is also an
accessibility or ordinary usability feature, so you are not maintaining a
second system for machines. If the agent stops being useful tomorrow, you
still have a capsule that people can read.

The honest counterweight: this is a publishing surface, not an agent
platform. It has no memory, no retrieval, no scheduling and no tool
protocol, and it is not going to grow them. If what you need is somewhere
for an agent to *think*, this is the wrong tool. If you need somewhere for
an agent to *put things* that will still be there and still be readable in
five years, that is what it is for.

## Reading a capsule

| You want | Fetch |
|---|---|
| Every page, one request | `/llms.txt` (web) or `/map.gmi` (Gemini) |
| A page without markup | any page with `.md` instead of `.html` |
| The machine index | `/sitemap.xml` |
| Dated posts | `/atom.xml`, or `/feed.gmi` |
| What this capsule is and where else it answers | `/usv` |

`/llms.txt` links the `.md` form of every page, so one fetch gives you the
inventory and the second gives you clean Markdown. Both are written by the
same render pass that writes the HTML (HyperText Markup Language) and the
gemtext, from the same source file: there is no path by which they
disagree.

`robots.txt` is permissive unless the operator wrote a `robots.txt` into
their content directory. AI crawling is allowed by default.

## Writing to a capsule

Publishing is a file write. Over the network, that is Titan on the same port
as Gemini, gated by client certificate:

1. The operator adds your certificate's SHA-256 fingerprint to the roster
   with the `titan-write` capability, and names it in a Titan zone.
2. You `titan://host/path/page.gmi;size=N;mime=text/gemini` and send the
   body.
3. Both surfaces are re-rendered within the debounce window (300 ms).

Your identity is the key. There is no account, no password, no token
exchange, and no session. `usv` records the date a key was enrolled and
nothing else about who holds it.

**Rotation.** An identity may hold a second fingerprint during an overlap
window, so you can enroll a new key and prove control from the old one
without losing the label or its capabilities. The window must carry an
expiry date; it closes itself.

**Capabilities** are server-wide grants that compose with zone membership,
both are required. There are three: `read`, `titan-write`, `admin`.

## Operating a server

Every read-only subcommand takes `--json` and prints one object on one line,
so several invocations concatenate into valid JSON (JavaScript Object
Notation) Lines:

```sh
usv status --json      # config, fingerprints, roster, zones, published
usv check --json       # config validity + content lint
usv stats --json       # what is currently published
usv zones --json       # certificate and Titan zones
usv fingerprint --json # this capsule's server certificate fingerprints
```

Logs go to stderr, so stdout is only ever the report:

```sh
usv status --json 2>/dev/null | jq .capsule.theme
```

`USV_LOG_FORMAT=json` switches the log itself to one JSON object per line.
`RUST_LOG` filters as usual.

**Exit codes are a contract**, checked by the test suite:

| Code | Means |
|---|---|
| 0 | success |
| 1 | the command ran and failed: bad config, unreadable state, I/O |
| 2 | the command line was wrong; nothing ran |

`--json` on a subcommand that has no report: `render`, `export`, `init`,
`identity`: is an error, not a silent no-op, so you never believe you asked
for JSON and receive prose.

**Over the wire**, an identity holding `admin` can fetch
`/admin/status.gmi`: health, the last render's stats, the roster, and a
recent-activity tail. It is read-only, and that is the whole remote surface.
Every mutation: reload, re-render, identity add/rotate/revoke: is CLI-only
and needs host access, so there is no remote control plane to seize.

## Status codes

Gemini's classes are already a machine interface and `usv` uses them
literally: `20` success, `30`/`31` redirect, `40`/`41`/`44` temporary,
`50`/`51`/`53`/`59` permanent, `60`/`61`/`62` certificate. A `20` response
never contains an error page. `53` means the request was for a host or
scheme this server does not serve: `usv` is not a proxy.

## Not provided

Stated so you do not go looking:

- **No MCP (Model Context Protocol), A2A (Agent2Agent), or agent-card
  endpoint.** Those are HTTP (HyperText Transfer Protocol) carrying
  JSON-RPC (JSON Remote Procedure Call); `usv` is a publishing surface that
  agents write to and read from, not a transport. An operator can host a
  static `agent-card.json` as ordinary content.
- **No JSON API (application programming interface) for content, and no
  admin API.** Content is gemtext.
- **No content negotiation.** The `.md` form is a separate address, not a
  different answer to the same one.
- **No memory or retrieval backend.** No vector index, no query. `usv` is
  durable addressable output, not a memory store.
- **No enrollment tokens yet.** ADR 0011: one of the architecture
  decision records in [`adr/`](adr/): specifies a single-use,
  capability-scoped token for claiming a roster slot on first contact. It is not implemented; enrolling a new identity today
  means the operator running `usv identity add` and pasting the block into
  `usv.toml`.
- **No `admin` surface on the web mirror.** `/admin/status.gmi` is
  Gemini-only, so an agent that reaches the capsule over `https://` cannot
  observe the server. Recorded in ADR 0011's amendment.

## Why any of this exists

The affordances above were chosen because each one is also an accessibility
feature or an ordinary usability feature. A site map is WCAG (Web Content
Accessibility Guidelines) 2.4.5 and a crawl-free inventory. A `.md` URL is a
clean read for a person too. Machine-readable CLI (command-line interface)
output is what any script wants. If the agent audience never arrives, none
of it is wasted, which is why these were built and the agent-only features
were not.

The reasoning is in
[`adr/0010-agent-and-assistive-access.md`](adr/0010-agent-and-assistive-access.md)
and
[`adr/0011-agent-identity-and-management.md`](adr/0011-agent-identity-and-management.md).
