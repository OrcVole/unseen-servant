# Cloudron package for usv. Constraints and every field choice here are
# recorded and cited in docs/recon/cloudron-fit.md — read that before
# touching this file, not the house cloudron-app-packaging skill (its
# base-image pin is stale; live docs win, see that recon doc's header).
#
# BASE IMAGE (revised live, 2026-08-10, see cloudron-fit.md's addendum):
# this used to be cloudron/base:5.1.0, at 2.46GB — almost entirely a huge
# apt-get layer (1.7GB) and a Node.js/yq toolchain (~650MB) that usv never
# touches; the actual payload (the usv binary) is 8.85MB. That size is
# what made even the *prebuilt-image* install slow: the CI push and the
# Cloudron-side pull both have to move ~2.46GB regardless of where the
# compile happens. Checked against the live Cloudron docs (not just the
# house skill or this repo's own prior recon): cloudron/base is a
# recommended convenience image, not a mandated one — the platform
# contract (filesystem paths, CLOUDRON_* env vars, health checks over
# HTTP, stdout/stderr logging) is enforced by the Cloudron runtime, not
# baked into that specific base. What usv's own start.sh needs from a
# base image is: a `cloudron` user at the conventional uid/gid 1000:1000,
# a gosu-equivalent privilege-drop tool, and a shell to run start.sh.
# `bash` is also installed explicitly (not just alpine's default ash) so
# the dashboard's web terminal / file manager — which exec `bash` into
# the container — keep working; conformance with the platform's admin
# conveniences matters even though usv itself has no in-app admin GUI.
# alpine supplies all of this (su-exec is gosu's tiny musl-friendly
# equivalent) at a few MB total instead of 2.46GB.

FROM rust:1.93.1 AS build
WORKDIR /build
RUN apt-get update -qq && apt-get install -y -qq musl-tools >/dev/null \
    && rustup target add x86_64-unknown-linux-musl
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
RUN cargo build --release --target x86_64-unknown-linux-musl --bin usv

FROM alpine:3.22
RUN apk add --no-cache su-exec ca-certificates bash \
    && addgroup -g 1000 cloudron \
    && adduser -D -H -u 1000 -G cloudron cloudron

COPY --from=build /build/target/x86_64-unknown-linux-musl/release/usv /app/code/usv
COPY start.sh /app/code/start.sh
RUN chmod +x /app/code/start.sh

CMD [ "/app/code/start.sh" ]
