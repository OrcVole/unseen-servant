# Cloudron package for usv. Constraints and every field choice here are
# recorded and cited in docs/recon/cloudron-fit.md — read that before
# touching this file, not the house cloudron-app-packaging skill (its
# base-image pin is stale; live docs win, see that recon doc's header).

FROM rust:1.93.1 AS build
WORKDIR /build
# Deliberately NOT copying rust-toolchain.toml into the build stage: the
# `rust:1.93.1` image already ships exactly that rustc as its default
# toolchain, so the pin is already satisfied by the tag. Copying the file
# in anyway makes rustup try to reconcile components (rustfmt, clippy —
# needed for local dev, not for `cargo build`) against
# static.rust-lang.org, and that network call hangs on proving-grounds-
# style restricted-MTU networks (found live: a 30s timeout fetching
# channel-rust-1.93.1.toml.sha256 failed the very first server-side
# build). No fuzz/tests copied in either: the packaged image ships the
# release binary only, and cargo-deny/clippy/test already gated this
# commit before it reached packaging (CI, AGENTS.md).
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin usv

# cloudron/base:5.1.0, digest-pinned per docs/recon/cloudron-fit.md §6
# (the house skill's 5.0.0 pin is stale as of that recon).
FROM cloudron/base:5.1.0@sha256:1c0666c9abe9e2090d33686826d4e97769b799124573118d41e0d7485135748e

COPY --from=build /build/target/release/usv /app/code/usv
COPY start.sh /app/code/start.sh
RUN chmod +x /app/code/start.sh

CMD [ "/app/code/start.sh" ]
