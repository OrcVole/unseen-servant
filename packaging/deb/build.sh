#!/bin/bash
# Build unseen-servant_<version>_amd64.deb from an already-built static
# musl binary. Run from the repo root:
#
#   cargo build --release --target x86_64-unknown-linux-musl
#   packaging/deb/build.sh
#
# Needs dpkg-deb (Debian/Ubuntu: apt-get install dpkg-dev).
set -euo pipefail
cd "$(dirname "$0")/../.."

BIN="target/x86_64-unknown-linux-musl/release/usv"
if [[ ! -x "$BIN" ]]; then
    echo "error: $BIN not found — build it first (see this script's header)" >&2
    exit 1
fi

VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

PKG="$WORK/unseen-servant_${VERSION}_amd64"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$PKG/lib/systemd/system" \
    "$PKG/usr/share/doc/unseen-servant"

install -m 755 "$BIN" "$PKG/usr/bin/usv"
install -m 644 packaging/deb/usv.service "$PKG/lib/systemd/system/usv.service"
install -m 644 LICENSE README.md "$PKG/usr/share/doc/unseen-servant/"
install -m 755 packaging/deb/postinst "$PKG/DEBIAN/postinst"
install -m 755 packaging/deb/postrm "$PKG/DEBIAN/postrm"

SIZE=$(du -sk "$PKG" | cut -f1)
sed -e "s/__VERSION__/${VERSION}/" -e "s/__SIZE__/${SIZE}/" \
    packaging/deb/control > "$PKG/DEBIAN/control"

mkdir -p target/distrib
dpkg-deb --build --root-owner-group "$PKG" \
    "target/distrib/unseen-servant_${VERSION}_amd64.deb"

echo "built target/distrib/unseen-servant_${VERSION}_amd64.deb"
