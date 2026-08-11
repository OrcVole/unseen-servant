# Recon: the agentic web, mapped to Unseen Servant

**Completed 2026-08-09.** Design reconnaissance commissioned by the
director's question: *why might AI agents want to use or run usv, what
could help with agent identity, and what would they need to manage the
server?* Four parallel surveys of 2025-2026 developments (identity/auth,
content legibility, memory/presence, agent-to-agent discovery), each with
dated primary sources and an honest *aligned / adaptable / at-odds*
verdict for usv. This document feeds a proposed **ADR 0011 (agent
identity lifecycle)** and a posture decision on the HTTP/agent surface.

Sources are inline. Where a claim rests on secondary analysis rather than
a primary spec, it is flagged.

## The one finding that matters

Across all four surveys the same shape appears: **the agentic web is
retrofitting, onto HTTP, three primitives usv already has natively.**

1. **Identity is the key, not an account.** Web Bot Auth (Cloudflare,
   May 2025, Ed25519 + JWKS directory, no CA), SPIFFE/SPIRE workload
   SVIDs, W3C DIDs/Verifiable Credentials, and the Open Agent Identity
   effort are all converging on *the private-key holder is the actor*
   and *drop the CA hierarchy*. TOFU is now explicitly endorsed by the
   Non-Human-Identity community as a valid bootstrap for ephemeral
   callers. usv's client-cert fingerprint model **is** that primitive, 
   native and mandatory since day one, where HTTP has to bolt it on.
   (https://blog.cloudflare.com/web-bot-auth/ ·
   https://www.rfc-editor.org/rfc/rfc9421.html · https://openagentidentity.org/
   · https://nhimg.org/nhi-101/trust-on-first-use-for-workloads)

2. **Clean, JS-free, losslessly-parseable, one-truth content.** llms.txt
   (Sept 2024; ~844k sites Oct 2025 but no major vendor confirmed
   reading it), `.md`-per-page and `Accept: text/markdown` "Markdown for
   Agents" (Cloudflare, Vercel, Sentry), and the SSR-scramble (GPTBot/
   ClaudeBot/PerplexityBot execute *no* JavaScript) are all reaching for
   what gemtext already is: six unambiguous line types, one-bit-state
   parseable, server-rendered. The single biggest 2025 agent-legibility
   failure: SPA content invisible to crawlers: cannot occur in usv by
   construction (ADR 0010 refuses JS). (https://llmstxt.org/ ·
   https://www.checklyhq.com/blog/state-of-ai-agent-content-negotation/ ·
   https://www.radiantelephant.com/server-side-rendering-ai-crawlers/)

3. **Publish = a file write at a stable URL, private by default.** An
   entire startup category: display.dev, Stacktree, Artifacta: was
   invented in 2025-26 for the exact pain usv solves natively: one call,
   a URL that never changes, private-by-default (the link/identity *is*
   the credential), no git-push/build/deploy, provenance travelling with
   the artifact. Their whole product is hiding deploy complexity; usv has
   no deploy step to hide. And the A2A "agent card at a well-known path"
   + llms.txt "sitemap for models" **is** usv's site-map-as-manifest
   doctrine, older and losslessly parseable. (https://display.dev/agents ·
   https://stacktr.ee/best-private-html-hosting ·
   https://a2a-protocol.org/latest/topics/agent-discovery/)

**So the honest pitch is not "usv is a clever new agent thing." It is
"usv already sits at the destination the agentic web is straining
toward."** The work is not invention; it is packaging and lifecycle.

## The debts (where usv is behind, and it is fixable)

- **Identity lifecycle.** Every serious effort (SPIFFE SVIDs, OAuth 2.1
  in MCP, IETF WIMSE, ID-JAG) treats a durable static secret as an
  anti-pattern: credentials must be short-lived, **rotatable**, and
  **capability-scoped**. usv's cert-zone allowlist is a static, long-
  lived pin with *no rotation* and only path-prefix granularity.
  Rotation is the single most important gap: mis-pinning is the known,
  costly TOFU failure mode. (https://datatracker.ietf.org/group/wimse/ ·
  https://blog.modelcontextprotocol.io/posts/enterprise-managed-auth/)

- **HTTP-surface packaging.** Every agent convention above lives on
  `https://`. A `gemini://` capsule is invisible to GPTBot/ClaudeBot/
  Perplexity: only usv's **HTML mirror** is reachable by the actual
  agent audience of 2026. usv already holds the data (the site map) but
  not in the format/location agents look for (`/llms.txt`, `.md` page
  URLs). These are cheap re-serializations of one content tree.

- **Ergonomics: no MCP publish tool.** The biggest adoption wall.
  display.dev/Stacktree lead with an MCP `publish()` tool; usv expects an
  agent to speak Titan directly. Claude/Cursor/Codex agents cannot use usv
  today without custom glue. (https://display.dev/agents)

## The refusals (each survey independently drew the same lines)

- **Not an A2A/MCP transport or registry node.** A2A/MCP/ACP/NLWeb are
  all HTTP + JSON-RPC/REST + SSE; *no* shipping agent runtime speaks
  Gemini/Titan. usv cannot be a callable agent endpoint without becoming
  a different program. A JSON-RPC/agent-card *schema* is exactly the
  "bespoke manifest / second protocol" ADR 0010 refuses. usv can be
  *published-to and linked-from*, never the transport.
  (https://github.com/a2aproject/A2A · https://modelcontextprotocol.io/)

- **No per-request signing (RFC 9421 / Web Bot Auth).** Gemini's
  mandatory client-cert mTLS already gives per-connection cryptographic
  caller identity: the exact property RFC 9421 exists to retrofit onto
  HTTP. usv gains nothing from a signature/JWKS layer.

- **No CA attestation, no cross-server portable identity, no "agent
  passport."** TOFU proves *continuity* ("same caller as last time"),
  never *provenance* ("this key belongs to Acme Corp"). That is a real
  limit for agentic-commerce/KYA, and it is also usv's censorship-
  resistance value. usv should verify continuity honestly and never mint
  trust it does not have (it may *carry* an external DID/VC reference if
  ever needed). (https://www.w3.org/TR/did-core/)

- **Not a memory backend.** Agent "memory" (mem0, Letta, Zep) means
  write-many facts then *semantic/temporal retrieval*: a vector index,
  query, ranking. usv has no database, query, or search and must not
  pretend to. The honest split: **memory = private, queried working
  state; usv = the addressable, durable, human-and-agent-read output/
  presence surface.** usv is the output, not the state store; it
  complements mem0/Letta, it does not compete.
  (https://mem0.ai/blog/state-of-ai-agent-memory-2026 ·
  https://simonwillison.net/2025/Sep/12/claude-memory/: Willison praises
  raw, inspectable *plaintext* memory over opaque injection: validates
  gemtext's diffable, human-readable nature as a virtue.)

- **No content negotiation that returns different content to agents vs.
  humans** (cloaking) and **no pay-per-crawl / RSL paywalling** (not
  usv's fight; contradicts open-access). Use addressable `.md` URLs (same
  content, distinct resource) instead of header-switched divergence.

## Demand signals (that the audience is real)

- **Moltbook**: a Reddit-for-agents launched 28 Jan 2026, grew to
  millions of agents, acquired by Meta 10 Mar 2026. Agents want a
  *presence*: to post, log, be observed.
  (https://www.nbcnews.com/tech/tech-news/ai-agents-social-media-platform-moltbook-rcna256738)
- **GitHub Agent HQ** ("welcome home, agents", Universe 2025): "a place
  the agent lives and works" is now a mainstream product shape.
  (https://github.blog/news-insights/company-news/welcome-home-agents/)
- The display.dev/Stacktree/Artifacta category existing *at all* is the
  market validating usv's thesis.

## Design implications for usv

Nothing here requires breaking doctrine. Every adopt-item is a re-
serialization of content usv already holds as one source of truth, or a
lifecycle operation on the identity it already pins. Grouped by the
director's two questions plus the strategic fork:

**Identity (feeds ADR 0011).** Upgrade the cert-zone allowlist to a
named **roster**: fingerprint → {label, capabilities, enrolled,
last-rotated}. Priority order: (1) **key rotation** via a two-fingerprint
overlap window: closes the biggest gap and defuses TOFU mis-pin;
(2) **capability scoping** (`read` / `titan-write` / `admin`) rather than
path-only; (3) **single-use, expiring, capability-scoped enrollment
tokens**: the minimal CIMD/ID-JAG pattern, still TOFU/no-CA;
(4) honest provenance (record enrolled/rotated dates; optional external
DID/VC reference). Refuse RFC 9421, CA attestation, portability.

**Managing the server (feeds the management-reach decision).** Same
doctrine as the site map: *addressable, cert-gated affordances*, not a
JSON admin API. Observe: a cert-gated gemtext **status/roster/audit**
resource (health, render stats, enrolled identities, recent log). Control
(C5, CLI-first): `usv identity add/revoke/rotate`, `usv reload`,
`usv render`. The **MCP `publish` tool** (a thin Titan wrapper returning
the stable URL) is the highest-leverage ergonomics unlock: it is how an
agent actually *operates* content on the server.

**Agent home/presence (optional, doctrine-pure).** Append-only log
affordance (`titan append`: plaintext file growth, no DB); per-agent
zones keyed by fingerprint (`/agents/<fp>/…`: a self-hosted, un-owned
Agent-Card/Moltbook-profile analogue); a server-generated reverse-chron
index/feed per zone (from mtime, no DB); optional provenance front-matter.

**HTTP-surface packaging (the strategic fork).** Because the agent
audience lives on HTTP, the agent story is carried almost entirely by the
HTML mirror: expose `/llms.txt` (re-serialize the site map),
`.md`-suffix page URLs (near-lossless gemtext→Markdown at write time),
minimal Schema.org, a permissive-by-doctrine robots/AI posture, and
optionally a static A2A `agent-card.json` served *beside* the gemtext
capability page for operators who run an HTTP agent elsewhere. The
decision is how hard to court that audience vs. keep agent-friendliness a
Gemini-first byproduct.
