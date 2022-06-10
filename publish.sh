#!/usr/bin/env bash
#
# Little script to publish to crates.io
# Usage: publish.sh x.y.z

set -uo pipefail

function publish-folder()
{
    local folder=$1

    echo "Publishing $folder"
    cd $folder
    cargo publish
    cd ..
}

function main()
{
    version=$1

    echo $version | rg "\d+\.\d+\.\d+" || { echo "first argument must be in the form x.y.z"; exit 1; }

    publish-folder "common"
    publish-folder "codegen"
    publish-folder "service"
    publish-folder "cargo-shuttle"

    git tag "v$version"
    git push --tags

    echo "Success!! Now tell about it on Discord :D"
}

main "${1-*}"
