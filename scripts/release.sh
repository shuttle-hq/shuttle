#!/usr/bin/env bash
#
# Little script to release a new version.
# Usage: release.sh x.y.z
#
# Dependencies: git, cargo-edit, ripgrep

set -uo pipefail

function update-cargo-versions()
{
    local version=$1

    cargo set-version --workspace $version
    git commit -am "chore: v$version"
}

function update-examples-versions()
{
    local version=$1

    rg "shuttle-service = \{ version" --files-with-matches -g '!www/*' | xargs sed -i "s/shuttle-service = { version = \"[[:digit:]]*.[[:digit:]]*.[[:digit:]]*\"/shuttle-service = { version = \"$version\"/g"
    git commit -am "docs: v$version"
}

function main()
{
    version=$1

    echo $version | rg "\d+\.\d+\.\d+" || { echo "first argument must be in the form x.y.z"; exit 1; }

    echo "Will try to update to version $version"
    git checkout -b "chore/v$version"

    update-cargo-versions $version
    update-examples-versions $version

    echo "Success!! You can now merge this branch"
    echo ""
    echo "Thereafter run:"
    echo "./publish.sh $version"
}

main "${1-*}"
