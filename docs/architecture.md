# Architecture

## The one idea

One content tree, rendered **at write time** to every surface, served as
static files.

Almost everything else follows. A gemtext file changes; a debounced
watcher notices; the whole tree is re-rendered into a staging directory
and swapped atomically; Gemini clients and the web mirror both read the
result. There is no per-request conversion, so there is no request-time
code path to attack, nothing to cache-invalidate, and the rendered tree
is portable on its own — `usv export` hands you a folder that works with
no server behind it (which is what makes an OnionShare mirror trivial).

The closest prior art, Agate+, converts gemtext to HTML *per request*.
ADR 0004 chose the opposite deliberately.

## Shape

```
                    ┌─ gemtext ─→ Gemini clients (1965, own TLS)
content/*.gmi ─→ render ─┤
                    └─ HTML/Atom/gemsub/sitemap/llms.txt ─→ web mirror
```

A single process, several tokio tasks: listeners, the file watcher, the
signal loop. ADR 0002 rejected the multi-process privilege separation
that C servers such as `gmid` use — in Rust, inside a container, with
fuzzed parsers and rustls holding the keys, the process boundary buys
little that the type system and systemd do not. What *was* kept from
that design is the shape: one task owns log output, key material lives
inside rustls's config and is never handled by request code, and SIGHUP
reloads configuration and certificates without dropping listeners.

## Module map

| Module | Responsibility |
|---|---|
| `protocol/` | Wire layer: request-line framing, URI validation, response writing, Titan request lines, gopher selectors and menu writing. Pure, no I/O, all fuzzed. |
| `server.rs` | Accept loop, TLS, scheme dispatch, timeouts, per-request logging. |
| `handler/` | `Handler` trait. Static files (traversal-proof), redirects, certificate zones, Titan uploads, the admin wire resource. |
| `render/` | gemtext parser → metadata → HTML/Atom/gemsub/sitemap/markdown/llms emitters; themes; the first-run skeleton; the pipeline and its watcher. |
| `identity/` | TOFU keypair minting and loading, per hostname. Never silently regenerates. |
| `roster.rs` | Named client identities, capabilities, self-closing rotation windows. |
| `config/` | One TOML file, `deny_unknown_fields`, env overrides. |
| `cli.rs`, `init.rs` | Read-only subcommands and the ratatui setup wizard. |

The `Handler` trait is borrowed from Jetforce's app/handler split: static
serving is one handler among several rather than a special case, which is
what makes cert zones, redirects and Titan compose instead of accreting
conditionals.

## Size

Measured 2026-08-10 with `find src -name '*.rs' | xargs cat`:

| | Lines |
|---|---:|
| `src/` total | 15,523 |
| — code | 11,245 |
| — comments | 3,081 |
| — blank | 1,197 |
| `tests/` (integration) | 2,010 |

39 source files; 438 test functions across in-module test suites, plus
the integration suites.

Two caveats, because a bare LOC figure invites the wrong conclusion.
This is a **deliberately comment-heavy** codebase — roughly one comment
line per four of code — because the reasoning behind a decision is
expected to survive next to it rather than only in a commit message; and
the test functions are counted in the `src/` total, since Rust keeps unit
tests in the module they test. Neither number is a quality claim. They
are here so "small and auditable" is a checkable statement rather than a
slogan.

## Why not the alternatives

Recorded properly in [`adr/`](adr/); briefly:

- **No CGI/FastCGI/SCGI, no plugin API** (ADR 0005). Content is data,
  never code. The escape hatch other servers provide is the source of
  most of their defect load — the twins issue tracker is the evidence
  file.
- **No reverse proxying.** Same reason.
- **TOFU self-signed over CA certificates** (ADR 0003). CA rotation every
  60–90 days breaks clients that pinned the previous certificate, which
  is the failure mode TOFU exists to make visible.
- **TOML, not a bespoke config grammar.** gmid's config *semantics* are
  the best in the field; its yacc grammar is not worth reimplementing.

## Reading order for a new contributor

`src/lib.rs` (module map) → `protocol/` (the wire is small; the whole
Gemini transaction is about a page of logic) → `handler/mod.rs` (the
trait) → `render/pipeline.rs` (where the one idea above lives) →
[`adr/`](adr/) for why any of it is shaped this way.
