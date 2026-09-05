#!/bin/bash
set -Eeuo pipefail

# Prepare /opt in a disposable build stage, after compilation and before the
# final runtime COPY. Deleting files after that COPY would leave their bytes in
# an earlier image layer. Extraction and strip tools stay in the build stage.
# These packaging rules depend on the pinned TON/indexer artifacts: recheck
# library compatibility and runtime data usage when updating those versions.

# pytonlib ships libraries for multiple operating systems and architectures.
# Select the Linux library using Docker TARGETARCH, not the build host's CPU.
target_arch="${1:?expected Docker target architecture}"
case "${target_arch}" in
    arm64) tonlib_name=libtonlibjson.aarch64.so ;;
    amd64) tonlib_name=libtonlibjson.x86_64.so ;;
    *) echo "Unsupported Docker architecture: ${target_arch}" >&2; exit 1 ;;
esac

started=${SECONDS}
# Apparent /opt sizes measure retained file bytes, not compressed pull size or
# the complete image size; use the exported image's layers for those totals.
before=$(du -sb /opt | cut -f1)
work_dir=$(mktemp -d)
trap 'rm -rf "${work_dir}"' EXIT

# Each release AppImage bundles an executable and libraries in a filesystem
# image. Run the extracted ELF directly so the container needs neither FUSE nor
# extraction at startup. Keep one copy of identical libraries in /opt/ton/lib,
# which the runtime Dockerfile includes in LD_LIBRARY_PATH.
for binary in create-state dht-server fift generate-random-id lite-client validator-engine validator-engine-console; do
    (
        cd "${work_dir}"
        "/opt/ton/${binary}" --appimage-extract >/dev/null
    )
    extracted="${work_dir}/squashfs-root"
    for library in "${extracted}/usr/lib/"*; do
        # Use Ubuntu's libstdc++ and libatomic. The pinned AppImage's older
        # libstdc++ would take precedence through LD_LIBRARY_PATH and break
        # indexer binaries that require GLIBCXX_3.4.31 / GLIBCXX_3.4.32.
        case "${library##*/}" in libstdc++.so.6 | libatomic.so.1) continue ;; esac
        destination="/opt/ton/lib/${library##*/}"
        if [[ -e "${destination}" ]]; then
            # A SONAME collision with different bytes must not silently change
            # the library used by another TON executable.
            cmp "${library}" "${destination}"
        else
            cp -a "${library}" "${destination}"
        fi
    done
    install -m 0755 "${extracted}/usr/bin/${binary}" "/opt/ton/${binary}"
    rm -rf "${extracted}"
done

venv=/opt/ton-indexer/venv
# Dependencies are already installed; runtime services do not install packages
# or run pytest. Uninstall through pip to remove their metadata and entry points
# too. Keep setuptools: pytoniq-core declares it as a runtime dependency.
"${venv}/bin/python" -m pip uninstall --yes pip pytest pytest-asyncio
site_packages="${venv}/lib/python3.12/site-packages"
distlib="${site_packages}/pytonlib/distlib"
# Fail before pruning if an upstream package changes the expected library path.
# Retain the selected library in place so pytonlib's lookup path still matches.
test -f "${distlib}/linux/${tonlib_name}"
find "${distlib}" -type f ! -path "${distlib}/linux/${tonlib_name}" -delete
find "${distlib}" -depth -type d -empty -delete

# The classifier directory is copied wholesale from upstream, including a
# ~24.9 MB JSON export (uncompressed) that the pinned version no longer reads.
# In indexer/indexer/events/blocks/utils/dedust_pools.py, init_pools_data_sync()
# selects address, asset_1 and asset_2 from dex_pools WHERE dex = 'dedust'. The
# C++ indexer populates that table; a background updater distributes it through
# Redis. An empty query result produces an empty pool map, with no JSON fallback.
# Recheck this loading path when changing COMMIT in xtask/src/http_api_v3.rs or
# overriding it with Docker's TON_INDEXER_COMMIT build argument.
rm -f /opt/ton-indexer/classifier/dedust_pools.json

# These directories contain classifier tests and cryptography self-tests, not
# the implementations used by services. Keep package code, metadata and other
# data files; removing arbitrary test-like paths could break runtime resources.
rm -rf /opt/ton-indexer/classifier/tests \
    "${site_packages}/Crypto/SelfTest" "${site_packages}/Cryptodome/SelfTest"

# Static archives are linker inputs; the runtime loads the shared marker library.
find /opt/ton-indexer/lib -type f -name '*.a' -delete
# Drop debug information and symbols unnecessary for relocation, preserving
# dynamic symbols needed by shared libraries and Python extension imports.
# This trades native debugging detail in the runtime image for smaller binaries.
find /opt/ton-indexer/bin /opt/ton-indexer/lib "${site_packages}" \
    -type f \( -name '*.so' -o -name '*.so.*' \) \
    -exec strip --strip-unneeded {} +
strip --strip-unneeded /opt/ton-indexer/bin/* /opt/ton/create-state \
    /opt/ton/dht-server /opt/ton/fift /opt/ton/generate-random-id \
    /opt/ton/lite-client /opt/ton/validator-engine /opt/ton/validator-engine-console

# Builder imports leave bytecode caches alongside Python source. Python can
# compile that source on demand, even when it cannot write a new cache file.
# Prune last because pip and other preparation steps can recreate these caches.
find "${venv}" /opt/ton-indexer/classifier \
    -type d -name __pycache__ -prune -exec rm -rf {} +
after=$(du -sb /opt | cut -f1)
printf 'operation=prepare_runtime target=%s duration_seconds=%s bytes_before=%s bytes_after=%s outcome=complete\n' \
    "${target_arch}" "$((SECONDS - started))" "${before}" "${after}"
