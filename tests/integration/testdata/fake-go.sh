#!/bin/sh
set -eu
[ "$#" -eq 10 ]
[ "$1" = "run" ]
[ "$2" = "-mod=readonly" ]
[ "$3" = "-trimpath" ]
[ "$4" = "./cmd/tolk-abi-to-go" ]
[ "$CGO_ENABLED" = "0" ]
[ "$GOWORK" = "off" ]
[ "$GOFLAGS" = " " ]
[ -f go.mod ]
[ -f go.sum ]
[ -f LICENSE ]
[ -f codegen/generate.go ]
[ -f cmd/tolk-abi-to-go/main.go ]
grep -q '^module github.com/ton-blockchain/acton/packages/abi-go$' go.mod
if [ -n "${ACTON_TEST_GO_WORKDIR:-}" ]; then
    pwd -P > "$ACTON_TEST_GO_WORKDIR"
fi
shift 4
[ "$1" = "--catalog" ]
[ "$3" = "--output-dir" ]
[ "$5" = "--package" ]
case "$2" in /*) ;; *) exit 2 ;; esac
case "$4" in /*) ;; *) exit 2 ;; esac
printf '%s\n' 'fake Go generator stdout'
printf '%s\n' 'fake Go generator stderr' >&2
if [ "$6" = "fail" ]; then
    exit 23
fi
mkdir -p "$4"
cp "$2" "$4/input.json"
printf '%s\n' "$2" > "$4/input-path.txt"
pwd -P > "$4/module-path.txt"
printf '%s\n' "$6" > "$4/package.txt"
printf '%s\n' "package $6" > "$4/registry_gen.go"
printf '%s\n' invocation >> "$4/invocations.txt"
