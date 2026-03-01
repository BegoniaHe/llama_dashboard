#!/usr/bin/env bash
# Wrapper script for llama-dashboard (CPU build).
# Installed to /usr/bin/llama-dashboard by the DEB/RPM package.

set -euo pipefail

# Source user-editable environment configuration
if [[ -f /etc/llama-dashboard/env ]]; then
    set -a
    # shellcheck source=/dev/null
    source /etc/llama-dashboard/env
    set +a
fi

exec /usr/lib/llama-dashboard/llama-dashboard "$@"
