# Fedora, RHEL and openSUSE (RPM)

**No RPM repository exists yet** (pre-release). Build the package from
the source tree:

```sh
cargo build --release --target x86_64-unknown-linux-musl
packaging/rpm/build.sh
sudo rpm -Uvh target/distrib/unseen-servant-<version>-1.*.x86_64.rpm
```

`build.sh` needs `rpmbuild` and the systemd macros
(`dnf install rpm-build systemd-rpm-macros`). Like the `.deb`, the spec
packages an **already-built** static musl binary rather than compiling
inside `rpmbuild` — the binary is produced once, with the toolchain the
project pins, and every packaging format ships that same artifact.

## What it installs

| Path | Contents |
|---|---|
| `/usr/bin/usv` | The binary |
| `/usr/lib/systemd/system/usv.service` | Hardened unit |
| `/var/lib/usv` | State: config, identity, content, rendered output |
| `/usr/share/licenses/unseen-servant/` | `LICENSE` |
| `/usr/share/doc/unseen-servant/` | `README.md` |

`%pre` creates the `usv` group and user with plain `groupadd`/`useradd`
rather than the `sysusers.d` macros — availability and version of those
macros varies across Fedora, RHEL and openSUSE, while `useradd` is
universal. (The Arch package *does* use `sysusers.d`, which is idiomatic
there; see [`aur.md`](aur.md).)

## Uninstalling keeps your capsule

RPM has no `dpkg`-style purge distinction, so `/var/lib/usv` is
deliberately never listed in `%files`:

```sh
sudo rpm -e unseen-servant
```

leaves the `usv` user, `/var/lib/usv`, your content, and your TOFU
identity untouched. Nothing in a scriptlet deletes them. To remove the
capsule you delete the directory yourself, explicitly — the same
"never silently lose the keypair" posture as ADR 0003 and the `.deb`'s
`remove` case.

## After installing

```sh
sudo systemctl enable --now usv
sudo -u usv usv status
sudo -u usv usv fingerprint
journalctl -u usv -f
```

Configuration lives at `/var/lib/usv/usv.toml`; see
[`../configuration.md`](../configuration.md).

## Verified

Built inside a Fedora 41 container, installed with `rpm -Uvh`, confirmed
the user and `/var/lib/usv` were created with correct ownership, ran the
binary as that user and watched it mint its identity and serve, then
`rpm -e`'d it and confirmed the user and state directory both survived.

Two spec bugs were found by actually running `rpmbuild` rather than
reading the spec back: a `%{...}`-shaped token inside a comment tripped
rpm's macro-expanded-in-comment check, and `%license`/`%doc` with bare
filenames expect `%prep` to have populated the build directory — which
this spec has no reason to do, since the binary arrives prebuilt.
