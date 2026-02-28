#!/usr/bin/env bash
set -euo pipefail

EXPORT_DIR="${1:-${LLAMA_EXPORT_DIR:-/export}}"
mkdir -p "${EXPORT_DIR}/bin" "${EXPORT_DIR}/lib"

#  Binary
cp -f /usr/local/bin/llama-dashboard "${EXPORT_DIR}/bin/"

#  ROCm shared libraries required at runtime
ldd /usr/local/bin/llama-dashboard 2>/dev/null \
  | awk '/=> \/opt\/rocm/ { print $3 }' \
  | sort -u | while read -r lib; do
    [[ -z "${lib}" ]] && continue
    real="$(readlink -f "${lib}" || true)"
    [[ -z "${real}" || ! -f "${real}" ]] && continue
    cp -f "${real}" "${EXPORT_DIR}/lib/"
    lib_base="$(basename "${lib}")"
    real_base="$(basename "${real}")"
    [[ "${lib_base}" != "${real_base}" ]] && \
        ln -sf "${real_base}" "${EXPORT_DIR}/lib/${lib_base}"
done

# rocBLAS kernel objects (required by rocblas at runtime)
if [[ -d /opt/rocm/lib/rocblas ]]; then
    cp -af /opt/rocm/lib/rocblas "${EXPORT_DIR}/lib/"
fi

#  DEB / RPM packages (built by the packager stage)
if [[ -d /packages ]] && ls /packages/*.deb /packages/*.rpm 2>/dev/null | head -1 >/dev/null; then
    mkdir -p "${EXPORT_DIR}/packages"
    cp -f /packages/*.deb /packages/*.rpm "${EXPORT_DIR}/packages/" 2>/dev/null || true
    echo "✓ Packages copied to: ${EXPORT_DIR}/packages/"
    ls -lh "${EXPORT_DIR}/packages/"
fi

echo "✓ Exported llama-dashboard to: ${EXPORT_DIR}"
ls -lh "${EXPORT_DIR}/bin/"
