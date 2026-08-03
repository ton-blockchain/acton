#!/bin/bash
set -Eeuo pipefail

component="${1:-}"
if [[ -n "${component}" ]]; then
    shift
fi

prepare_postgres() {
    : "${POSTGRES_HOST:=postgres}"
    : "${POSTGRES_PORT:=5432}"
    : "${POSTGRES_USER:=postgres}"
    : "${POSTGRES_DB:=ton_index}"

    local password=""
    if [[ -n "${POSTGRES_PASSWORD_FILE:-}" ]]; then
        if [[ ! -f "${POSTGRES_PASSWORD_FILE}" ]]; then
            echo "POSTGRES_PASSWORD_FILE does not exist: ${POSTGRES_PASSWORD_FILE}" >&2
            exit 1
        fi
        password="$(tr -d '\r\n' < "${POSTGRES_PASSWORD_FILE}")"
    elif [[ -n "${POSTGRES_PASSWORD:-}" ]]; then
        password="${POSTGRES_PASSWORD}"
    fi

    if [[ -n "${password}" ]]; then
        local pgpass
        pgpass="$(mktemp)"
        printf '*:*:*:*:%s\n' "${password}" > "${pgpass}"
        chmod 0600 "${pgpass}"
        export PGPASSFILE="${pgpass}"
    fi

    export TON_INDEXER_PG_URI="postgresql://${POSTGRES_USER}@${POSTGRES_HOST}:${POSTGRES_PORT}/${POSTGRES_DB}"
}

case "${component}" in
    v3-migrate)
        prepare_postgres
        exec /opt/ton-indexer/bin/ton-index-postgres-migrate \
            --pg "${TON_INDEXER_PG_URI}" \
            "$@"
        ;;
    v3-worker)
        prepare_postgres
        : "${TON_WORKER_DBROOT:=/var/lib/localton/genesis/db}"
        : "${TON_WORKER_WORKDIR:=/var/lib/ton-indexer/work}"
        : "${TON_WORKER_FROM:=1}"
        mkdir -p "${TON_WORKER_WORKDIR}"
        exec /opt/ton-indexer/bin/ton-index-postgres \
            --pg "${TON_INDEXER_PG_URI}" \
            --db "${TON_WORKER_DBROOT}" \
            --working-dir "${TON_WORKER_WORKDIR}" \
            --from "${TON_WORKER_FROM}" \
            "$@"
        ;;
    v3-api)
        prepare_postgres
        api_args=(
            -pg "${TON_INDEXER_PG_URI}"
            -bind "${TON_INDEXER_API_BIND:-:18003}"
            -threads "${TON_INDEXER_API_THREADS:-2}"
        )
        if [[ -n "${TON_INDEXER_TON_HTTP_API_ENDPOINT:-}" ]]; then
            api_args+=(-v2 "${TON_INDEXER_TON_HTTP_API_ENDPOINT}")
        fi
        if [[ -n "${TON_INDEXER_REDIS_DSN:-}" ]]; then
            api_args+=(-redis "${TON_INDEXER_REDIS_DSN}")
        fi
        if [[ "${TON_INDEXER_IS_TESTNET:-0}" =~ ^(1|true|yes|on)$ ]]; then
            api_args+=(-testnet)
        fi
        exec /opt/ton-indexer/bin/ton-index-go "${api_args[@]}" "$@"
        ;;
    v3-classifier)
        prepare_postgres
        export TON_INDEXER_PG_DSN="postgresql+asyncpg://${POSTGRES_USER}@${POSTGRES_HOST}:${POSTGRES_PORT}/${POSTGRES_DB}"
        export TON_INDEXER_REDIS_DSN="${TON_INDEXER_REDIS_DSN:-redis://redis:6379}"
        export TON_INDEXER_IS_TESTNET="${TON_INDEXER_IS_TESTNET:-0}"
        export TQDM_NCOLS=0
        export TQDM_POSITION=-1
        exec /opt/ton-indexer/venv/bin/python \
            /opt/ton-indexer/classifier/event_classifier.py \
            "$@"
        ;;
    "")
        exec /usr/local/bin/localton
        ;;
    *)
        exec /usr/local/bin/localton "${component}" "$@"
        ;;
esac
