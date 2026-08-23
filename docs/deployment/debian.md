---
title: "Debian and Ubuntu (`.deb`)"
description: "No APT repository exists yet (pre-release), so there is nothing to apt install by name. Build the package from the source tree:."
type: howto
status: decided
last_verified: 2026-08-11
---

# Debian and Ubuntu (`.deb`)

**No APT repository exists yet** (pre-release), so there is nothing to
`apt install` by name. Build the package from the source tree:

```sh
cargo build --release --target x86_64-unknown-linux-musl
packaging/deb/build.sh
sudo apt install ./target/distrib/unseen-servant_<version>_amd64.deb
```

`build.sh` needs `dpkg-deb` (`apt install dpkg-dev`). The binary is built
against musl and statically linked, so the package has no library
dependencies beyond a Linux kernel.

## What it installs

| Path | Contents |
|---|---|
| `/usr/bin/usv` | The binary |
| `/lib/systemd/system/usv.service` | Hardened unit (see below) |
| `/var/lib/usv` | State: config, identity, content, rendered output |
| `/usr/share/doc/unseen-servant/` | `README.md`, `LICENSE` |

`postinst` creates a dedicated `usv` system user, chowns `/var/lib/usv`,
then enables and starts the service. Because `usv` is zero-configuration
(ADR 0008), starting immediately is safe: a fresh `/var/lib/usv` gets an
auto-minted TOFU identity and a starter capsule rather than a
half-configured daemon.

## Removing versus purging

This distinction is deliberate and matches the "never silently lose the
keypair" posture of ADR 0003:

```sh
sudo apt remove unseen-servant   # keeps /var/lib/usv and the usv user
sudo apt purge  unseen-servant   # deletes both
```

Reinstalling after `remove` recovers the same capsule and the same
identity, so readers who pinned your certificate are undisturbed.

## After installing

```sh
systemctl status usv
sudo -u usv usv status          # config, fingerprints, zones, published pages
sudo -u usv usv fingerprint     # what readers will pin
journalctl -u usv -f            # logs (usv logs to stdout/stderr only)
```

Configuration is one file at `/var/lib/usv/usv.toml`: see
[`../configuration.md`](../configuration.md). It does not need to exist;
every setting has a working default.

```sh
sudo systemctl reload usv       # SIGHUP: re-reads config + certs, keeps listeners
```

An invalid edit is rejected and the old configuration stays live rather
than taking the server down.

## Verified

The full install → run → remove → purge cycle was exercised in a real
Debian container before this package was considered done, including
confirming that `remove` preserves `/var/lib/usv` and `purge` removes it.
