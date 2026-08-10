# From source, and running under systemd

## Building

```sh
git clone <repository-url> unseen-servant
cd unseen-servant
cargo build --release          # target/release/usv
```

The toolchain is pinned by `rust-toolchain.toml`; under `rustup` that is
honoured automatically. For a fully static, dependency-free binary — what
every distro package ships — build against musl instead:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Running it immediately

```sh
./target/release/usv
```

That is the whole setup. With no configuration file and an empty state
directory, `usv` mints a TOFU identity, writes a starter capsule, and
serves it (ADR 0008 — zero-configuration is a supported configuration,
not a degraded one). `usv init` walks through an interactive terminal
wizard instead, if you would rather answer questions than write TOML.

Useful read-only commands: `usv status`, `usv fingerprint`, `usv check`,
`usv zones`, `usv stats`.

## Running under systemd

A reference unit is provided at `packaging/systemd/usv.service`. The
distro packages install their own copy with the path retargeted; for a
source build:

```sh
sudo install -Dm755 target/release/usv /usr/local/bin/usv
sudo useradd --system --home-dir /var/lib/usv --create-home usv
sudo install -Dm644 packaging/systemd/usv.service /etc/systemd/system/usv.service
sudo systemctl daemon-reload
sudo systemctl enable --now usv
```

Config, if you want one, goes at `/var/lib/usv/usv.toml`
([`../configuration.md`](../configuration.md)).

```sh
sudo systemctl reload usv    # SIGHUP: re-reads config and certificates
```

Reload does not drop listeners, and an invalid configuration is rejected
with the previous one left running rather than taking the capsule down.

## What the unit hardens, and why

ADR 0002 chose a single memory-safe process over the multi-process
privilege separation that C servers like `gmid` use. The unit is where
that trade is paid back:

| Directive | Effect |
|---|---|
| `ProtectSystem=strict` | Entire filesystem read-only… |
| `ReadWritePaths=/var/lib/usv` | …except the one directory `usv` owns |
| `NoNewPrivileges=true` | No privilege escalation, ever |
| `CapabilityBoundingSet=` | *Empty.* Not "no extra capabilities" — none at all |
| `PrivateTmp`, `ProtectHome`, `ProtectKernelTunables`, `ProtectKernelModules`, `ProtectControlGroups` | Standard isolation for a service with no business touching any of it |
| `RestrictSUIDSGID`, `RestrictRealtime`, `LockPersonality`, `MemoryDenyWriteExecute` | Close the usual escalation routes |

The empty capability bounding set is possible because both ports (1965
and any HTTP surface) are above 1024, so `usv` never needs
`CAP_NET_BIND_SERVICE`. If you deliberately move `usv` to a privileged
port, that line is the one to revisit — prefer a redirect or a proxy.

Verified with `systemd-analyze verify`.
