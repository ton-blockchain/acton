# syntax=docker/dockerfile:1

FROM oven/bun:1.3.14-slim AS frontend-builder

WORKDIR /workspace

COPY package.json bun.lock ./
COPY packages ./packages
RUN bun install --frozen-lockfile --filter @acton/verifier-ui \
    && bun --filter @acton/verifier-ui build

FROM rust:1.96.1-slim-bookworm AS rust-builder

WORKDIR /build/apps/verifier

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates git libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY apps/verifier/Cargo.toml apps/verifier/Cargo.lock apps/verifier/rust-toolchain.toml apps/verifier/build.rs ./
COPY apps/verifier/src ./src
COPY --from=frontend-builder /workspace/packages/verifier-ui/dist /build/packages/verifier-ui/dist

RUN cargo build --release --locked

FROM node:22-bookworm-slim AS compiler-worker

WORKDIR /compiler-worker

COPY apps/verifier/compiler-worker/package.json apps/verifier/compiler-worker/package-lock.json apps/verifier/compiler-worker/.npmrc ./
COPY apps/verifier/compiler-worker/scripts ./scripts
RUN npm ci --omit=dev
RUN npm run apply-patches

COPY apps/verifier/compiler-worker/compile.mjs ./compile.mjs
COPY apps/verifier/compiler-worker/languages ./languages
COPY apps/verifier/compiler-worker/fixtures ./fixtures

FROM node:22-bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client tini \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=rust-builder /build/apps/verifier/target/release/verifier /usr/local/bin/verifier
COPY --chown=node:node --from=compiler-worker /compiler-worker /app/compiler-worker
COPY apps/verifier/docker/entrypoint.sh /usr/local/bin/verifier-entrypoint

RUN chmod +x /usr/local/bin/verifier-entrypoint \
    && mkdir -p /etc/verifier /var/lib/verifier/source-repo /var/lib/verifier/registry-index \
    && chown -R node:node /etc/verifier /var/lib/verifier

USER node

ENV VERIFIER_CONFIG=/etc/verifier/config.toml \
    VERIFIER_BIND_ADDR=0.0.0.0:3000 \
    VERIFIER_LOG_LEVEL=info \
    VERIFIER_NETWORK=mainnet \
    SOURCE_REPOSITORY_PATH=/var/lib/verifier/source-repo \
    SOURCE_REPOSITORY_REMOTE=origin \
    SOURCE_REPOSITORY_STORAGE_ROOT=sources \
    SOURCE_REPOSITORY_BRANCH=main \
    SOURCE_REPOSITORY_AUTHOR_NAME=ton-verifier \
    SOURCE_REPOSITORY_AUTHOR_EMAIL=ton-verifier@example.invalid \
    VERIFIER_REGISTRY_INDEX_PATH=/var/lib/verifier/registry-index/registry-index.sqlite3 \
    VERIFIER_COMPILER_NODE_BIN=/usr/local/bin/node \
    VERIFIER_COMPILER_WORKER_PATH=/app/compiler-worker/compile.mjs \
    VERIFIER_COMPILER_TIMEOUT_MS=5000

EXPOSE 3000

ENTRYPOINT ["tini", "--", "verifier-entrypoint"]
CMD ["verifier"]
