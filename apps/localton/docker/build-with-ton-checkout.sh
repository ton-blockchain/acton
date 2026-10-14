#!/bin/bash
set -Eeuo pipefail

# Run `docker build` with a TON git checkout as the `ton-src` build context.
#
# BuildKit transfers a named context whole: COPY --exclude filters only what is
# copied. Pass a snapshot of the files git tracks instead, so git metadata,
# build directories and other untracked files never reach the builder. The
# snapshot keeps uncommitted changes; new files must be added to the index.
#
# Usage: build-with-ton-checkout.sh <ton-checkout> <docker build arguments>...

ton_dir="${1:?expected a TON checkout directory}"
shift

git -C "${ton_dir}" rev-parse --is-inside-work-tree >/dev/null
snapshot=$(mktemp -d)
trap 'rm -rf "${snapshot}"' EXIT

# The index also lists tracked files deleted from the working tree.
git -C "${ton_dir}" ls-files -z --recurse-submodules |
    (
        cd "${ton_dir}"
        while IFS= read -r -d '' path; do
            if [[ -e "${path}" || -L "${path}" ]]; then
                printf '%s\0' "${path}"
            fi
        done | tar --null --files-from - --create --file -
    ) |
    tar --extract --file - --directory "${snapshot}"

docker build --build-context ton-src="${snapshot}" "$@"
