# ============================================================
# Multi-stage build: llama_dashboard — CPU-only build
# ============================================================
#
# Build:
#   docker buildx build --load -t llama-dashboard-cpu:latest \
#     -f docker/cpu/Dockerfile .
#
# Export binary to host:
#   docker run --rm -v ./artifact:/export llama-dashboard-cpu:latest /export
# ============================================================

#  Stage 1: Build llama.cpp (CPU-only)
FROM ubuntu:24.04 AS llama-builder

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
        git cmake build-essential clang ca-certificates \
 && rm -rf /var/lib/apt/lists/*

ARG LLAMA_CPP_REPO=https://github.com/ggml-org/llama.cpp.git
ARG LLAMA_CPP_REF=master

RUN git clone ${LLAMA_CPP_REPO} --depth 1 --branch ${LLAMA_CPP_REF} /src/llama.cpp

RUN cmake -S /src/llama.cpp -B /build/llama.cpp \
      -DCMAKE_BUILD_TYPE=Release \
      -DBUILD_SHARED_LIBS=OFF \
      -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
      -DGGML_NATIVE=OFF \
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

# Safety net: ensure all static libs land in the install prefix
RUN find /build/llama.cpp -name '*.a' -exec cp -n {} /opt/llama-prebuilt/lib/ \;

#  Stage 2: Build Rust project
FROM ubuntu:24.04 AS rust-builder

ENV DEBIAN_FRONTEND=noninteractive

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

# Copy project source
COPY . /src/llama-dashboard
WORKDIR /src/llama-dashboard

# Build frontend
WORKDIR /src/llama-dashboard/frontend
RUN npm ci && npm run build
WORKDIR /src/llama-dashboard

# Build with prebuilt llama.cpp + embedded frontend
ENV LLAMA_PREBUILT_DIR=/opt/llama-prebuilt
RUN cargo build --release -p llama-dashboard --features embed-frontend

#  Stage 3: Package builder
FROM ubuntu:24.04 AS packager

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
        rpm dpkg \
 && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary
COPY --from=rust-builder /src/llama-dashboard/target/release/llama-dashboard \
     /usr/local/bin/llama-dashboard

# Copy packaging scripts & templates
COPY docker/cpu/build-packages.sh          /src/docker/cpu/build-packages.sh
COPY docker/cpu/packaging/                 /src/docker/cpu/packaging/

# Build the packages
RUN chmod +x /src/docker/cpu/build-packages.sh \
 && BINARY=/usr/local/bin/llama-dashboard \
    VERSION=0.1.0 \
    OUT_DIR=/packages \
    /src/docker/cpu/build-packages.sh

#  Stage 4: Export / Runtime
FROM ubuntu:24.04 AS runtime

ENV DEBIAN_FRONTEND=noninteractive

# Copy the compiled binary
COPY --from=rust-builder /src/llama-dashboard/target/release/llama-dashboard \
     /usr/local/bin/llama-dashboard

# Copy the built packages
COPY --from=packager /packages /packages

# Export helper script
COPY docker/cpu/export-llama-dashboard.sh /usr/local/bin/export-llama-dashboard
RUN chmod +x /usr/local/bin/export-llama-dashboard

# Default: export artifacts to /export (bind-mount from host)
ENTRYPOINT ["/usr/local/bin/export-llama-dashboard"]
CMD ["/export"]
