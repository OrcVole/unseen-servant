#!/bin/bash
# Cloudron entrypoint. Runs as root; every field choice here is cited in
# docs/internal/recon/cloudron-fit.md's hard-constraints checklist — read that
# before changing this file, not the house cloudron-app-packaging skill.
set -eu -o pipefail

echo "==> Fixing permissions"
# On every start, not just first run: backup/restore can reset ownership
# (docs/internal/recon/cloudron-fit.md §3), and this must never be conditional on
# whether /app/data looks "already set up".
chown -R cloudron:cloudron /app/data

export USV_STATE_DIR="/app/data"
# USV_HOSTNAME overrides *every* [[host]] a usv.toml file might define down
# to a single one named $CLOUDRON_APP_DOMAIN (config::Config::resolve's own
# documented behavior) — right for the common single-domain install, wrong
# the moment an operator adds a manifest multiDomain alias and writes a
# usv.toml with a [[host]] entry per hostname for real per-domain SNI
# vhosting. Found live, 2026-08-10: an alias domain got refused with 53
# until this was made conditional. A usv.toml already at /app/data means
# the operator has taken over host configuration explicitly; defer to it.
if [[ ! -f /app/data/usv.toml ]]; then
    export USV_HOSTNAME="${CLOUDRON_APP_DOMAIN}"
fi
# httpPort in CloudronManifest.json — keep the two in step if either changes.
export USV_HTTP_LISTEN="0.0.0.0:8000"

if [[ -n "${GEMINI_PORT:-}" ]]; then
    echo "==> Gemini enabled, external port ${GEMINI_PORT}"
    # containerPort is fixed at 1965 in the manifest (readOnly tcpPorts
    # entry), so this never actually diverges from GEMINI_PORT today — but
    # binding the container port and advertising the external port
    # separately is the correct shape in general (docs/internal/recon/cloudron-fit.md
    # §1), not an assumption that stays true only by accident.
    export USV_LISTEN="0.0.0.0:1965"
    export USV_ADVERTISED_PORT="${GEMINI_PORT}"
else
    # Absent GEMINI_PORT means the admin disabled the tcpPorts service.
    # USV_LISTEN="" is usv's own explicit "no Gemini listener" signal
    # (config::Config::listen's docs) — distinct from leaving it unset,
    # which would mean "use the default 1965" instead of "off".
    echo "==> Gemini disabled by platform config; HTTP-only"
    export USV_LISTEN=""
fi

if [[ -n "${GOPHER_PORT:-}" ]]; then
    # The admin enabled the gopher tcpPorts service. Same shape as
    # GEMINI_PORT: bind the fixed containerPort, advertise the external
    # one, since menus carry an absolute host:port and the platform may
    # map them differently.
    echo "==> Gopher enabled (CLEARTEXT), external port ${GOPHER_PORT}"
    export USV_GOPHER_LISTEN="0.0.0.0:7070"
    export USV_GOPHER_ADVERTISED_PORT="${GOPHER_PORT}"
else
    # Absent means the service is disabled (or was never enabled — it is
    # off by default). Explicitly empty, not merely unset, so it also
    # overrides a usv.toml that turns gopher on: the platform's switch
    # wins over the file, matching USV_LISTEN's semantics.
    echo "==> Gopher disabled by platform config"
    export USV_GOPHER_LISTEN=""
fi

echo "==> Starting usv"
# su-exec, not gosu: the base image is alpine now (see Dockerfile), and
# su-exec is its equivalent tiny setuid-drop tool — same CLI shape.
exec su-exec cloudron:cloudron /app/code/usv
