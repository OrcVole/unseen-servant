# Debugging

> Unseen Servant

Organised by symptom. For the agent-facing machine surfaces this uses,
`--json`, `USV_LOG_FORMAT`, exit codes: see
[`docs/agents.md`](docs/agents.md).

## First moves

```sh
usv status                 # config, fingerprints, roster, zones, published
usv status --json | jq .   # the same, parseable
usv check                  # is the config valid, is the content sane
RUST_LOG=usv=debug usv     # everything, on stderr
USV_LOG_FORMAT=json usv    # one JSON object per log line
```

`status` and `check` never render and never write. Logs go to stderr and
report output to stdout, so the two can always be separated.

## The app is up but Gemini is unreachable

Expected, if `GEMINI_PORT` is absent: that is a Cloudron admin having
disabled the TCP (Transmission Control Protocol) port. `usv` then serves the
HTTP (HyperText Transfer Protocol) surface only and stays healthy on purpose
(ADR 0008). The status page and the logs both say so. It is not a bug and
there is nothing to fix but the port setting.

Otherwise: check `usv status` for the listen addresses, and remember Gemini
cannot be reverse-proxied: it is TLS-native but not HTTP, so the port must
be passed through, not terminated upstream.

## A request is being refused and I want to know why

Every rejection comes from one of three layers, documented in
`src/protocol/mod.rs`:

| Layer | Owns | Rejects with |
|---|---|---|
| Framing (`protocol/request.rs`) | CRLF terminator, the 1024-byte budget, bare LF, stray CR | `59` |
| URI validation (`protocol/uri.rs`) | RFC 3986 parse; userinfo, fragments, non-ASCII, foreign schemes | `59` |
| Authority (`protocol/mod.rs`) | is this a host and port we serve | `53` |

The log line names the layer and the specific error variant, so a `59` never
leaves you guessing which rule fired.

`53` on a request that looks correct usually means the authority did not
match a configured `[[host]]`: including the case where a client connected
by IP address rather than by hostname.

## A client refuses the certificate

```sh
usv fingerprint          # what this capsule actually serves
openssl s_client -connect host:1965 </dev/null 2>/dev/null \
  | openssl x509 -noout -fingerprint -sha256 -dates -subject
```

If those two disagree, something in front of the server is terminating TLS
(Transport Layer Security). If they agree and the client still complains,
the client has an older certificate pinned, which is TOFU (trust on first
use) working, not failing. `usv` never silently regenerates a key, so a
changed fingerprint always has a cause worth finding before you tell anyone
to click through.

## A Titan upload is refused

The status code says which check failed:

| Status | Meaning |
|---|---|
| `60` | no client certificate presented |
| `62` | the certificate is expired or not yet valid |
| `61` | the certificate is valid but not authorised here |

`61` has three distinct causes: the fingerprint is not in the zone, the
identity does not hold `titan-write`, or a rotation window has closed. `usv
zones --json` and `usv status --json` show all three: capability grants are
enforced at request time, not at startup, so a zone may name an identity
that no longer holds the capability.

`59` on a Titan request is the request *line*: size, MIME (Multipurpose
Internet Mail Extensions), or a malformed token, not authorisation.

## Content changed but nothing was published

The watcher is debounced (300 ms) and renders the whole tree into a staging
directory before swapping it in atomically, so you see either the old tree
or the new one, never a half-written one.

```sh
usv stats --json    # what is currently in the rendered tree
usv render          # force one now, synchronously
```

Generated filenames (`map.gmi`, `feed.gmi`) are reserved and ignored by the
watcher: a render that triggered itself was a real bug once, and `usv
check` warns if you have authored a file at one of those names.

## Working on the code

```sh
cargo test                                    # unit + integration
cargo test --test wire                        # real sockets, real TLS
cargo test --test smoke                       # the CLI and process surface
cargo test <substring>                        # one test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo +nightly fuzz run frame_request_line    # needs cargo-fuzz
```

The wire suite spawns the real binary on a loopback port and prints the
exact bytes exchanged on failure, so a failing assertion tells you what went
over the socket rather than what a mock believed. Unit tests live in the
module they test; integration tests are in `tests/`.

Every parser has a fuzz target in `fuzz/`, each with a committed regression
corpus. A new parser lands with its fuzz target in the same change: that is
an invariant, not a preference.

## Fuzzing, and what a campaign has actually cost

Eight targets, one per parser, run from `fuzz/`. CI runs a 60-second
smoke per target — enough to catch a crash somebody just introduced,
not a campaign.

```sh
cargo +nightly fuzz run <target> -- -max_total_time=600 -print_final_stats=1
```

Extended campaign, 600 s per target, 2026-08-30, on the build
workstation. **1,864,674,573 executions, no crashes, no artefacts.**

| Target | Executions | exec/s | New corpus units | Coverage | Corpus |
|---|---:|---:|---:|---:|---:|
| `frame_request_line` | 829,169,308 | 1,379,649 | 6 | 50 | 32 |
| `validate_uri` | 300,261,445 | 499,603 | 182 | 380 | 409 |
| `config_parse` | 9,378,049 | 15,604 | 13,495 | 3,260 | 4,946 |
| `parse_gemtext` | 121,300,563 | 201,831 | 1,402 | 155 | 296 |
| `parse_titan` | 186,874,771 | 310,939 | 1,176 | 466 | 531 |
| `parse_gopher_selector` | 194,958,257 | 324,389 | 288 | 187 | 97 |
| `static_path_sanitize` | 217,792,149 | 112,263 | 79 | 104 | 91 |
| `render_html` | 4,940,031 | 8,219 | 3,628 | 417 | 648 |

**Read the "new units" column, not the executions column.** Executions
measure how cheap a target is to run, not how much it learned.
`frame_request_line` managed 829 million runs and found **six** new
inputs: it is saturated, and a longer campaign there buys nothing.
`config_parse` and `render_html` are three orders of magnitude slower
per execution — they parse TOML and build a whole document — and each
added thousands of new inputs, meaning the fuzzer was still finding
genuinely new paths when the clock ran out. Those two are where the next
campaign's hours belong.

A target that is still adding units at the end of its budget has not
finished; it has been interrupted. Say which when reporting a campaign.

## Conformance

```sh
gemini-diagnostics <host> <port>
```

27 checks. Three known results are tool or environment artifacts rather than
defects, re-confirmed across runs in C1 and C7:

| Check | Why it is not a defect |
|---|---|
| `TLSClaims` | The tool calls `ssl.match_hostname`, removed from Python's stdlib in 3.13. The claims it checks were verified independently with `openssl x509`. |
| `URLByIPAddress` | Reports `None`, not a failure. The check accepts either serving or refusing with `53`; `usv` refuses, which is spec-legitimate. |
| `IPv6Address` | Timed out from a workstation with no IPv6 egress at all (`curl -6` to a known-good host failed too). Needs re-running from a host with real IPv6 before it means anything. |

One real defect this suite found is worth knowing about, because the fix is
unusual: `TLSRequired` failed because rustls writes a TLS alert before
aborting a garbled handshake: a response, just not a Gemini one, and worse
than silence because it confirms a TLS stack is listening. The fix in
`server.rs` (`peek_looks_like_tls`) peeks the first byte before the acceptor
runs and drops anything that is not a TLS handshake record (`0x16`) with no
response at all.

## On Cloudron

```sh
cloudron logs -f --app <id>    # stream
cloudron exec --app <id>       # shell
```

The panel's file manager edits `/app/data`, which is the state directory,
identity, content, rendered output and config all live there. Back that one
directory up and you have backed up the capsule, including the certificate
readers have pinned.
