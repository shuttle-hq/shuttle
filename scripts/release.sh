#!/usr/bin/env bash
#
# Little script to release a new version.
# Usage: release.sh x.y.z
#
# Dependencies: git, cargo-edit, ripgrep

set -ueo pipefail

function update-cargo-versions()
{
    local version=$1

    cargo set-version --workspace $version
    git commit -am "chore: v$version"
}

function update-examples-versions()
{
    local version=$1

    for d in examples/*/*/;
    do
        cd "$d"

        if [[ -f Cargo.toml ]]
        then
            cargo add shuttle-service@$version
        fi

        cd ../../../
    done

    # Update docs in service and README
    rg "shuttle-service = \{ version" --files-with-matches service/ | xargs sed -i "s/shuttle-service = { version = \"[[:digit:]]*.[[:digit:]]*.[[:digit:]]*\"/shuttle-service = { version = \"$version\"/g"
    sed -i "s/shuttle-service = { version = \"[[:digit:]]*.[[:digit:]]*.[[:digit:]]*\"/shuttle-service = { version = \"$version\"/g" README.md

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
