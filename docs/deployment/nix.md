# Nix

A flake is provided at the repository root.

```sh
nix build .#default     # binary at ./result/bin/usv
nix run   .#default     # build and run
nix develop             # dev shell: cargo, rustc, rustfmt, clippy, cargo-fuzz
```

The package is `pkgs.rustPlatform.buildRustPackage` with
`cargoLock.lockFile`, so dependencies come from `Cargo.lock` as
per-crate fixed-output derivations. The first build is slow and
I/O-heavy; subsequent ones reuse the store.

`fuzz/` is filtered out of the source: it carries its own separate
`Cargo.lock` and would otherwise drag an unrelated workspace into the
build's source hash.

## The sandboxed check phase runs unit tests only

`cargoTestFlags = [ "--lib" "--bins" ]`. The integration suite in
`tests/smoke.rs` is deliberately excluded from the *Nix build's* own
check phase, and this is worth understanding rather than working around:

Those tests spawn real `usv` subprocesses that bind a listener and wait
for it to come up. Under Nix's build sandbox — a restricted network
namespace — two of them hang indefinitely rather than failing fast, and
two others that only inspect a subprocess's exit code and stderr fail
outright. This was diagnosed rather than assumed: an initial run's
failures were first (wrongly) blamed on a stale concurrent build process,
but killing that and retrying alone reproduced the identical failures and
hang, and the same tests pass in `nix develop`'s interactive shell and in
every other packaging format's real-environment verification.

**No coverage is lost.** `.forgejo/workflows/pr-checks.yml` runs the full
`cargo test`, including `tests/smoke.rs`, on a real unsandboxed runner
for every push. The flag only narrows what the Nix build re-proves
internally.

## Verified

`nix build .#default` completes green with 423 unit tests passing inside
the sandbox, and the resulting `./result/bin/usv` was run outside the
sandbox — where the network restriction does not apply — and confirmed to
mint its identity, bind a listener, and serve.
