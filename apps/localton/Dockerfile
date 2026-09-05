# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.97.1
ARG GO_VERSION=1.24.11
ARG UBUNTU_VERSION=24.04
ARG LOCALTON_BASE_IMAGE=ghcr.io/ton-blockchain/localton:sha-fb807b4286149081379b337cc1f50165972384d7

FROM oven/bun:1.4.0 AS ui-builder
WORKDIR /repo
COPY package.json bun.lock bunfig.toml ./
COPY packages ./packages
RUN bun ci \
    && bun --filter @acton/localton-ui build

FROM rust:${RUST_VERSION}-bookworm AS localton-builder
WORKDIR /src/apps/localton
ARG VCS_REF=unknown

COPY Cargo.toml Cargo.lock /src/
COPY build.rs /src/build.rs
COPY src /src/src
COPY crates /src/crates
COPY xtask /src/xtask
COPY apps/localton/Cargo.toml apps/localton/Cargo.lock apps/localton/build.rs ./
COPY apps/localton/src/main.rs ./src/main.rs
COPY apps/localton/xtask/Cargo.toml ./xtask/Cargo.toml
COPY apps/localton/xtask/src/main.rs ./xtask/src/main.rs
COPY apps/localton/crates ./crates
RUN --mount=type=cache,id=localton-cargo-registry,target=/usr/local/cargo/registry \
    cargo fetch --locked

COPY apps/localton/src ./src
COPY apps/localton/xtask ./xtask
COPY apps/localton/docker ./docker
COPY apps/localton/assets ./assets
COPY apps/localton/NOTICE.md apps/localton/README.md ./
COPY --from=ui-builder /repo/packages/localton-ui/dist /src/packages/localton-ui/dist
RUN --mount=type=cache,id=localton-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=localton-cargo-target,target=/src/target \
    LOCALTON_GIT_HASH="${VCS_REF}" cargo build --locked --release --workspace \
    && install -Dm755 target/release/localton /out/localton \
    && install -Dm755 target/release/xtask /out/xtask

# Development target: reuse the native TON and indexer layers from the published
# image and replace only the localton binary built from this checkout.
FROM ${LOCALTON_BASE_IMAGE} AS localton-rust-only
USER root
COPY --from=localton-builder --chmod=0755 /out/localton /usr/local/bin/localton
COPY --chmod=0755 apps/localton/docker/container-entrypoint.sh /usr/local/bin/localton-container-entrypoint
RUN install -d -o localton -g localton -m 0700 /var/lib/localton-snapshots \
    && install -d -o localton -g localton -m 0755 /var/lib/ton-indexer/account-scan
USER localton

FROM ubuntu:${UBUNTU_VERSION} AS ton-binaries
ARG TARGETARCH
ARG TON_RELEASE=v2026.08

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
            sha256="548f0f29b5ebf1a42d6aa23438b25c7a0c05b2330a7f278d2ca83fc2015685e9" \
            ;; \
        arm64) \
            asset="ton-linux-arm64.zip"; \
            sha256="acb73cd85118754e149744d35c3ef4775f065c8545b49600f70ffe8dea68060e" \
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
ARG TON_HTTP_API_COMMIT
ARG TON_HTTP_API_REPOSITORY

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

COPY --from=localton-builder /out/localton /usr/local/bin/localton
COPY --from=localton-builder /out/xtask /usr/local/bin/xtask

RUN --mount=type=cache,id=ton-http-api-v2-ccache,target=/tmp/http-api-build/tools/ton-http-api-v2/ccache \
    xtask build-ton-http-api-v2 \
        --state-dir /tmp/http-api-build \
        --install-dir /opt/ton-http-api-v2 \
        --jobs "${HTTP_API_BUILD_JOBS}" \
    && strip --strip-unneeded /opt/ton-http-api-v2/bin/ton-http-api-v2 \
    && rm -rf \
        /tmp/http-api-build/tools/ton-http-api-v2/build \
        /tmp/http-api-build/tools/ton-http-api-v2/source

FROM golang:${GO_VERSION}-bookworm AS go-toolchain

FROM ubuntu:${UBUNTU_VERSION} AS ton-indexer-builder
ARG TON_INDEXER_BUILD_JOBS=4
ARG TON_INDEXER_COMMIT
ARG TON_INDEXER_REPOSITORY

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

COPY --from=localton-builder /out/xtask /usr/local/bin/xtask

RUN --mount=type=cache,id=ton-indexer-ccache,target=/ccache \
    xtask build-ton-http-api-v3 \
        --state-dir /tmp/indexer-build \
        --install-dir /out/ton-indexer \
        --component worker \
        --jobs "${TON_INDEXER_BUILD_JOBS}"

FROM ton-indexer-builder AS ton-index-api-builder
COPY --from=go-toolchain /usr/local/go /usr/local/go
ENV PATH=/usr/local/go/bin:/root/go/bin:${PATH}

RUN --mount=type=cache,id=ton-indexer-go-build,target=/root/.cache/go-build \
    --mount=type=cache,id=ton-indexer-go-mod,target=/go/pkg/mod \
    xtask build-ton-http-api-v3 \
        --state-dir /tmp/indexer-build \
        --install-dir /out/ton-indexer \
        --component api \
        --jobs "${TON_INDEXER_BUILD_JOBS}"

FROM ton-indexer-builder AS ton-index-classifier-builder
RUN --mount=type=cache,id=ton-indexer-pip,target=/root/.cache/pip \
    xtask build-ton-http-api-v3 \
        --state-dir /tmp/indexer-build \
        --install-dir /out/ton-indexer \
        --component classifier

# xtask owns compilation and installation; prepare only the runtime copies here
# so discarded files never enter the final image's layers.
FROM ubuntu:${UBUNTU_VERSION} AS runtime-artifacts
ARG TARGETARCH

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        binutils \
        python3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=ton-binaries /opt/ton /opt/ton
COPY --from=http-api-builder /opt/ton-http-api-v2 /opt/ton-http-api-v2
COPY --from=ton-indexer-builder \
    /out/ton-indexer/bin/ton-index-postgres \
    /out/ton-indexer/bin/ton-index-postgres-migrate \
    /out/ton-indexer/bin/ton-smc-scanner \
    /opt/ton-indexer/bin/
COPY --from=ton-indexer-builder /out/ton-indexer/lib /opt/ton-indexer/lib
COPY --from=ton-index-api-builder /out/ton-indexer/bin/ton-index-go /opt/ton-indexer/bin/ton-index-go
COPY --from=ton-index-classifier-builder /out/ton-indexer/venv /opt/ton-indexer/venv
COPY --from=ton-index-classifier-builder /out/ton-indexer/classifier /opt/ton-indexer/classifier
COPY apps/localton/docker/prepare-runtime.sh /tmp/prepare-runtime.sh
RUN bash /tmp/prepare-runtime.sh "${TARGETARCH}"

# Development image for administrative hardforks: both the CLI and account
# indexer must be upgraded together. Native TON remains the pinned stock build.
FROM localton-rust-only AS localton-admin-dev
USER root
COPY --from=ton-indexer-builder /out/ton-indexer/bin/ /opt/ton-indexer/bin/
COPY --from=ton-indexer-builder /out/ton-indexer/lib/ /opt/ton-indexer/lib/
LABEL org.ton.localton.admin-hardforks="1"
USER localton

FROM ubuntu:${UBUNTU_VERSION} AS runtime
LABEL org.ton.localton.admin-hardforks="1"
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
        plzip \
        tini \
        zlib1g \
    && rm -rf /var/lib/apt/lists/* \
    && find /usr/lib/python3.12 -type d -name __pycache__ -prune -exec rm -rf {} + \
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

COPY --from=localton-builder /out/localton /usr/local/bin/localton
COPY --from=runtime-artifacts /opt /opt
COPY --from=ton-indexer-builder /out/ton-indexer/LICENSE /usr/share/doc/localton/TON-INDEXER-LICENSE
COPY --chmod=0755 apps/localton/docker/container-entrypoint.sh /usr/local/bin/localton-container-entrypoint
COPY apps/localton/NOTICE.md /usr/share/doc/localton/

ENV HOME=/var/lib/localton \
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
EXPOSE 18000/tcp 18001/tcp 18002/tcp 18003/tcp 18004/tcp 18007/tcp
STOPSIGNAL SIGTERM

HEALTHCHECK --interval=10s --timeout=3s --start-period=240s --retries=12 \
    CMD curl --fail --silent --show-error \
        http://127.0.0.1:18002/api/v2/getMasterchainInfo >/dev/null || exit 1

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/localton-container-entrypoint"]
CMD ["bootstrap", "--state-dir", "/var/lib/localton", "--ton-http-api"]
