#!/bin/bash
# Build an unseen-servant RPM from an already-built static musl binary.
# Run from the repo root:
#
#   cargo build --release --target x86_64-unknown-linux-musl
#   packaging/rpm/build.sh
#
# Needs rpmbuild (Fedora/RHEL: dnf install rpm-build systemd-rpm-macros).
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

mkdir -p "$WORK"/{BUILD,RPMS,SOURCES,SPECS,SRPMS,BUILDROOT}
install -m 755 "$BIN" "$WORK/SOURCES/usv"
install -m 644 packaging/systemd/usv.service "$WORK/SOURCES/usv.service"
# Retarget the reference unit's /usr/local/bin path to RPM's convention.
sed -i 's#/usr/local/bin/usv#/usr/bin/usv#' "$WORK/SOURCES/usv.service"
install -m 644 LICENSE "$WORK/SOURCES/LICENSE"
install -m 644 README.md "$WORK/SOURCES/README.md"

# _buildhost is pinned because rpm records the machine that built the
# package and publishes it in `rpm -qpi`. Whatever this happens to be
# built on is nobody's business, and this project's repositories carry no
# host detail by standing rule.
#
# %{?dist} is emptied deliberately. The payload is a static musl binary
# with no library dependencies, so it installs and runs on any RPM
# distribution; a name ending .fc43 would claim a Fedora 43 package and
# send an openSUSE or RHEL user looking for a build that does not need
# to exist. Whatever box happens to build it must not end up in its name.
rpmbuild --define "_topdir $WORK" --define "_usv_version ${VERSION}" \
         --define "dist %{nil}" \
         --define "_buildhost unseen-servant-build" \
    -bb packaging/rpm/usv.spec

mkdir -p target/distrib
find "$WORK/RPMS" -name '*.rpm' -exec cp {} target/distrib/ \;
echo "built: $(find target/distrib -name "unseen-servant-${VERSION}*.rpm")"
