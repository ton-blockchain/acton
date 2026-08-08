#!/bin/sh
set -eu

config_path="${VERIFIER_CONFIG:-/etc/verifier/config.toml}"

toml_escape() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

write_optional_string() {
    key="$1"
    value="$2"
    if [ -n "$value" ]; then
        printf '%s = "%s"\n' "$key" "$(toml_escape "$value")"
    fi
}

write_optional_int() {
    key="$1"
    value="$2"
    if [ -n "$value" ]; then
        printf '%s = %s\n' "$key" "$value"
    fi
}

configure_git_ssh() {
    if [ -n "${SOURCE_REPOSITORY_SSH_KEY_FILE:-}" ]; then
        strict_host_key_checking="${SOURCE_REPOSITORY_SSH_STRICT_HOST_KEY_CHECKING:-accept-new}"
        export GIT_SSH_COMMAND="ssh -i ${SOURCE_REPOSITORY_SSH_KEY_FILE} -o IdentitiesOnly=yes -o StrictHostKeyChecking=${strict_host_key_checking}"
    fi
}

ensure_source_repository() {
    repo_path="${SOURCE_REPOSITORY_PATH:-}"
    repo_url="${SOURCE_REPOSITORY_URL:-}"
    branch="${SOURCE_REPOSITORY_BRANCH:-}"
    remote="${SOURCE_REPOSITORY_REMOTE:-origin}"

    if [ -z "$repo_path" ] || [ -z "$repo_url" ]; then
        return 0
    fi

    if [ -d "$repo_path/.git" ]; then
        if git -C "$repo_path" remote get-url "$remote" >/dev/null 2>&1; then
            git -C "$repo_path" remote set-url "$remote" "$repo_url"
        else
            git -C "$repo_path" remote add "$remote" "$repo_url"
        fi
        return 0
    fi

    if [ -e "$repo_path" ] && [ "$(find "$repo_path" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' ')" != "0" ]; then
        echo "source repository path exists and is not an empty git repository: $repo_path" >&2
        exit 1
    fi

    mkdir -p "$(dirname "$repo_path")"
    git clone "$repo_url" "$repo_path"
    if [ -n "$branch" ]; then
        git -C "$repo_path" switch "$branch" 2>/dev/null || git -C "$repo_path" switch -c "$branch"
    fi
}

write_generated_config() {
    mkdir -p "$(dirname "$config_path")"

    {
        printf '[server]\n'
        printf 'bind_addr = "%s"\n' "$(toml_escape "${VERIFIER_BIND_ADDR:-0.0.0.0:3000}")"
        write_optional_string api_key "${VERIFIER_API_KEY:-}"
        printf '\n'

        printf '[logging]\n'
        printf 'level = "%s"\n\n' "$(toml_escape "${VERIFIER_LOG_LEVEL:-info}")"

        printf '[network]\n'
        printf 'name = "%s"\n\n' "$(toml_escape "${VERIFIER_NETWORK:-mainnet}")"

        printf '[toncenter]\n'
        write_optional_string base_url "${VERIFIER_TONCENTER_BASE_URL:-}"
        write_optional_string api_key "${VERIFIER_TONCENTER_API_KEY:-}"
        printf '\n'

        printf '[source_repository]\n'
        write_optional_string path "${SOURCE_REPOSITORY_PATH:-/var/lib/verifier/source-repo}"
        write_optional_string remote "${SOURCE_REPOSITORY_REMOTE:-origin}"
        write_optional_string storage_root "${SOURCE_REPOSITORY_STORAGE_ROOT:-sources}"
        write_optional_string branch "${SOURCE_REPOSITORY_BRANCH:-main}"
        write_optional_string author_name "${SOURCE_REPOSITORY_AUTHOR_NAME:-ton-verifier}"
        write_optional_string author_email "${SOURCE_REPOSITORY_AUTHOR_EMAIL:-ton-verifier@example.invalid}"
        printf '\n'

        printf '[registry_index]\n'
        write_optional_string path "${VERIFIER_REGISTRY_INDEX_PATH:-/var/lib/verifier/registry-index/registry-index.sqlite3}"
        printf '\n'

        printf '[compiler]\n'
        write_optional_string node_bin "${VERIFIER_COMPILER_NODE_BIN:-node}"
        write_optional_string worker_path "${VERIFIER_COMPILER_WORKER_PATH:-/app/compiler-worker/compile.mjs}"
        write_optional_int timeout_ms "${VERIFIER_COMPILER_TIMEOUT_MS:-5000}"
    } > "$config_path"
}

configure_git_ssh
ensure_source_repository

if [ ! -f "$config_path" ] || [ "${VERIFIER_FORCE_GENERATE_CONFIG:-0}" = "1" ]; then
    write_generated_config
fi

exec "$@"
