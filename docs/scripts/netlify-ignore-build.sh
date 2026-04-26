#!/usr/bin/env bash
set -euo pipefail

if [[ "${CONTEXT:-}" != "deploy-preview" ]]; then
  echo "Skipping Netlify build for context: ${CONTEXT:-unknown}"
  exit 0
fi

if [[ -z "${CACHED_COMMIT_REF:-}" || "${CACHED_COMMIT_REF}" == "${COMMIT_REF:-}" ]]; then
  echo "No cached commit available; building deploy preview."
  exit 1
fi

if git diff --quiet "${CACHED_COMMIT_REF}" "${COMMIT_REF}" -- . ../netlify.toml; then
  echo "No docs changes detected; skipping deploy preview."
  exit 0
fi

echo "Docs changes detected; building deploy preview."
exit 1
