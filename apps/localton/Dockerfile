# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.97.1
ARG GO_VERSION=1.24.11
ARG UBUNTU_VERSION=24.04
ARG LOCALTON_BASE_IMAGE=ghcr.io/ton-blockchain/localton:sha-ede71005293920005bdc5639eac6c0d399d8c7a6

FROM rust:${RUST_VERSION}-bookworm AS launcher-builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY src/main.rs ./src/main.rs
COPY xtask/Cargo.toml ./xtask/Cargo.toml
COPY xtask/src/main.rs ./xtask/src/main.rs
RUN --mount=type=cache,id=localton-cargo-registry,target=/usr/local/cargo/registry \
    cargo fetch --locked

COPY src ./src
COPY xtask ./xtask
COPY assets ./assets
COPY NOTICE.md README.md ./
RUN --mount=type=cache,id=localton-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=localton-cargo-target,target=/src/target \
    cargo build --locked --release --workspace \
    && install -Dm755 target/release/localton /out/localton \
    && install -Dm755 target/release/xtask /out/xtask

# Development target: reuse the native TON and indexer layers from the published
# image and replace only the localton launcher built from this checkout.
FROM ${LOCALTON_BASE_IMAGE} AS localton-rust-only
USER root
COPY --from=launcher-builder --chmod=0755 /out/localton /usr/local/bin/localton
COPY --chmod=0755 docker/container-entrypoint.sh /usr/local/bin/localton-container-entrypoint
RUN install -d -o localton -g localton -m 0700 /var/lib/localton-snapshots \
    && install -d -o localton -g localton -m 0755 /var/lib/ton-indexer/account-scan
USER localton

FROM ubuntu:${UBUNTU_VERSION} AS ton-binaries
ARG TARGETARCH
ARG TON_RELEASE=v2026.06

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        unzip \
    && rm -rf /var/lib/apt/lists/*

# Keep release filenames and checksums aligned with src/binaries/release.rs.
RUN set -eux; \
    case "${TARGETARCH}" in \
        amd64) \
            asset="ton-linux-x86_64.zip"; \
            sha256="15a252cbe49f700f52863a397c615283e150ec0fa72eb5d55893d3346bec8d04" \
            ;; \
        arm64) \
            asset="ton-linux-arm64.zip"; \
            sha256="f93dd78d907d47507b3b41f74a4fcee5fefb5b164c3df1469d436100dfd87a7a" \
            ;; \
        *) \
            echo "unsupported Docker target architecture: ${TARGETARCH}" >&2; \
            exit 1 \
            ;; \
    esac; \
    curl --fail --location --retry 5 \
        "https://github.com/ton-blockchain/ton/releases/download/${TON_RELEASE}/${asset}" \
        --output /tmp/ton.zip; \
    echo "${sha256}  /tmp/ton.zip" | sha256sum --check --strict; \
    mkdir -p /opt/ton; \
    unzip -q /tmp/ton.zip -d /opt/ton; \
    rm /tmp/ton.zip; \
    runtime_dir=/opt/ton-runtime; \
    mkdir -p "${runtime_dir}"; \
    for binary in \
        create-state \
        dht-server \
        fift \
        generate-random-id \
        lite-client \
        validator-engine \
        validator-engine-console; \
    do \
        install -m 0755 "/opt/ton/${binary}" "${runtime_dir}/${binary}"; \
    done; \
    cp -a /opt/ton/lib /opt/ton/smartcont "${runtime_dir}/"; \
    rm -rf /opt/ton; \
    mv "${runtime_dir}" /opt/ton; \
    chmod -R a+rX /opt/ton; \
    test -d /opt/ton/lib; \
    test -d /opt/ton/smartcont; \
    APPIMAGE_EXTRACT_AND_RUN=1 \
        /opt/ton/generate-random-id -m keys -n 1 >/dev/null

FROM ubuntu:${UBUNTU_VERSION} AS http-api-builder
ARG HTTP_API_BUILD_JOBS=4

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        autoconf \
        automake \
        build-essential \
        ca-certificates \
        ccache \
        clang \
        clang-format \
        clang-tools \
        cmake \
        curl \
        g++ \
        gcc \
        git \
        gperf \
        libabsl-dev \
        libbenchmark-dev \
        libblas-dev \
        libboost-context1.83-dev \
        libboost-coroutine1.83-dev \
        libboost-filesystem1.83-dev \
        libboost-iostreams1.83-dev \
        libboost-locale1.83-dev \
        libboost-program-options1.83-dev \
        libboost-regex1.83-dev \
        libboost-stacktrace1.83-dev \
        libboost1.83-dev \
        libbson-dev \
        libbz2-dev \
        libc-ares-dev \
        libcctz-dev \
        libcrypto++-dev \
        libcurl4-openssl-dev \
        libdouble-conversion-dev \
        libev-dev \
        libfmt-dev \
        libgflags-dev \
        libgmock-dev \
        libgsl-dev \
        libgslcblas0 \
        libgtest-dev \
        libhiredis-dev \
        libidn11-dev \
        libjemalloc-dev \
        libjemalloc2 \
        libkrb5-dev \
        libldap2-dev \
        liblz4-dev \
        libmicrohttpd-dev \
        libnghttp2-dev \
        libpugixml-dev \
        libre2-dev \
        libsasl2-dev \
        libsodium-dev \
        libsnappy-dev \
        libssh2-1-dev \
        libssl-dev \
        libtool \
        libxxhash-dev \
        libyaml-cpp0.8 \
        libyaml-cpp-dev \
        liblzma-dev \
        libzstd-dev \
        lsb-release \
        netbase \
        ninja-build \
        openssl \
        pkg-config \
        python3-dev \
        python3-jinja2 \
        python3-venv \
        python3-yaml \
        ragel \
        software-properties-common \
        gnupg \
        wget \
        yasm \
        yq \
        zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

ENV CC=clang
ENV CXX=clang++
ENV BUILD_TON_PLAYGROUND=0
ENV CCACHE_COMPRESS=true
ENV CCACHE_MAXSIZE=4G
ENV USERVER_FEATURE_STACK_USAGE_MONITOR=0

COPY --from=launcher-builder /out/localton /usr/local/bin/localton
COPY --from=launcher-builder /out/xtask /usr/local/bin/xtask

RUN --mount=type=cache,id=ton-http-api-v2-ccache,target=/tmp/http-api-build/tools/ton-http-api-v2/ccache \
    xtask build-ton-http-api-v2 \
        --state-dir /tmp/http-api-build \
        --jobs "${HTTP_API_BUILD_JOBS}" \
    && mkdir -p /opt/ton-http-api-v2 \
    && cp -a /tmp/http-api-build/tools/ton-http-api-v2/install/. /opt/ton-http-api-v2/ \
    && strip --strip-unneeded /opt/ton-http-api-v2/bin/ton-http-api-v2 \
    && rm -rf \
        /tmp/http-api-build/tools/ton-http-api-v2/build \
        /tmp/http-api-build/tools/ton-http-api-v2/install \
        /tmp/http-api-build/tools/ton-http-api-v2/source

FROM golang:${GO_VERSION}-bookworm AS go-toolchain

FROM ubuntu:${UBUNTU_VERSION} AS ton-indexer-builder
ARG TON_INDEXER_BUILD_JOBS=4
ARG TON_INDEXER_COMMIT=eb9fbfa3212a583d3eef672f74b98600dfdd898c
ARG TON_INDEXER_REPOSITORY=https://github.com/toncenter/ton-indexer.git

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        autoconf \
        automake \
        build-essential \
        ca-certificates \
        ccache \
        clang \
        cmake \
        curl \
        git \
        gperf \
        libcurl4-openssl-dev \
        libhiredis-dev \
        libjemalloc-dev \
        liblz4-dev \
        libmicrohttpd-dev \
        libpq-dev \
        libreadline-dev \
        libsecp256k1-dev \
        libsodium-dev \
        libssl-dev \
        libtool \
        ninja-build \
        openssl \
        pkg-config \
        python3-dev \
        python3-pip \
        python3-venv \
        wget \
        zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

ENV CC=clang \
    CXX=clang++ \
    CCACHE_COMPRESS=true \
    CCACHE_DIR=/ccache \
    CCACHE_MAXSIZE=8G

COPY docker/ton-indexer-cors.patch /tmp/ton-indexer-cors.patch
COPY docker/ton-indexer-classifier-cpu.patch /tmp/ton-indexer-classifier-cpu.patch
COPY docker/ton-indexer-v3-catchup.patch /tmp/ton-indexer-v3-catchup.patch
COPY docker/ton-indexer-localton-scanner.patch /tmp/ton-indexer-localton-scanner.patch

RUN git init -q /src/ton-indexer \
    && git -C /src/ton-indexer remote add origin "${TON_INDEXER_REPOSITORY}" \
    && git -C /src/ton-indexer fetch --depth 1 origin "${TON_INDEXER_COMMIT}" \
    && git -C /src/ton-indexer checkout --detach FETCH_HEAD \
    && git -C /src/ton-indexer submodule update --init --recursive --depth 1 \
    && test "$(git -C /src/ton-indexer rev-parse HEAD)" = "${TON_INDEXER_COMMIT}" \
    && git -C /src/ton-indexer apply \
        /tmp/ton-indexer-cors.patch \
        /tmp/ton-indexer-classifier-cpu.patch \
        /tmp/ton-indexer-v3-catchup.patch \
        /tmp/ton-indexer-localton-scanner.patch

RUN --mount=type=cache,id=ton-indexer-ccache,target=/ccache \
    cmake \
        -S /src/ton-indexer/ton-index-worker \
        -B /src/ton-indexer/build-worker \
        -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_C_COMPILER_LAUNCHER=ccache \
        -DCMAKE_CXX_COMPILER_LAUNCHER=ccache \
        -DPORTABLE=1 \
        -DTON_ARCH= \
    && cmake --build /src/ton-indexer/build-worker \
        --parallel "${TON_INDEXER_BUILD_JOBS}" \
        --target \
            ton-index-postgres \
            ton-index-postgres-migrate \
            ton-smc-scanner \
            ton-marker \
            ton-marker-cli \
            ton-marker-core \
    && install -d /out/ton-indexer/bin /out/ton-indexer/lib \
    && install -m 0755 \
        /src/ton-indexer/build-worker/ton-index-postgres/ton-index-postgres \
        /src/ton-indexer/build-worker/ton-index-postgres/ton-index-postgres-migrate \
        /src/ton-indexer/build-worker/ton-smc-scanner/ton-smc-scanner \
        /out/ton-indexer/bin/ \
    && cp -a \
        /src/ton-indexer/build-worker/ton-marker/libton-marker* \
        /out/ton-indexer/lib/

FROM ton-indexer-builder AS ton-index-api-builder
COPY --from=go-toolchain /usr/local/go /usr/local/go
ENV PATH=/usr/local/go/bin:/root/go/bin:${PATH}
WORKDIR /src/ton-indexer/ton-index-go

RUN install -m 0644 \
        /src/ton-indexer/ton-index-worker/ton-marker/src/wrapper.h \
        /usr/local/include/wrapper.h \
    && cp -a /out/ton-indexer/lib/libton-marker* /usr/lib/ \
    && ldconfig

RUN --mount=type=cache,id=ton-indexer-go-build,target=/root/.cache/go-build \
    --mount=type=cache,id=ton-indexer-go-mod,target=/go/pkg/mod \
    go install github.com/swaggo/swag/cmd/swag@v1.16.3 \
    && swag init \
    && CGO_ENABLED=1 go build \
        -trimpath \
        -ldflags="-s -w -buildid=" \
        -o /out/ton-indexer/bin/ton-index-go \
        ./main.go

FROM ton-indexer-builder AS ton-index-classifier-builder
RUN --mount=type=cache,id=ton-indexer-pip,target=/root/.cache/pip \
    python3 -m venv /out/ton-indexer/venv \
    && /out/ton-indexer/venv/bin/pip install \
        -r /src/ton-indexer/indexer/requirements.txt \
    && cp -a /src/ton-indexer/indexer /out/ton-indexer/classifier

FROM ubuntu:${UBUNTU_VERSION} AS runtime
ARG BUILD_VERSION=dev
ARG SOURCE_URL=https://github.com/ton-blockchain/acton
ARG VCS_REF=unknown

LABEL org.opencontainers.image.title="localton" \
      org.opencontainers.image.description="Headless local TON network with TON HTTP API v2 and indexed API v3" \
      org.opencontainers.image.source="${SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0" \
      org.opencontainers.image.version="${BUILD_VERSION}" \
      org.opencontainers.image.revision="${VCS_REF}"

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libatomic1 \
        libcctz2 \
        libcurl4 \
        libfmt9 \
        libhiredis1.1.0 \
        libicu74 \
        libpq5 \
        libsecp256k1-1 \
        libsodium23 \
        passwd \
        python3 \
        tini \
        zlib1g \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 localton \
    && useradd \
        --uid 10001 \
        --gid 10001 \
        --home-dir /var/lib/localton \
        --no-create-home \
        --shell /usr/sbin/nologin \
        localton \
    && install -d -o localton -g localton -m 0700 /var/lib/localton \
    && install -d -o localton -g localton -m 0700 /var/lib/localton-snapshots \
    && install -d -o localton -g localton -m 0755 /var/lib/ton-indexer/account-scan \
    && install -d -m 0755 /usr/share/doc/localton

COPY --from=launcher-builder /out/localton /usr/local/bin/localton
COPY --from=ton-binaries /opt/ton /opt/ton
COPY --from=http-api-builder /opt/ton-http-api-v2 /opt/ton-http-api-v2
COPY --from=ton-indexer-builder \
    /out/ton-indexer/bin/ton-index-postgres \
    /out/ton-indexer/bin/ton-index-postgres-migrate \
    /out/ton-indexer/bin/ton-smc-scanner \
    /opt/ton-indexer/bin/
COPY --from=ton-indexer-builder /out/ton-indexer/lib /opt/ton-indexer/lib
COPY --from=ton-index-api-builder \
    /out/ton-indexer/bin/ton-index-go \
    /opt/ton-indexer/bin/ton-index-go
COPY --from=ton-index-classifier-builder \
    /out/ton-indexer/venv \
    /opt/ton-indexer/venv
COPY --from=ton-index-classifier-builder \
    /out/ton-indexer/classifier \
    /opt/ton-indexer/classifier
COPY --from=ton-indexer-builder /src/ton-indexer/LICENSE /usr/share/doc/localton/TON-INDEXER-LICENSE
COPY --chmod=0755 docker/container-entrypoint.sh /usr/local/bin/localton-container-entrypoint
COPY NOTICE.md /usr/share/doc/localton/

ENV HOME=/var/lib/localton \
    APPIMAGE_EXTRACT_AND_RUN=1 \
    LD_LIBRARY_PATH=/opt/ton/lib:/opt/ton-indexer/lib \
    LOCALTON_CONFIG_HTTP_BIND=0.0.0.0 \
    LOCALTON_ADMIN_HTTP_BIND=0.0.0.0 \
    LOCALTON_HTTP_API_BIND=0.0.0.0 \
    LOCALTON_HTTP_API_COMMAND=/opt/ton-http-api-v2/bin/ton-http-api-v2 \
    LOCALTON_HTTP_API_STATIC_CONFIG=/opt/ton-http-api-v2/config/static_config.yaml \
    RUST_LOG=localton=info \
    TON_BIN_DIR=/opt/ton

WORKDIR /var/lib/localton
USER localton

VOLUME ["/var/lib/localton"]
EXPOSE 18000/tcp 18001/tcp 18002/tcp 18003/tcp 18004/tcp
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=10s --timeout=3s --start-period=240s --retries=12 \
    CMD curl --fail --silent --show-error \
        http://127.0.0.1:18002/api/v2/getMasterchainInfo >/dev/null || exit 1

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/localton-container-entrypoint"]
CMD ["bootstrap", "--state-dir", "/var/lib/localton", "--ton-http-api"]
