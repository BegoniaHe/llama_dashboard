#!/usr/bin/env bash
# Wrapper script for llama-dashboard (ROCm build).
# Installed to /usr/bin/llama-dashboard by the DEB/RPM package.
#
# - Automatically sets LD_LIBRARY_PATH for bundled ROCm libraries
# - Sources /etc/llama-dashboard/env for GPU-specific overrides

set -euo pipefail

# Source user-editable environment configuration
if [[ -f /etc/llama-dashboard/env ]]; then
    set -a
    # shellcheck source=/dev/null
    source /etc/llama-dashboard/env
    set +a
fi

LIB_DIR="/usr/lib/llama-dashboard"
export LD_LIBRARY_PATH="${LIB_DIR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

exec "${LIB_DIR}/llama-dashboard" "$@"
