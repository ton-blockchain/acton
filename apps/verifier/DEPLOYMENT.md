# Server Deployment Guide

This document describes how to deploy the verifier backend as a Docker service on a server.

The Docker image contains the verifier backend, Node.js, the compiler worker,
Git, and OpenSSH. It does not run a TON node. The verifier always needs a
TonCenter v3 endpoint for TON testnet payment verification.

## Architecture

At runtime the service needs:

- Verifier HTTP backend exposed on port `3000`.
- TonCenter-compatible API endpoint for address-to-code-hash resolution.
- Testnet wallet address that receives verification payments.
- SQLite payment ledger that prevents transaction replay.
- Git source repository for verified source bundles.
- SQLite registry index for fast reads, rebuilt from Git when stale or missing.
- Git credentials that allow pushing to the source repository.

The verifier stores verified source bundles in Git under the configured
`source_repository.storage_root`, which defaults to `sources`:

```text
sources/{code_hash}/
```

Each bundle contains:

- `manifest.json`
- `files/...`

## Prerequisites

Install on the server:

- Docker Engine
- Docker Compose plugin
- Git, only if you plan to build the image from the repository on the server

Recommended server setup:

- Linux VM with at least 2 CPU cores and 2 GB RAM
- Persistent Docker volume for `/var/lib/verifier/source-repo`
- Persistent Docker volume for `/var/lib/verifier/registry-index`
- Persistent Docker volume for `/var/lib/verifier/payment-ledger`
- Firewall allowing inbound traffic only to the reverse proxy or to port `3000` if exposing directly
- Secrets managed through environment files, Docker secrets, or your orchestrator

## CI-Published Image

GitHub Actions builds the same Dockerfile used locally:

- Pull requests build `ton-verifier:ci` and smoke-test `/healthz`.
- Pushes to `master` or `main` publish a multi-arch image to GHCR.
- Tags matching `v*.*.*` publish release tags.

For this repository the published image is:

```bash
ghcr.io/i582/verifier
```

Common tags:

- `latest` for the default branch
- `master` or `main` for branch builds
- `sha-<short-sha>` for immutable commit builds
- `v1.2.3`, `1.2.3`, and `1.2` for release tags

Pull the CI-built image on the server:

```bash
docker pull ghcr.io/i582/verifier:latest
```

If the package is private, authenticate first with a token that has `read:packages`:

```bash
echo "$GHCR_TOKEN" | docker login ghcr.io -u <github-user> --password-stdin
```

## Build The Image Locally

From the monorepo root:

```bash
docker build -f apps/verifier/Dockerfile -t ton-verifier:local .
```

Manual registry publishing is still possible:

```bash
docker tag ton-verifier:local registry.example.com/ton-verifier:0.1.0
docker push registry.example.com/ton-verifier:0.1.0
```

If you use a manually published image instead of the GHCR image, pull it on the server:

```bash
docker pull registry.example.com/ton-verifier:0.1.0
```

## Configure The Service

Create an environment file on the server:

```bash
sudo mkdir -p /opt/ton-verifier
sudo install -m 600 /dev/null /opt/ton-verifier/verifier.env
```

Example `/opt/ton-verifier/verifier.env`:

```bash
VERIFIER_NETWORK=testnet
VERIFIER_LOG_LEVEL=info
VERIFIER_API_KEY=
VERIFIER_TONCENTER_BASE_URL=https://testnet.toncenter.com
VERIFIER_TONCENTER_API_KEY=
VERIFIER_PAYMENT_ADDRESS="0:<64-hex-character-testnet-wallet-address>"
VERIFIER_PAYMENT_MIN_AMOUNT_NANO=500000000
VERIFIER_PAYMENT_LEDGER_PATH=/var/lib/verifier/payment-ledger/payment-ledger.sqlite3

SOURCE_REPOSITORY_URL=git@github.com:i582/test-verify-repo.git
SOURCE_REPOSITORY_STORAGE_ROOT=sources
SOURCE_REPOSITORY_BRANCH=main
SOURCE_REPOSITORY_AUTHOR_NAME=ton-verifier
SOURCE_REPOSITORY_AUTHOR_EMAIL=ton-verifier@example.invalid
SOURCE_REPOSITORY_SSH_KEY_FILE=/run/secrets/source_repo_key
SOURCE_REPOSITORY_SSH_STRICT_HOST_KEY_CHECKING=accept-new

VERIFIER_REGISTRY_INDEX_PATH=/var/lib/verifier/registry-index/registry-index.sqlite3
```

`VERIFIER_API_KEY` protects the optional `verified_at` field on
`POST /api/v1/verify`; clients pass it in the `X-Verifier-Key` header.

The payment verifier supports only TON testnet. `VERIFIER_PAYMENT_ADDRESS` must
use the raw basechain form `0:<64 hex characters>`. The minimum amount is in
nanoGRAM and must be more than zero. This example sets the amount to
`0.5 GRAM`.

## Configure GitHub Source Storage

Use a dedicated repository for source storage. The verifier only needs push access to that repository.

Prepare the repository from the monorepo root before the first deployment:

```bash
git clone 'git@github.com:i582/test-verify-repo.git' source-repo
apps/verifier/scripts/prepare-source-repository.sh config.toml
git -C source-repo push origin HEAD:main
```

Set `source_repository.path` in `config.toml` to the cloned `source-repo`
directory before you run the preparation script. The script creates the
required root commit and `.gitattributes` rule.

Recommended authentication is an SSH deploy key with write access:

```bash
sudo install -m 700 -d /opt/ton-verifier/secrets
sudo install -m 600 source_repo_key /opt/ton-verifier/secrets/source_repo_key
sudo chown 1000:1000 /opt/ton-verifier/secrets/source_repo_key
```

The container runs as user ID `1000`. Make sure that this user can read the
mounted key. Use the equivalent ownership rule for your secret manager.

The key must match:

```bash
SOURCE_REPOSITORY_URL=git@github.com:i582/test-verify-repo.git
SOURCE_REPOSITORY_SSH_KEY_FILE=/run/secrets/source_repo_key
```

HTTPS remotes can also work, but then credentials must be provided through Docker secret mounts, a credential helper, or a tokenized remote URL. Avoid committing tokens to files or image layers.

## Docker Compose Deployment

Create `/opt/ton-verifier/docker-compose.yml`:

```yaml
services:
  verifier:
    image: ghcr.io/i582/verifier:latest
    restart: unless-stopped
    ports:
      - "3000:3000"
    env_file:
      - /opt/ton-verifier/verifier.env
    volumes:
      - source-repo:/var/lib/verifier/source-repo
      - registry-index:/var/lib/verifier/registry-index
      - payment-ledger:/var/lib/verifier/payment-ledger
      - /opt/ton-verifier/secrets/source_repo_key:/run/secrets/source_repo_key:ro
    extra_hosts:
      - "host.docker.internal:host-gateway"
    healthcheck:
      test:
        [
          "CMD",
          "node",
          "-e",
          "fetch('http://127.0.0.1:3000/healthz').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))",
        ]
      interval: 10s
      timeout: 3s
      retries: 6
      start_period: 10s

volumes:
  source-repo:
  registry-index:
  payment-ledger:
```

Start the service:

```bash
cd /opt/ton-verifier
docker compose pull
docker compose up -d
```

Check status:

```bash
docker compose ps
docker compose logs -f verifier
curl -sS http://127.0.0.1:3000/healthz
```

Expected health response:

```json
{ "ok": true }
```

During startup recovery, `/healthz` returns `503` with this response:

```json
{ "ok": false, "payment_recovery": "rebuilding" }
```

The server scans the complete payment-wallet history before it becomes ready.
It marks every funded protocol payment as consumed. It also preserves all
known replay records. A failed scan retries with an exponential delay.

One payment permits at most three verification claims. The limit includes a
claim that resumes after an expired processing lease. Later claims fail as
used without another TonCenter request.

## Systemd Wrapper

Use systemd to keep Docker Compose running after reboots.

Create `/etc/systemd/system/ton-verifier.service`:

```ini
[Unit]
Description=TON Verifier Backend
Requires=docker.service
After=docker.service network-online.target
Wants=network-online.target

[Service]
Type=oneshot
WorkingDirectory=/opt/ton-verifier
RemainAfterExit=yes
ExecStart=/usr/bin/docker compose up -d
ExecStop=/usr/bin/docker compose down
TimeoutStartSec=0

[Install]
WantedBy=multi-user.target
```

Enable it:

```bash
sudo systemctl daemon-reload
sudo systemctl enable ton-verifier
sudo systemctl start ton-verifier
sudo systemctl status ton-verifier
```

## Reverse Proxy

If exposing publicly, put the service behind Nginx, Caddy, Traefik, or another reverse proxy with TLS.

Minimal Nginx location:

```nginx
location / {
    proxy_pass http://127.0.0.1:3000;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Keep direct port `3000` closed to the public internet if the reverse proxy is the public entrypoint.

Configure rate limits in the reverse proxy. A useful initial policy permits 10
ticket requests and two verification uploads per source IP each minute. Return
`429` when a client exceeds the limit, and tune the values from production
traffic. Keep `/healthz` outside this limit.

## Smoke Test

Check service health:

```bash
curl -sS http://127.0.0.1:3000/healthz
```

Check verification status for a known code hash:

```bash
curl -sS 'http://127.0.0.1:3000/api/v1/verification/status?code_hash=<code_hash>'
```

Fetch the OpenAPI schema:

```bash
curl -sS 'http://127.0.0.1:3000/api/v1/openapi.json'
```

Request a payment ticket:

```bash
curl -sS -X POST http://127.0.0.1:3000/api/v1/take-ticket \
  -H 'Content-Type: application/json' \
  -d '{"code_hash":"<code_hash>"}'
```

If the response status is `payment_required`, send the returned amount and
comment to the returned testnet address. Wait for the finalized recipient
transaction hash.

Submit a verification request with that transaction hash:

```bash
curl -sS -X POST http://127.0.0.1:3000/api/v1/verify \
  -F 'code_hash=<code_hash>' \
  -F 'tx_hash=<recipient-transaction-hash>' \
  -F language=tolk \
  -F 'compile_params={"compiler_version":"1.4.2"}' \
  -F 'sources=[{"path":"main.tolk","is_entrypoint":true},{"path":"imports/math.tolk","is_entrypoint":false}]' \
  -F 'files=@./main.tolk;filename=main.tolk' \
  -F 'files=@./imports/math.tolk;filename=imports/math.tolk'
```

The successful response contains:

```json
{
  "verification_result": "match",
  "source_bundle_hash": "...",
  "storage_revision": "..."
}
```

If the code hash was verified earlier, the verifier skips compilation and
returns `verification_result: "already_verified"` with the original
`source_bundle_hash` and `storage_revision`.

Fetch verified source:

```bash
curl -sS 'http://127.0.0.1:3000/api/v1/verification/source?code_hash=<code_hash>'
```

Fetch recent verified contracts:

```bash
curl -sS 'http://127.0.0.1:3000/api/v1/last_verified?limit=50&offset=0'
```

Fetch indexed Tolk ABI records:

```bash
curl -sS 'http://127.0.0.1:3000/api/v1/abi?code_hash=<code_hash>'
```

This does not build Rust, Node.js packages, or compilers on the server. The server only pulls the CI-built image and starts it.

## Updating

Pull the new image:

```bash
docker pull ghcr.io/i582/verifier:latest
```

Update the image tag in `/opt/ton-verifier/docker-compose.yml`, then restart:

```bash
cd /opt/ton-verifier
docker compose up -d
docker compose logs -f verifier
```

Run smoke checks after every update.

## Backup

Back up:

- GitHub source repository
- `/opt/ton-verifier/verifier.env`
- SSH deploy key or Git credentials
- Docker Compose file

The SQLite registry index volume is useful for fast restarts, but it is not the
source of truth. If the index volume is lost, the service rebuilds it from the
Git source repository.

The payment ledger is also derived state. If this volume is lost, the service
rebuilds it from TON testnet history and marks all funded protocol payments as
used. Keeping or backing up this volume does not skip the full startup history
scan.

The Docker `source-repo` volume is a local clone. The remote Git repository is
the authoritative source storage after every successful push.

## Troubleshooting

### `source_repository.path` missing

The service has no Git storage path configured. Set:

```bash
SOURCE_REPOSITORY_PATH=/var/lib/verifier/source-repo
SOURCE_REPOSITORY_STORAGE_ROOT=sources
```

or mount a TOML config with `[source_repository].path` and
`[source_repository].storage_root`.

### Git push fails

Check:

```bash
docker compose exec verifier git -C /var/lib/verifier/source-repo remote -v
docker compose exec verifier git -C /var/lib/verifier/source-repo status
```

For SSH remotes, verify the key mount:

```bash
docker compose exec verifier test -r /run/secrets/source_repo_key
```

Common causes:

- Deploy key does not have write access.
- `SOURCE_REPOSITORY_URL` points to HTTPS while only SSH credentials are mounted.
- Host key verification blocks the first connection.
- The branch configured in `SOURCE_REPOSITORY_BRANCH` is protected.

### TonCenter lookup fails

Check:

- `VERIFIER_NETWORK` is `testnet`.
- `VERIFIER_TONCENTER_BASE_URL` is reachable from inside the container.
- `VERIFIER_TONCENTER_API_KEY` is set if your endpoint requires it.

Connectivity check:

```bash
docker compose exec verifier node -e "const u=new URL('/api/v3/transactions',process.env.VERIFIER_TONCENTER_BASE_URL);u.searchParams.set('account',process.env.VERIFIER_PAYMENT_ADDRESS);u.searchParams.set('limit','1');fetch(u,{headers:{'X-API-Key':process.env.VERIFIER_TONCENTER_API_KEY||''}}).then(r=>{if(!r.ok)throw Error(r.status);return r.json()}).then(()=>console.log('ok')).catch(e=>{console.error(e);process.exit(1)})"
```

### Compiler fails

Check that the worker exists:

```bash
docker compose exec verifier test -f /app/compiler-worker/compile.mjs
```

Check Node:

```bash
docker compose exec verifier node --version
```

Supported Tolk compiler versions are the statically bundled `@ton/tolk-js` versions from the
compiler worker package.

### Container starts but `/healthz` is not reachable

Check:

```bash
docker compose logs verifier
docker compose ps
docker compose exec verifier cat /etc/verifier/config.toml
```

Make sure the generated config has:

```toml
[server]
bind_addr = "0.0.0.0:3000"
```

Binding to `127.0.0.1:3000` inside the container will not expose the service correctly through Docker port mapping.

## Operational Notes

- Run one write-capable verifier instance for each payment wallet. SQLite does
  not coordinate payment claims or startup recovery across replicas.
- Keep only one verifier instance writing to the same Git checkout. The current Git storage lock is process-local.
- Treat the configured TonCenter provider as trusted for payment data and
  finality. The provider sees the payment address, wallet-history reads, and
  transaction hashes.
- Before changing the payment address or increasing the minimum amount, stop
  ticket issuance at the reverse proxy. Keep `/verify` available for an
  announced grace interval. Then deploy the new configuration. Quotes have no
  server-side expiry, so payments submitted after this interval are discarded.
- Recovery scans only the configured payment address. Keep the address stable
  unless the deployment intentionally invalidates old outstanding payments.
- Use a dedicated Git repository for source storage.
- Do not bake deploy keys into the image.
- Prefer SSH deploy keys scoped to one repository.
- Use a reverse proxy for TLS and request size limits.
- Add rate limits for `/api/v1/take-ticket` and `/api/v1/verify`. Testnet GRAM
  alone does not stop a funded attacker.
- Do not submit secrets in source files. Git and the source API publish every
  accepted source bundle.
- Monitor logs for failed Git pushes, compiler errors, and TonCenter API errors.
- Monitor `/healthz` for payment-history recovery failures.
