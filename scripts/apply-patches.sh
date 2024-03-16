#!/usr/bin/env bash

# Use this to create a proper cargo config file with Shuttle's crates.io patches.
# The output file is overwritten if it exists.
#
# Usage:
#     ./scripts/apply-patches.sh [OUTPUT] [ROOT]
#
# Args:
#     OUTPUT - output config file   (default: .cargo/config.toml)
#     ROOT   - path to shuttle repo (default: $(pwd))

OUT=".cargo/config.toml"
if [ -n "$1" ]; then
    OUT="$1"
fi

ROOT="$(pwd)"
if [ -n "$2" ]; then
    ROOT="$2"
fi

mkdir -p "$(dirname "$OUT")"

sed "s|BASE|$ROOT|" "$ROOT/scripts/patches.toml" > "$OUT"
