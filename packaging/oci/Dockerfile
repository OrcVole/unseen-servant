# Plain OCI image (ADR 0008 — standalone is a first-class profile, not a
# footnote): a distroless `scratch` image, not the Cloudron-specific one
# at the repo root (that one exists for cloudron/base's platform
# integration; this one carries none of that and is smaller for it).
#
# Build from the repo root so the build context includes src/:
#   podman build -f packaging/oci/Dockerfile -t usv .
#
# Run:
#   podman run -p 1965:1965 -v usv-data:/data -e USV_STATE_DIR=/data usv

FROM rust:1.93.1 AS build
WORKDIR /build
RUN apt-get update -qq && apt-get install -y -qq musl-tools >/dev/null \
    && rustup target add x86_64-unknown-linux-musl
# See the repo-root Dockerfile for why rust-toolchain.toml is deliberately
# not copied in — the base image's own default toolchain already matches
# it, and copying it in only invites a network call rustup doesn't need.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --target x86_64-unknown-linux-musl --bin usv

FROM scratch
COPY --from=build /build/target/x86_64-unknown-linux-musl/release/usv /usv
# No /etc/passwd on scratch — a numeric UID needs none; it's still a real
# non-root identity as far as the kernel's permission checks are concerned
# (ADR 0002: usv itself never asks to run as root).
USER 1000:1000
EXPOSE 1965
# The HTTP surface is opt-in (ADR 0008) — only meaningful if the operator
# also sets USV_HTTP_LISTEN and publishes a port for it; not exposed by
# default so a Gemini-only image doesn't advertise a surface it isn't
# serving.
ENTRYPOINT ["/usv"]
