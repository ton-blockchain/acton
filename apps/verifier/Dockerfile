# syntax=docker/dockerfile:1

# TODO: Remove yq from the runtime image once repository initialization no longer parses TOML in shell.
FROM mikefarah/yq:4.53.6 AS yq

FROM oven/bun:1.4.0-slim AS frontend-builder

WORKDIR /workspace

COPY package.json bun.lock ./
COPY packages ./packages
RUN bun install --frozen-lockfile --filter @acton/verifier-ui \
    && bun --filter @acton/verifier-ui build

FROM rust:1.97.1-slim-bookworm AS rust-builder

WORKDIR /build/apps/verifier
ARG VCS_REF=unknown

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates git libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY apps/verifier/Cargo.toml apps/verifier/Cargo.lock apps/verifier/rust-toolchain.toml apps/verifier/build.rs ./
COPY apps/verifier/assets ./assets
COPY apps/verifier/src ./src
COPY apps/verifier/github-app-credential ./github-app-credential
COPY libs/rston /build/libs/rston
COPY --from=frontend-builder /workspace/packages/verifier-ui/dist /build/packages/verifier-ui/dist

RUN VERIFIER_GIT_HASH="${VCS_REF}" cargo build --release --locked --bins

FROM node:22-bookworm-slim AS compiler-worker

WORKDIR /compiler-worker

COPY apps/verifier/compiler-worker/package.json apps/verifier/compiler-worker/package-lock.json apps/verifier/compiler-worker/.npmrc ./
COPY apps/verifier/compiler-worker/scripts ./scripts
RUN npm ci --omit=dev \
    && npm run apply-patches

COPY apps/verifier/compiler-worker/compile.mjs ./compile.mjs
COPY apps/verifier/compiler-worker/languages ./languages
COPY apps/verifier/compiler-worker/fixtures ./fixtures

FROM node:22-bookworm-slim AS runtime
ARG BUILD_VERSION=dev
ARG SOURCE_URL=https://github.com/ton-blockchain/acton
ARG VCS_REF=unknown

LABEL org.opencontainers.image.title="verifier" \
      org.opencontainers.image.description="TON smart contract source verification service" \
      org.opencontainers.image.source="${SOURCE_URL}" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0" \
      org.opencontainers.image.version="${BUILD_VERSION}" \
      org.opencontainers.image.revision="${VCS_REF}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client tini \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 10002 verifier \
    && useradd --uid 10002 --gid verifier --create-home --home-dir /var/lib/verifier \
        --shell /usr/sbin/nologin --no-log-init verifier

WORKDIR /app

COPY --from=rust-builder /build/apps/verifier/target/release/verifier /usr/local/bin/verifier
COPY --from=rust-builder /build/apps/verifier/target/release/github-app-credential /usr/local/bin/github-app-credential
COPY --from=yq /usr/bin/yq /usr/local/bin/yq
COPY --chown=verifier:verifier --from=compiler-worker /compiler-worker /app/compiler-worker
COPY apps/verifier/docker/entrypoint.sh /usr/local/bin/verifier-entrypoint
COPY apps/verifier/scripts/prepare-source-repository.sh /usr/local/bin/verifier-prepare-source-repository

RUN node /app/compiler-worker/scripts/check-runtime-symlinks.mjs /app/compiler-worker \
    && chmod +x /usr/local/bin/verifier-entrypoint /usr/local/bin/verifier-prepare-source-repository \
    && mkdir -p /etc/verifier /var/lib/verifier/source-repo /var/lib/verifier/registry-index /var/lib/verifier/payment-ledger \
    && chmod 0700 /var/lib/verifier/source-repo /var/lib/verifier/registry-index /var/lib/verifier/payment-ledger \
    && chown -R verifier:verifier /etc/verifier /var/lib/verifier

USER verifier

ENV VERIFIER_CONFIG=/etc/verifier/config.toml \
    VERIFIER_BIND_ADDR=0.0.0.0:3000 \
    VERIFIER_LOG_LEVEL=info \
    VERIFIER_PAYMENT_PRIMARY_NETWORK=testnet \
    SOURCE_REPOSITORY_PATH=/var/lib/verifier/source-repo \
    SOURCE_REPOSITORY_AUTH_MODE=none \
    SOURCE_REPOSITORY_REMOTE=origin \
    SOURCE_REPOSITORY_STORAGE_ROOT=sources \
    SOURCE_REPOSITORY_BRANCH=main \
    SOURCE_REPOSITORY_COMMIT_ENABLED=true \
    SOURCE_REPOSITORY_PUSH_ENABLED=true \
    SOURCE_REPOSITORY_AUTHOR_NAME=ton-verifier \
    SOURCE_REPOSITORY_AUTHOR_EMAIL=ton-verifier@example.invalid \
    VERIFIER_REGISTRY_INDEX_PATH=/var/lib/verifier/registry-index/registry-index.sqlite3 \
    VERIFIER_COMPILER_NODE_BIN=/usr/local/bin/node \
    VERIFIER_COMPILER_WORKER_PATH=/app/compiler-worker/compile.mjs \
    VERIFIER_COMPILER_TIMEOUT_MS=10000

EXPOSE 3000

ENTRYPOINT ["tini", "--", "verifier-entrypoint"]
CMD ["verifier"]
