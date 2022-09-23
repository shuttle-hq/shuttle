#!/usr/bin/env bash
#
# Little script to update all cargo dependencies
#
# Dependencies: git, cargo-edit

set -ueo pipefail

function upgrade-workspace()
{
    echo "upgrading workspace..."

    cargo upgrade --workspace --verbose
    cargo check
    git commit -am "chore: upgrade workspace dependencies"
}

function upgrade-examples()
{
    echo "upgrading examples..."

    for d in examples/*/*/;
    do
        cd "$d"

        if [[ -f Cargo.toml ]]
        then
            rm Cargo.lock
            cargo upgrade --verbose
            cargo check
            cargo clean
        fi

        cd ../../../
    done

    git commit -am "docs: upgrade example dependencies"
}

function upgrade-tests-resources()
{
    echo "upgrading tests resources..."

    for d in service/tests/resources/*/;
    do
        cd "$d"

        if [[ -f Cargo.toml ]]
        then
            cargo upgrade --verbose
            cargo check
            cargo clean
        fi

        cd ../../../../
    done

    git commit -am "refactor: upgrade tests resources dependencies"
}

function main()
{
    git checkout -b "chore/dependencies-upgrades"

    upgrade-workspace
    upgrade-examples
    upgrade-tests-resources

    echo "Upgrades all done!!"
}

main "${1-*}"
