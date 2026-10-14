#!/bin/bash
set -Eeuo pipefail

# Build native TON from the checkout in the current directory and install it
# into /opt/ton with the official release layout: executables at the top level,
# Fift libraries in lib/ and smart contract sources in smartcont/. Unlike the
# release, the result also contains create-hardfork.

clang_version="${1:?expected clang major version}"
jobs="${2:?expected build job count}"
build_dir=/tmp/ton-build
binaries=(
    create-hardfork
    create-state
    dht-server
    fift
    generate-random-id
    lite-client
    validator-engine
    validator-engine-console
)

if [[ ! -f CMakeLists.txt ]]; then
    echo "TON sources are missing; pass --build-context ton-src=/path/to/ton" >&2
    exit 1
fi
# Git leaves an uninitialized submodule as an empty directory.
while read -r _ path; do
    if [[ -z "$(ls -A "${path}" 2>/dev/null)" ]]; then
        echo "TON submodule ${path} is empty; run 'git submodule update --init --recursive' in the checkout" >&2
        exit 1
    fi
done < <(git config --file .gitmodules --get-regexp '\.path$' || true)

started=${SECONDS}
# TON generates sources inside the checkout, so this builds a writable copy.
# An empty TON_ARCH keeps the compiler's baseline instead of -march=native.
cmake -S . -B "${build_dir}" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER="clang-${clang_version}" \
    -DCMAKE_CXX_COMPILER="clang++-${clang_version}" \
    -DPORTABLE=1 \
    -DTON_ARCH=
# gen_fif generates smartcont/auto, which the localton zerostate script includes.
cmake --build "${build_dir}" --parallel "${jobs}" --target "${binaries[@]}" gen_fif

for binary in "${binaries[@]}"; do
    mapfile -t matches < <(find "${build_dir}" -type f -name "${binary}" -perm -u+x)
    if (( ${#matches[@]} != 1 )); then
        echo "expected one ${binary} executable in the build, found ${#matches[@]}" >&2
        exit 1
    fi
    install -Dm755 "${matches[0]}" "/opt/ton/${binary}"
done
cp -a crypto/fift/lib /opt/ton/lib
cp -a crypto/smartcont /opt/ton/smartcont

# Like the release AppImages, carry the shared libraries that the runtime image
# does not install, such as GSL, BLAS and readline. The runtime image provides
# glibc, the GCC runtime and zlib itself.
dependencies=$(cd /opt/ton && ldd "${binaries[@]}")
if grep --quiet 'not found' <<<"${dependencies}"; then
    printf '%s\n' "${dependencies}" >&2
    exit 1
fi
awk '$2 == "=>" && $3 ~ /^\// { print $3 }' <<<"${dependencies}" | sort -u |
    while read -r library; do
        case "${library##*/}" in
            ld-linux* | libc.so.* | libdl.so.* | libm.so.* | libpthread.so.* | libresolv.so.* | librt.so.* \
                | libgcc_s.so.* | libstdc++.so.* | libatomic.so.* | libz.so.*) continue ;;
        esac
        cp -L "${library}" /opt/ton/lib/
    done

chmod -R a+rX /opt/ton
LD_LIBRARY_PATH=/opt/ton/lib /opt/ton/generate-random-id -m keys -n 1 >/dev/null
printf 'operation=build_ton duration_seconds=%s outcome=complete\n' "$((SECONDS - started))"
