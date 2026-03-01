#!/usr/bin/env bash
set -euo pipefail

EXPORT_DIR="${1:-${LLAMA_EXPORT_DIR:-/export}}"
mkdir -p "${EXPORT_DIR}/bin"

# Binary
cp -f /usr/local/bin/llama-dashboard "${EXPORT_DIR}/bin/"

# DEB / RPM packages (built by the packager stage)
if [[ -d /packages ]] && ls /packages/*.deb /packages/*.rpm 2>/dev/null | head -1 >/dev/null; then
    mkdir -p "${EXPORT_DIR}/packages"
    cp -f /packages/*.deb /packages/*.rpm "${EXPORT_DIR}/packages/" 2>/dev/null || true
    echo "✓ Packages copied to: ${EXPORT_DIR}/packages/"
    ls -lh "${EXPORT_DIR}/packages/"
fi

echo "✓ Exported llama-dashboard (CPU) to: ${EXPORT_DIR}"
ls -lh "${EXPORT_DIR}/bin/"
