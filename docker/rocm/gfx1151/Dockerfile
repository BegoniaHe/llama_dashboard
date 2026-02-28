# ============================================================
# Multi-stage build: llama_dashboard with ROCm (HIP) support
# ============================================================
#
# Build:
#   docker compose -f docker/rocm/docker-compose.yml build
#
# Export binary to host:
#   docker compose -f docker/rocm/docker-compose.yml up
#
# Customise GPU target (default gfx1151):
#   AMDGPU_TARGETS=gfx1100 docker compose … build
# ============================================================

#  Stage 1: Build llama.cpp with ROCm/HIP
FROM rocm/dev-ubuntu-24.04:7.2-complete AS llama-builder

ENV DEBIAN_FRONTEND=noninteractive \
    ROCM_PATH=/opt/rocm \
    HIP_PATH=/opt/rocm

RUN apt-get update && apt-get install -y --no-install-recommends \
        git cmake build-essential clang ca-certificates \
 && rm -rf /var/lib/apt/lists/*

ARG LLAMA_CPP_REPO=https://github.com/ggml-org/llama.cpp.git
ARG LLAMA_CPP_REF=master
ARG AMDGPU_TARGETS=gfx1151

RUN git clone ${LLAMA_CPP_REPO} --depth 1 --branch ${LLAMA_CPP_REF} /src/llama.cpp

RUN cmake -S /src/llama.cpp -B /build/llama.cpp \
      -DGGML_HIP=ON \
      -DAMDGPU_TARGETS=${AMDGPU_TARGETS} \
      -DCMAKE_HIP_COMPILER_ROCM_ROOT=/opt/rocm \
      -DCMAKE_HIP_FLAGS="--rocm-path=/opt/rocm" \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_SHARED_LIBS=OFF \
      -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
      -DLLAMA_BUILD_SERVER=OFF \
      -DLLAMA_BUILD_TESTS=OFF \
      -DLLAMA_BUILD_EXAMPLES=OFF \
      -DLLAMA_BUILD_TOOLS=OFF \
      -DLLAMA_BUILD_COMMON=OFF \
      -DCMAKE_C_FLAGS="-O3 -ffunction-sections -fdata-sections" \
      -DCMAKE_CXX_FLAGS="-O3 -ffunction-sections -fdata-sections" \
      -DCMAKE_INSTALL_PREFIX=/opt/llama-prebuilt \
 && cmake --build /build/llama.cpp --config Release -j"$(nproc)" \
 && cmake --install /build/llama.cpp

# Safety net: ensure all static libs are present in the install prefix
RUN find /build/llama.cpp -name '*.a' -exec cp -n {} /opt/llama-prebuilt/lib/ \;

#  Stage 2: Build Rust project
FROM rocm/dev-ubuntu-24.04:7.2-complete AS rust-builder

ENV DEBIAN_FRONTEND=noninteractive \
    ROCM_PATH=/opt/rocm

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake clang libclang-dev pkg-config \
        ca-certificates curl gnupg \
 && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy prebuilt llama.cpp libraries & headers from stage 1
COPY --from=llama-builder /opt/llama-prebuilt /opt/llama-prebuilt

# Copy project source (see .dockerignore – reference/ & target/ excluded)
COPY . /src/llama-dashboard
WORKDIR /src/llama-dashboard

# Build frontend
WORKDIR /src/llama-dashboard/frontend
RUN npm ci && npm run build
WORKDIR /src/llama-dashboard

# Build with prebuilt llama.cpp + ROCm feature + embedded frontend
ENV LLAMA_PREBUILT_DIR=/opt/llama-prebuilt
RUN cargo build --release -p llama-dashboard --no-default-features --features rocm,embed-frontend

#  Stage 3: Package builder — creates DEB & RPM from the compiled artifacts
FROM rocm/dev-ubuntu-24.04:7.2-complete AS packager

ENV DEBIAN_FRONTEND=noninteractive \
    ROCM_PATH=/opt/rocm

RUN apt-get update && apt-get install -y --no-install-recommends \
        rpm \
 && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary
COPY --from=rust-builder /src/llama-dashboard/target/release/llama-dashboard \
     /usr/local/bin/llama-dashboard

# Collect ROCm runtime libraries into /staging/lib (same logic as export script)
RUN mkdir -p /staging/lib \
 && ldd /usr/local/bin/llama-dashboard 2>/dev/null \
    | awk '/=> \/opt\/rocm/ { print $3 }' \
    | sort -u | while read -r lib; do \
        [ -z "${lib}" ] && continue; \
        real="$(readlink -f "${lib}" || true)"; \
        [ -z "${real}" ] || [ ! -f "${real}" ] && continue; \
        cp -f "${real}" /staging/lib/; \
        lib_base="$(basename "${lib}")"; \
        real_base="$(basename "${real}")"; \
        [ "${lib_base}" != "${real_base}" ] && \
            ln -sf "${real_base}" "/staging/lib/${lib_base}"; \
    done \
 && if [ -d /opt/rocm/lib/rocblas ]; then \
        cp -af /opt/rocm/lib/rocblas /staging/lib/; \
    fi

# Copy packaging scripts & templates
COPY docker/rocm/gfx1151/build-packages.sh      /src/docker/rocm/gfx1151/build-packages.sh
COPY docker/rocm/gfx1151/packaging/              /src/docker/rocm/gfx1151/packaging/
RUN chmod +x /src/docker/rocm/gfx1151/build-packages.sh

ARG AMDGPU_TARGETS=gfx1151

# Build the packages
RUN BINARY=/usr/local/bin/llama-dashboard \
    LIB_SOURCE=/staging/lib \
    GPU_TARGET=${AMDGPU_TARGETS} \
    VERSION=0.1.0 \
    OUT_DIR=/packages \
    /src/docker/rocm/gfx1151/build-packages.sh

#  Stage 4: Export / Runtime
FROM rocm/dev-ubuntu-24.04:7.2-complete AS runtime

ENV DEBIAN_FRONTEND=noninteractive \
    ROCM_PATH=/opt/rocm

# Copy the compiled binary
COPY --from=rust-builder /src/llama-dashboard/target/release/llama-dashboard \
     /usr/local/bin/llama-dashboard

# Copy the built packages
COPY --from=packager /packages /packages

# Export helper script — copies binary + required ROCm runtime libs + packages
COPY docker/rocm/gfx1151/export-llama-dashboard.sh /usr/local/bin/export-llama-dashboard
RUN chmod +x /usr/local/bin/export-llama-dashboard

# gfx1151 is not in ROCm's official support matrix → override
ENV HSA_OVERRIDE_GFX_VERSION=11.0.0

# Default: export artifacts to /export (bind-mount from host)
ENTRYPOINT ["/usr/local/bin/export-llama-dashboard"]
CMD ["/export"]
