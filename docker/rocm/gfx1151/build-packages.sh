#!/usr/bin/env bash
# ============================================================
# build-packages.sh — Create DEB and RPM packages for
# llama-dashboard ROCm build.
#
# Expected environment / arguments:
#   BINARY       — path to the compiled llama-dashboard binary
#   EXPORT_DIR   — path containing lib/ with ROCm shared libs
#   VERSION      — package version  (default: 0.1.0)
#   GPU_TARGET   — AMDGPU target    (default: gfx1151)
#   OUT_DIR      — where to write the final .deb / .rpm
#
# Usage (inside Docker):
#   ./build-packages.sh
# ============================================================
set -euo pipefail

# ---------- tunables ----------
VERSION="${VERSION:-0.1.0}"
GPU_TARGET="${GPU_TARGET:-gfx1151}"
ARCH="amd64"

BINARY="${BINARY:-/usr/local/bin/llama-dashboard}"
LIB_SOURCE="${LIB_SOURCE:-/staging/lib}"
OUT_DIR="${OUT_DIR:-/packages}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_NAME="llama-dashboard-rocm-${GPU_TARGET}"
PKG_DIR="/tmp/${PKG_NAME}_${VERSION}_${ARCH}"

# ---------- helpers ----------
info()  { echo "==> $*"; }
die()   { echo "ERROR: $*" >&2; exit 1; }

[[ -f "${BINARY}" ]]      || die "Binary not found: ${BINARY}"
[[ -d "${LIB_SOURCE}" ]]  || die "Lib directory not found: ${LIB_SOURCE}"

mkdir -p "${OUT_DIR}"

# ============================================================
#  Build DEB
# ============================================================
build_deb() {
    info "Building DEB package …"

    rm -rf "${PKG_DIR}"
    mkdir -p "${PKG_DIR}"/{DEBIAN,usr/bin,usr/lib/llama-dashboard,etc/llama-dashboard,usr/lib/systemd/system}

    # -- control file --
    cat > "${PKG_DIR}/DEBIAN/control" <<EOF
Package: ${PKG_NAME}
Version: ${VERSION}
Section: science
Priority: optional
Architecture: ${ARCH}
Depends: libc6, libstdc++6
Recommends: rocm-hip-runtime
Maintainer: BegoniaHe <begonia@users.noreply.github.com>
Homepage: https://github.com/BegoniaHe/llama_dashboard
Description: Local LLM management platform powered by llama.cpp (ROCm ${GPU_TARGET})
 llama-dashboard provides a web-based UI for managing and running local
 LLMs via llama.cpp with AMD ROCm GPU acceleration.
 .
 This package bundles the required ROCm runtime libraries so a full
 ROCm installation is NOT required on the host.
EOF

    # -- conffiles (preserved on upgrade) --
    cat > "${PKG_DIR}/DEBIAN/conffiles" <<EOF
/etc/llama-dashboard/env
EOF

    # -- postinst --
    cat > "${PKG_DIR}/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
fi
EOF
    chmod 0755 "${PKG_DIR}/DEBIAN/postinst"

    # -- postrm --
    cat > "${PKG_DIR}/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
fi
EOF
    chmod 0755 "${PKG_DIR}/DEBIAN/postrm"

    # -- payload --
    install -m 0755 "${SCRIPT_DIR}/packaging/llama-dashboard-wrapper.sh" \
        "${PKG_DIR}/usr/bin/llama-dashboard"

    install -m 0755 "${BINARY}" \
        "${PKG_DIR}/usr/lib/llama-dashboard/llama-dashboard"

    # Copy ROCm shared libs
    find "${LIB_SOURCE}" -maxdepth 1 \( -name '*.so' -o -name '*.so.*' \) \
        -exec cp -a {} "${PKG_DIR}/usr/lib/llama-dashboard/" \;

    # Copy symlinks
    find "${LIB_SOURCE}" -maxdepth 1 -type l \
        -exec cp -a {} "${PKG_DIR}/usr/lib/llama-dashboard/" \;

    # Copy rocBLAS kernels if present
    if [[ -d "${LIB_SOURCE}/rocblas" ]]; then
        cp -a "${LIB_SOURCE}/rocblas" "${PKG_DIR}/usr/lib/llama-dashboard/"
    fi

    install -m 0644 "${SCRIPT_DIR}/packaging/llama-dashboard.env" \
        "${PKG_DIR}/etc/llama-dashboard/env"

    install -m 0644 "${SCRIPT_DIR}/packaging/llama-dashboard.service" \
        "${PKG_DIR}/usr/lib/systemd/system/llama-dashboard.service"

    # Build it
    dpkg-deb --build --root-owner-group "${PKG_DIR}" \
        "${OUT_DIR}/${PKG_NAME}_${VERSION}_${ARCH}.deb"

    info "DEB created: ${OUT_DIR}/${PKG_NAME}_${VERSION}_${ARCH}.deb"
    rm -rf "${PKG_DIR}"
}

# ============================================================
#  Build RPM
# ============================================================
build_rpm() {
    info "Building RPM package …"

    local rpmbuild_dir="/tmp/rpmbuild-${PKG_NAME}"
    rm -rf "${rpmbuild_dir}"
    mkdir -p "${rpmbuild_dir}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS,BUILDROOT}

    # Stage files in a separate directory (NOT inside BUILDROOT).
    # rpmbuild wipes BUILDROOT before %install, so we must copy from here.
    local staging="${rpmbuild_dir}/STAGING"
    mkdir -p "${staging}"/{usr/bin,usr/lib/llama-dashboard,etc/llama-dashboard,usr/lib/systemd/system}

    install -m 0755 "${SCRIPT_DIR}/packaging/llama-dashboard-wrapper.sh" \
        "${staging}/usr/bin/llama-dashboard"

    install -m 0755 "${BINARY}" \
        "${staging}/usr/lib/llama-dashboard/llama-dashboard"

    find "${LIB_SOURCE}" -maxdepth 1 \( -name '*.so' -o -name '*.so.*' \) \
        -exec cp -a {} "${staging}/usr/lib/llama-dashboard/" \;

    find "${LIB_SOURCE}" -maxdepth 1 -type l \
        -exec cp -a {} "${staging}/usr/lib/llama-dashboard/" \;

    if [[ -d "${LIB_SOURCE}/rocblas" ]]; then
        cp -a "${LIB_SOURCE}/rocblas" "${staging}/usr/lib/llama-dashboard/"
    fi

    install -m 0644 "${SCRIPT_DIR}/packaging/llama-dashboard.env" \
        "${staging}/etc/llama-dashboard/env"

    install -m 0644 "${SCRIPT_DIR}/packaging/llama-dashboard.service" \
        "${staging}/usr/lib/systemd/system/llama-dashboard.service"

    # Generate file list from the staging tree
    local filelist="${rpmbuild_dir}/filelist.txt"
    (cd "${staging}" && find . -not -type d | sed 's|^\./|/|') > "${filelist}"

    # Spec file — %install copies from the staging directory
    cat > "${rpmbuild_dir}/SPECS/${PKG_NAME}.spec" <<SPEC
%define __strip /bin/true

Name:           ${PKG_NAME}
Version:        ${VERSION}
Release:        1
Summary:        Local LLM management platform (ROCm ${GPU_TARGET})
License:        MIT
URL:            https://github.com/BegoniaHe/llama_dashboard
AutoReqProv:    no

%description
llama-dashboard provides a web-based UI for managing and running local
LLMs via llama.cpp with AMD ROCm GPU acceleration.

This package bundles the required ROCm runtime libraries so a full
ROCm installation is NOT required on the host.

%install
cp -a ${staging}/* %{buildroot}/

%files -f ${filelist}
%config(noreplace) /etc/llama-dashboard/env

%post
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
fi

%postun
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
fi
SPEC

    rpmbuild -bb \
        --define "_topdir ${rpmbuild_dir}" \
        "${rpmbuild_dir}/SPECS/${PKG_NAME}.spec"

    # Move output
    find "${rpmbuild_dir}/RPMS" -name '*.rpm' -exec cp {} "${OUT_DIR}/" \;
    info "RPM created in: ${OUT_DIR}/"

    rm -rf "${rpmbuild_dir}"
}

# ============================================================
#  Main
# ============================================================
info "Package: ${PKG_NAME}  Version: ${VERSION}  Arch: ${ARCH}"

build_deb

if command -v rpmbuild >/dev/null 2>&1; then
    build_rpm
else
    info "rpmbuild not found — skipping RPM (install 'rpm' package to enable)"
fi

info "Done! Packages in ${OUT_DIR}:"
ls -lh "${OUT_DIR}/"
