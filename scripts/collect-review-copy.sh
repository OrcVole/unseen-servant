#!/usr/bin/env bash
# Gather every page a reader or operator actually sees into one folder,
# so they can be reviewed together instead of hunted down one protocol
# at a time.
#
# The colophons are fetched FROM A RUNNING SERVER rather than rendered
# from source. That is the whole point: it is the served bytes that get
# reviewed, so a formatting bug that only appears on the wire cannot
# hide behind a tidy-looking source file.
#
# Usage:  scripts/collect-review-copy.sh [host] [output-dir]
# Needs:  a usv running with gemini/gopher/spartan/nex/finger enabled.

set -uo pipefail

HOST="${1:-localhost}"
OUT="${2:-docs/review-copy}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1

GEMINI_PORT=1965
GOPHER_PORT=7070
SPARTAN_PORT=3000
NEX_PORT=1900
FINGER_PORT=7979

mkdir -p "$OUT"

note() { printf '  %s\n' "$1"; }

echo "Collecting served pages from $HOST"

# --- Gemini: TLS, so openssl rather than nc. -servername is required,
# --- not optional: usv selects the host by SNI, and without it the
# --- handshake yields nothing at all (which looked exactly like "the
# --- listener is down" the first time this ran).
# --- Header line stripped into a sidecar so the MIME stays reviewable.
if command -v openssl >/dev/null; then
  printf 'gemini://%s/usv\r\n' "$HOST" |
    timeout 10 openssl s_client -quiet -verify_quiet \
      -servername "$HOST" -connect "$HOST:$GEMINI_PORT" \
      2>/dev/null >"$OUT/.gemini.raw"
  if [ -s "$OUT/.gemini.raw" ]; then
    head -1 "$OUT/.gemini.raw" | tr -d '\r' >"$OUT/colophon-gemini.header.txt"
    tail -n +2 "$OUT/.gemini.raw" >"$OUT/colophon-gemini.gmi"
    note "colophon-gemini.gmi"
  else
    note "gemini: NO RESPONSE (is the listener up?)"
  fi
  rm -f "$OUT/.gemini.raw"
else
  note "gemini: SKIPPED (no openssl)"
fi

# --- The cleartext four: plain TCP, one request line each.
fetch() { # name, port, request-bytes, outfile
  local name="$1" port="$2" req="$3" out="$4"
  printf '%b' "$req" | timeout 10 nc "$HOST" "$port" >"$OUT/$out" 2>/dev/null
  if [ -s "$OUT/$out" ]; then note "$out"; else note "$name: NO RESPONSE"; fi
}

fetch gopher  "$GOPHER_PORT"  '/usv\r\n'                    colophon-gopher.txt
fetch spartan "$SPARTAN_PORT" "$HOST /usv 0\r\n"            colophon-spartan.gmi
fetch nex     "$NEX_PORT"     '/usv\n'                      colophon-nex.gmi
fetch finger  "$FINGER_PORT"  '\r\n'                        finger-profile.txt

# Spartan prefixes a status header; keep it visible but out of the prose.
if [ -s "$OUT/colophon-spartan.gmi" ]; then
  head -1 "$OUT/colophon-spartan.gmi" | tr -d '\r' >"$OUT/colophon-spartan.header.txt"
  tail -n +2 "$OUT/colophon-spartan.gmi" >"$OUT/.sp" && mv "$OUT/.sp" "$OUT/colophon-spartan.gmi"
fi

# --- Operator-facing Cloudron copy. Not served to readers, but reviewed
# --- in the same pass because it is the other half of the first-run
# --- experience: the landing page sells it, the admin panel explains it.
cp -f DESCRIPTION.md "$OUT/cloudron-landing-page.md" && note "cloudron-landing-page.md"
cp -f POSTINSTALL.md "$OUT/cloudron-admin-panel.md" && note "cloudron-admin-panel.md"

echo "Collected into $OUT"
