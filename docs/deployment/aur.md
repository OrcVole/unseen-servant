---
title: "Arch Linux (AUR)"
description: "Not published to the AUR yet (pre-release). The PKGBUILD lives at packaging/aur/PKGBUILD and can be built directly:."
type: howto
status: decided
last_verified: 2026-08-11
---

# Arch Linux (AUR)

**Not published to the AUR yet** (pre-release). The `PKGBUILD` lives at
`packaging/aur/PKGBUILD` and can be built directly:

```sh
cd packaging/aur
makepkg -si
```

## It is a `-git` package

`unseen-servant-git` tracks `main` rather than a tagged release, because
no release is tagged yet. That is the correct AUR shape for a project in
this state. When v1.0 ships, this file becomes the template for a plain
`unseen-servant` package sourced from a release tarball: remove the
`source=`, `pkgver()` and `_srcname` VCS machinery and pin the version.

## What it installs

| Path | Contents |
|---|---|
| `/usr/bin/usv` | The binary |
| `/usr/lib/systemd/system/usv.service` | Hardened unit, retargeted to `/usr/bin` |
| `/usr/lib/sysusers.d/usv.conf` | Declares the `usv` system user |
| `/usr/share/licenses/unseen-servant-git/` | `LICENSE` |

The `usv` user is created by `systemd-sysusers` from the shipped
`sysusers.d` file: Arch's idiomatic mechanism, and the reason this
package needs no install scriptlet at all. (The RPM uses plain `useradd`
instead; see [`rpm.md`](rpm.md) for why.)

State lives in `/var/lib/usv`, as everywhere else.

## Two real build issues, and why the PKGBUILD looks like it does

Both were found by building it, not by reading it, and both are recorded
in comments in the file itself:

**`cargo build --frozen` fails on a clean machine.** `--frozen` disables
*all* network access, not merely "respect `Cargo.lock`", so the build
died with `no matching package named crossterm found`. The fix is a
`prepare()` step running `cargo fetch --locked`, which also makes the
network boundary provable rather than merely intended, since `prepare()`
is the only step permitted network access.

**`makepkg`'s default flags break `ring`.** The build produced dozens of
`undefined symbol: ring_core_*` link errors. Isolating one variable at a
time against a plain `cargo build` (which reproduced neither failure)
showed two independent causes in Arch's `makepkg.conf`: `CFLAGS` carries
`-flto=auto`, which turns `ring`'s hand-written C/assembly objects into
LTO bitcode a non-LTO Rust link cannot consume; and `LDFLAGS` carries
`-Wl,--as-needed`, which can drop the resulting static archive. Both
`build()` and `check()` therefore `unset LDFLAGS CFLAGS CXXFLAGS` first.
Distro hardening flags belong on distro-compiled C, not on the few bytes
of C a Rust crate's `build.rs` compiles internally.

## Verified

`makepkg -s` (which runs the full test suite in `check()`), then
`pacman -U`, confirming the `sysusers` hook created the `usv` user with
no postinst script, then removal.
