# Plain container image (OCI)

Distinct from the Cloudron image at the repository root: that one carries
Cloudron's platform integration, this one carries none and is smaller for
it. Standalone is a first-class profile, not a footnote (ADR 0008).

```sh
podman build -f packaging/oci/Dockerfile -t usv .    # build from the repo root
podman run -p 1965:1965 -v usv-data:/data -e USV_STATE_DIR=/data usv
```

Build from the repository root, not from `packaging/oci/`, so the build
context includes `src/`.

## What is in it

`FROM scratch` — the final image contains the statically linked musl
binary and nothing else. **8.77MB.** No shell, no package manager, no
libc, no CA bundle: nothing to exploit that isn't `usv` itself.

It runs as `USER 1000:1000`. There is no `/etc/passwd` on `scratch` and
none is needed — a numeric UID is a real, unprivileged identity as far as
the kernel's permission checks are concerned, which is all ADR 0002 asks
for.

Port 1965 is exposed. The HTTP surface is **not** exposed by default:
it's opt-in (ADR 0008), only meaningful if you also set
`USV_HTTP_LISTEN` and publish a port for it, and a Gemini-only image
should not advertise a surface it is not serving.

## State

Everything `usv` persists — identity, content, rendered output — lives
under `USV_STATE_DIR`. Mount a volume there or the capsule (and its TOFU
identity) is discarded when the container is removed.

```sh
podman run -d --name usv \
  -p 1965:1965 -p 8000:8000 \
  -e USV_STATE_DIR=/data \
  -e USV_HTTP_LISTEN=0.0.0.0:8000 \
  -e USV_HOSTNAME=example.org \
  -v usv-data:/data \
  usv
```

## Debugging a distroless image

There is no shell to `exec` into. Inspect the state volume from another
container instead:

```sh
podman run --rm -v usv-data:/data alpine ls -la /data /data/certs
```

Logs go to stdout/stderr as usual (`podman logs -f usv`).
