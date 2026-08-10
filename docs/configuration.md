# Configuration

One TOML file (ADR 0007), read from the state directory as
`usv.toml`. **It does not need to exist.** Every setting has a working
default, and a capsule with no configuration file at all is a supported
configuration, not a degraded one (ADR 0008).

Unknown keys are a **startup error**, not a warning. A typo in a
security-relevant setting must not fail open by being silently ignored.

`systemctl reload usv` (SIGHUP) re-reads this file and the certificates
without dropping listeners. If the new file is invalid, the previous
configuration stays live and the error is logged — an edit can never take
the capsule down.

## A minimal file

Nothing here is required; this is what a typical single-capsule config
looks like once you've made a couple of choices.

```toml
[server]
http_listen = "0.0.0.0:8000"
theme = "midnight"
lang = "en"

[[host]]
name = "example.org"
```

## `[server]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `state_dir` | path | platform default | Where identity, content and rendered output live |
| `listen` | list of addresses | `["0.0.0.0:1965", "[::]:1965"]` | Gemini listeners. **`[]` means "no Gemini listener"** — explicitly off, distinct from unset |
| `advertised_port` | integer | derived from the listener | Port used when building links, for when the public port differs from the bound one |
| `advertised_host` | string | first `[[host]]` | Hostname used in generated absolute URLs. Accepts onion addresses |
| `http_listen` | address | *unset — surface off* | The web mirror's listener |
| `theme` | string | `daybreak` | `daybreak`, `midnight`, `tokyo-night`, `paper`. Unknown names are a startup error listing the real ones |
| `lang` | string | `en` | Emitted as `<html lang>` and as the `text/gemini; lang=` parameter |
| `tls_min` | string | `1.3` | Set `1.2` only if you must serve an old client |
| `log_peer` | string | `off` | How much of a visitor's address the request log carries: `off` (nothing — the default), `hashed` (per-boot-salted digest, correlates within one run only), `full` (verbatim) |
| `max_connections` | integer | — | Concurrent connection ceiling |
| `request_timeout_secs` | integer | — | Slowloris defence |
| `response_timeout_secs` | integer | — | Write-side timeout |

### Visitor addresses in logs

`log_peer` defaults to `off`, so a capsule you never configure logs no
visitor addresses at all — matching Geminispace's log-minimalism norm
rather than web-server habit. The query string is redacted by
construction in every mode, since status `10`/`11` input lands there.

```toml
[server]
log_peer = "hashed"    # correlate repeat visits within one run, keep nothing
```

`hashed` digests the address only, never the ephemeral source port, and
the salt is regenerated at every start and never written down. A
mistyped value is a startup error rather than a silent fallback.

## `[[host]]` — one block per hostname

Multiple blocks give SNI virtual hosting from a single process, each
hostname with its own certificate.

```toml
[[host]]
name = "example.org"
docroot = "/var/lib/usv/content"

[[host.redirect]]
pattern = "^/old/(.*)$"
target  = "/new/$1"
permanent = true

[[host.cert_zone]]
path_prefix = "/private/"
fingerprints = ["sha256-hex…"]
```

- **`redirect`** — regex with capture groups. `permanent` chooses status
  `31` over `30`. Single hop; chains are not followed.
- **`cert_zone`** — read gating by client-certificate fingerprint,
  Molly Brown's model. An **empty** `fingerprints` list means "any valid
  client certificate", which is a legitimate "prove you have an identity"
  gate. Unlisted certificates get `61`; absent ones get `60`.

## Titan uploads

Writable zones are per-host, because writable paths belong to a host's
content tree.

```toml
[titan]
max_upload_bytes = 10485760        # server-wide default (10 MiB)

[[host.titan_zone]]
path_prefix = "/uploads/"
fingerprints = ["sha256-hex…"]     # or identities = ["laptop"]
max_upload_bytes = 1048576
mime = ["text/gemini"]
allow_delete = false
```

**An empty allowlist here is a startup error, never "anyone".** A
writable zone with no `fingerprints` and no `identities` would let anyone
able to mint a self-signed certificate — which is anyone — write to your
capsule. `usv` refuses to start rather than come up in that state. This
is the one place where the `cert_zone` convention is deliberately
inverted, and the asymmetry is intentional: reading and writing do not
deserve the same default.

`token` adds a shared secret alongside the certificate gate. It is
weaker (it rides in the URL) and is never a substitute for the
fingerprint allowlist.

## `[[identity]]` — the roster

Names for fingerprints, so zone configuration reads in labels rather than
64 hex characters (ADR 0011).

```toml
[[identity]]
label = "laptop"
fingerprint = "sha256-hex…"
capabilities = ["titan"]
enrolled = "2026-08-10"

superseded = ["old-sha256-hex…"]   # key rotation, both valid…
superseded_until = "2026-09-01"    # …until this day, then self-closing
```

Rotation windows close themselves. `superseded` without
`superseded_until` is a startup error, so there is no way to leave an old
key valid forever by forgetting about it.

`usv` records *when* a key was enrolled, never *who* it belongs to —
continuity, not attestation.

## Environment variables

For container and platform deployments where a file is awkward. They
override the file.

| Variable | Overrides |
|---|---|
| `USV_STATE_DIR` | `server.state_dir` |
| `USV_LISTEN` | `server.listen` — empty string means "Gemini off" |
| `USV_ADVERTISED_PORT` | `server.advertised_port` |
| `USV_HOSTNAME` | Collapses hosts to this single name |
| `USV_HTTP_LISTEN` | `server.http_listen` |

`USV_HOSTNAME` overrides *every* `[[host]]` block down to one. That's
right for the common single-domain container install and wrong the moment
you want per-host SNI — which is why the Cloudron `start.sh` only sets it
when no `usv.toml` exists.

## Checking a file before trusting it

```sh
usv check      # validates config and lints the content tree
usv zones      # lists certificate and Titan zones as understood
usv status     # config, fingerprints, roster, zones, published pages
```
