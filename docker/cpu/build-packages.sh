#!/usr/bin/env bash
# ============================================================
# build-packages.sh — Create DEB and RPM packages for
# llama-dashboard CPU build.
#
# Expected environment:
#   BINARY   — path to the compiled llama-dashboard binary
#   VERSION  — package version  (default: 0.1.0)
#   OUT_DIR  — where to write the final .deb / .rpm
#
# Usage (inside Docker):
#   ./build-packages.sh
# ============================================================
set -euo pipefail

# ---------- tunables ----------
VERSION="${VERSION:-0.1.0}"
ARCH="amd64"

BINARY="${BINARY:-/usr/local/bin/llama-dashboard}"
OUT_DIR="${OUT_DIR:-/packages}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_NAME="llama-dashboard-cpu"
PKG_DIR="/tmp/${PKG_NAME}_${VERSION}_${ARCH}"

# ---------- helpers ----------
info() { echo "==> $*"; }
die()  { echo "ERROR: $*" >&2; exit 1; }

[[ -f "${BINARY}" ]] || die "Binary not found: ${BINARY}"

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
Depends: libc6, libstdc++6, libgomp1
Maintainer: BegoniaHe <begonia@users.noreply.github.com>
Homepage: https://github.com/BegoniaHe/llama_dashboard
Description: Local LLM management platform powered by llama.cpp (CPU build)
 llama-dashboard provides a web-based UI for managing and running local
 LLMs via llama.cpp. This build is CPU-only and requires no GPU drivers.
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

    local staging="${rpmbuild_dir}/STAGING"
    mkdir -p "${staging}"/{usr/bin,usr/lib/llama-dashboard,etc/llama-dashboard,usr/lib/systemd/system}

    install -m 0755 "${SCRIPT_DIR}/packaging/llama-dashboard-wrapper.sh" \
        "${staging}/usr/bin/llama-dashboard"

    install -m 0755 "${BINARY}" \
        "${staging}/usr/lib/llama-dashboard/llama-dashboard"

    install -m 0644 "${SCRIPT_DIR}/packaging/llama-dashboard.env" \
        "${staging}/etc/llama-dashboard/env"

    install -m 0644 "${SCRIPT_DIR}/packaging/llama-dashboard.service" \
        "${staging}/usr/lib/systemd/system/llama-dashboard.service"

    local filelist="${rpmbuild_dir}/filelist.txt"
    (cd "${staging}" && find . -not -type d | sed 's|^\./|/|') > "${filelist}"

    cat > "${rpmbuild_dir}/SPECS/${PKG_NAME}.spec" <<SPEC
%define __strip /bin/true

Name:           ${PKG_NAME}
Version:        ${VERSION}
Release:        1
Summary:        Local LLM management platform (CPU build)
License:        MIT
URL:            https://github.com/BegoniaHe/llama_dashboard
AutoReqProv:    no

%description
llama-dashboard provides a web-based UI for managing and running local
LLMs via llama.cpp. CPU-only build, no GPU drivers required.

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
