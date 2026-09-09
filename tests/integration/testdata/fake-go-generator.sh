#!/bin/sh
set -eu
[ "$#" -eq 6 ]
[ "$1" = "--catalog" ]
[ "$3" = "--output-dir" ]
[ "$5" = "--package" ]
printf '%s\n' 'fake Go generator stdout'
printf '%s\n' 'fake Go generator stderr' >&2
if [ "$6" = "fail" ]; then
    exit 23
fi
mkdir -p "$4"
cp "$2" "$4/input.json"
printf '%s\n' "$2" > "$4/input-path.txt"
printf '%s\n' "$6" > "$4/package.txt"
printf '%s\n' "package $6" > "$4/registry_gen.go"
printf '%s\n' invocation >> "$4/invocations.txt"
