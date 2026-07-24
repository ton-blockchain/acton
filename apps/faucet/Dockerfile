# syntax=docker/dockerfile:1

FROM rust:1.94.1-slim-bookworm AS rust-builder

WORKDIR /build/apps/faucet

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates git libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY apps/faucet/Cargo.toml apps/faucet/Cargo.lock apps/faucet/rust-toolchain.toml apps/faucet/build.rs ./
COPY apps/faucet/crates ./crates
COPY apps/faucet/src ./src

RUN cargo build --release --locked --bin faucet-backend

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 libsqlite3-0 tini wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home --home-dir /var/lib/faucet faucet

WORKDIR /var/lib/faucet

COPY --from=rust-builder /build/apps/faucet/target/release/faucet-backend /usr/local/bin/faucet-backend

USER faucet

ENV DATABASE_URL=sqlite:./db.sqlite \
    HOST=0.0.0.0 \
    PORT=3000 \
    LOG_LEVEL=info \
    LOG_FORMAT=json

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --quiet --output-document=- http://127.0.0.1:3000/health || exit 1

ENTRYPOINT ["tini", "--"]
CMD ["faucet-backend"]
