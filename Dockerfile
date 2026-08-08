FROM ubuntu:noble

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        git \
        libgcc-s1 \
        libssl3 \
        libstdc++6 \
    && update-ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY dist/docker/acton /usr/local/bin/acton

RUN chmod +x /usr/local/bin/acton

ENTRYPOINT ["/usr/local/bin/acton"]

LABEL org.opencontainers.image.title="Acton" \
      org.opencontainers.image.description="TON smart contract development toolkit" \
      org.opencontainers.image.url="https://ton-blockchain.github.io/acton/" \
      org.opencontainers.image.source="https://github.com/ton-blockchain/acton" \
      org.opencontainers.image.documentation="https://ton-blockchain.github.io/acton/docs/welcome" \
      org.opencontainers.image.vendor="TON Core" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0"
