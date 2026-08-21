# syntax=docker/dockerfile:1

FROM rust:1.97.1-slim-bookworm AS rust-builder

WORKDIR /build/apps/actonscan-backend

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates git libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml build.rs /build/
COPY src /build/src
COPY crates /build/crates
COPY xtask /build/xtask
COPY apps/actonscan-backend/Cargo.toml apps/actonscan-backend/Cargo.lock apps/actonscan-backend/rust-toolchain.toml ./
COPY apps/actonscan-backend/src ./src

RUN cargo build --release --locked --bin actonscan-backend

FROM debian:bookworm-slim AS runtime
ARG BUILD_VERSION=dev
ARG SOURCE_URL=https://github.com/ton-blockchain/acton
ARG VCS_REF=unknown

LABEL org.opencontainers.image.title="actonscan-backend" \
      org.opencontainers.image.description="Actonscan network statistics backend" \
      org.opencontainers.image.source="${SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0" \
      org.opencontainers.image.version="${BUILD_VERSION}" \
      org.opencontainers.image.revision="${VCS_REF}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --home-dir /var/lib/actonscan actonscan

WORKDIR /var/lib/actonscan

COPY --from=rust-builder /build/target/release/actonscan-backend /usr/local/bin/actonscan-backend
COPY crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json /etc/actonscan/mainnet-global.config.json
COPY apps/actonscan-backend/docker/config.toml /etc/actonscan/config.toml

USER actonscan

ENV ACTONSCAN_CONFIG=/etc/actonscan/config.toml

VOLUME ["/var/lib/actonscan"]

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget --quiet --output-document=- http://127.0.0.1:3000/healthz || exit 1

ENTRYPOINT ["tini", "--"]
CMD ["actonscan-backend"]
