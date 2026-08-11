# Docker Deployment

Production deployments should pull the CI-built image:

```bash
docker pull ghcr.io/i582/verifier:latest
```

Local development can build the image:

```bash
docker build -f apps/verifier/Dockerfile -t ton-verifier:local .
```

Initialize an empty source repository before deploying the verifier:

```bash
git clone <source-repository-url> source-repo
# Set source_repository.path = "source-repo" in config.toml.
apps/verifier/scripts/prepare-source-repository.sh config.toml
git -C source-repo push origin HEAD:main
```

The preparation script creates the required root commit with the source-storage
Git attributes. The verifier refuses to start when this commit is missing or
the current `.gitattributes` no longer contains
`<source_repository.storage_root>/** -text`.

Or use the local build override:

```bash
docker compose \
  -f apps/verifier/docker-compose.yml \
  -f apps/verifier/docker-compose.local.yml \
  up -d --build
```

Run with generated config:

```bash
docker run --rm -p 3000:3000 \
  -e VERIFIER_NETWORK=testnet \
  -e VERIFIER_TONCENTER_BASE_URL=https://testnet.toncenter.com \
  -e 'VERIFIER_PAYMENT_ADDRESS=0:<64-hex-character-testnet-wallet-address>' \
  -e VERIFIER_PAYMENT_MIN_AMOUNT_NANO=500000000 \
  -e SOURCE_REPOSITORY_URL=https://github.com/i582/test-verify-repo \
  -e SOURCE_REPOSITORY_STORAGE_ROOT=sources \
  -e SOURCE_REPOSITORY_BRANCH=main \
  -v verifier-source-repo:/var/lib/verifier/source-repo \
  -v verifier-registry-index:/var/lib/verifier/registry-index \
  -v verifier-payment-ledger:/var/lib/verifier/payment-ledger \
  ghcr.io/i582/verifier:latest
```

The verifier accepts payments only on TON testnet. The payment address must
use raw basechain form. At startup, the service rebuilds the payment ledger
from wallet history and reports `503` until the scan is complete. This example
sets the minimum payment to `0.5 GRAM`.

Or mount a full TOML config:

```bash
docker run --rm -p 3000:3000 \
  -e VERIFIER_CONFIG=/etc/verifier/config.toml \
  -v ./config.toml:/etc/verifier/config.toml:ro \
  -v verifier-source-repo:/var/lib/verifier/source-repo \
  -v verifier-registry-index:/var/lib/verifier/registry-index \
  -v verifier-payment-ledger:/var/lib/verifier/payment-ledger \
  ghcr.io/i582/verifier:latest
```

For SSH Git remotes, mount a deploy key and pass:

```bash
-e SOURCE_REPOSITORY_URL=git@github.com:i582/test-verify-repo.git
-e SOURCE_REPOSITORY_SSH_KEY_FILE=/run/secrets/source_repo_key
-v ./source_repo_key:/run/secrets/source_repo_key:ro
```

The image contains:

- `verifier` Rust backend
- Node.js runtime
- `compiler-worker/compile.mjs`
- Static NPM compiler packages for supported FunC, Tact, and Tolk versions
- Git and OpenSSH client for source storage commit/push
