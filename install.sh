#!/usr/bin/env bash
set -euo pipefail

# Colors
if [[ -t 1 ]]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[1;33m'
  BLUE='\033[0;34m'
  BOLD='\033[1m'
  NC='\033[0m' # No Color
else
  RED=''
  GREEN=''
  YELLOW=''
  BLUE=''
  BOLD=''
  NC=''
fi

function info() {
  echo -e "${BLUE}${BOLD}==>${NC} ${BOLD}$1${NC}"
}

function success() {
  echo -e "${GREEN}${BOLD}==>${NC} ${BOLD}$1${NC}"
}

function warn() {
  echo -e "${YELLOW}${BOLD}warning:${NC} $1"
}

function error() {
  echo -e "${RED}${BOLD}error:${NC} $1" >&2
}

function check_dependency() {
  if ! command -v "$1" >/dev/null 2>&1; then
    error "Required dependency '$1' is not installed."
    exit 1
  fi
}

# Check dependencies
check_dependency "curl"
check_dependency "python3"

REPO="i582/acton"
TAG="${1:-latest}"

API="https://api.github.com/repos/${REPO}"
AUTH=(-H "Accept: application/vnd.github+json")

if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  AUTH=(-H "Authorization: Bearer ${GITHUB_TOKEN}" "${AUTH[@]}")
fi

info "Fetching release information for ${TAG}..."
# Fetch release JSON
if [[ "$TAG" == "latest" ]]; then
  URL="${API}/releases/latest"
else
  # Add 'v' prefix if missing for version-like tags (common in GitHub releases)
  if [[ "$TAG" =~ ^[0-9] ]]; then
    CLEAN_TAG="v$TAG"
  else
    CLEAN_TAG="$TAG"
  fi
  URL="${API}/releases/tags/${CLEAN_TAG}"
fi

release_json="$(curl -fsSL "${AUTH[@]}" "$URL" 2>/dev/null)" || {
  echo -e "\n${RED}${BOLD}error:${NC} Could not find release ${BOLD}${TAG}${NC} on GitHub." >&2
  echo -e "Check if the tag exists here: ${BLUE}https://github.com/${REPO}/releases${NC}" >&2
  exit 1
}

# Get tag_name from release JSON
tag_name="$(
  python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name", ""))' <<<"$release_json"
)"

if [[ -z "$tag_name" ]]; then
  error "Failed to parse release information. The GitHub API response might be invalid."
  exit 1
fi

ASSET="acton-installer.sh"

# Find installer download URL by exact name
asset_url="$(
  python3 -c '
import json, sys
r = json.load(sys.stdin)
want = sys.argv[1]
print(next((a.get("browser_download_url", "") for a in r.get("assets", []) if a.get("name") == want), ""))
' "$ASSET" <<<"$release_json"
)"

if [[ -z "${asset_url:-}" ]]; then
  error "Asset not found in release ${tag_name}: ${ASSET}"
  exit 1
fi

tmp="$(mktemp -d)"

info "Downloading ${ASSET}..."
curl -fL --progress-bar \
  "${AUTH[@]}" \
  "${asset_url}" \
  -o "$tmp/$ASSET"

chmod +x "$tmp/$ASSET"

info "Running the release installer for ${tag_name}..."
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  export ACTON_GITHUB_TOKEN="${GITHUB_TOKEN}"
fi

sh "$tmp/$ASSET"
