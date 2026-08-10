#!/bin/bash
# Cloudron entrypoint. Runs as root; every field choice here is cited in
# docs/recon/cloudron-fit.md's hard-constraints checklist — read that
# before changing this file, not the house cloudron-app-packaging skill.
set -eu -o pipefail

echo "==> Fixing permissions"
# On every start, not just first run: backup/restore can reset ownership
# (docs/recon/cloudron-fit.md §3), and this must never be conditional on
# whether /app/data looks "already set up".
chown -R cloudron:cloudron /app/data

export USV_STATE_DIR="/app/data"
export USV_HOSTNAME="${CLOUDRON_APP_DOMAIN}"
# httpPort in CloudronManifest.json — keep the two in step if either changes.
export USV_HTTP_LISTEN="0.0.0.0:8000"

if [[ -n "${GEMINI_PORT:-}" ]]; then
    echo "==> Gemini enabled, external port ${GEMINI_PORT}"
    # containerPort is fixed at 1965 in the manifest (readOnly tcpPorts
    # entry), so this never actually diverges from GEMINI_PORT today — but
    # binding the container port and advertising the external port
    # separately is the correct shape in general (docs/recon/cloudron-fit.md
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

echo "==> Starting usv"
exec gosu cloudron:cloudron /app/code/usv
