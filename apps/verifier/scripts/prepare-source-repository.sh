#!/usr/bin/env bash
set -euo pipefail

readonly init_commit_message="Initialize verifier source repository"
readonly init_commit_date="2022-11-01T00:00:00+00:00"

log() {
    printf 'prepare-source-repository: %s\n' "$*"
}

usage() {
    echo "Usage: $0 [config-path]" >&2
}

if [[ $# -gt 1 ]]; then
    usage
    exit 2
fi

config_path="${1:-${VERIFIER_CONFIG:-config.toml}}"

if [[ ! -f "$config_path" ]]; then
    echo "verifier config does not exist: $config_path" >&2
    exit 1
fi

if ! command -v yq >/dev/null 2>&1; then
    echo "yq is required to read the verifier config" >&2
    exit 1
fi

repo_path="$(yq -p=toml -o=json -r '.source_repository.path // ""' "$config_path")"
if [[ -z "$repo_path" ]]; then
    echo "source_repository.path is missing in verifier config: $config_path" >&2
    exit 1
fi

storage_root="$(
    yq -p=toml -o=json -r '.source_repository.storage_root // "sources"' "$config_path"
)"
attributes_rule="$storage_root/** -text"

branch="$(yq -p=toml -o=json -r '.source_repository.branch // "master"' "$config_path")"
if ! git check-ref-format --branch "$branch" >/dev/null 2>&1; then
    echo "source_repository.branch is invalid: $branch" >&2
    exit 1
fi

author_name="$(
    yq -p=toml -o=json -r '.source_repository.author_name // "ton-verifier"' "$config_path"
)"
author_email="$(
    yq -p=toml -o=json -r \
        '.source_repository.author_email // "ton-verifier@example.invalid"' \
        "$config_path"
)"

if [[ ! -e "$repo_path" ]]; then
    mkdir -p "$repo_path"
    log "created directory: $repo_path"
fi

if [[ ! -d "$repo_path" ]]; then
    echo "repository path is not a directory: $repo_path" >&2
    exit 1
fi

repo_root="$(cd -P -- "$repo_path" && pwd)"
git_root=""
if detected_git_root="$(git -C "$repo_path" rev-parse --show-toplevel 2>/dev/null)"; then
    git_root="$(cd -P -- "$detected_git_root" && pwd)"
fi

if [[ "$git_root" != "$repo_root" ]]; then
    if [[ -n "$(find "$repo_path" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
        echo "repository path is not an empty directory: $repo_path" >&2
        exit 1
    fi

    git -C "$repo_path" init --quiet -b "$branch"
    log "initialized Git repository on branch $branch: $repo_path"
fi

if git -C "$repo_path" rev-parse --verify HEAD >/dev/null 2>&1; then
    echo "source repository already has commits: $repo_path" >&2
    exit 1
fi

if [[ -n "$(git -C "$repo_path" status --porcelain --untracked-files=all)" ]]; then
    echo "source repository has uncommitted files: $repo_path" >&2
    exit 1
fi

git -C "$repo_path" symbolic-ref HEAD "refs/heads/$branch"

printf '%s\n' "$attributes_rule" > "$repo_path/.gitattributes"
git -C "$repo_path" add -- .gitattributes

GIT_AUTHOR_DATE="$init_commit_date" \
GIT_COMMITTER_DATE="$init_commit_date" \
git \
    -C "$repo_path" \
    -c "user.name=$author_name" \
    -c "user.email=$author_email" \
    commit \
    --quiet \
    --no-gpg-sign \
    -m "$init_commit_message"

commit="$(git -C "$repo_path" rev-parse --short HEAD)"
log "created root commit $commit on branch $branch: $init_commit_message"
