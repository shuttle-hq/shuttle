#!/bin/bash

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

# Stuff that depends on local source files
if [ "$1" = "--after-src" ]; then
    exit 0
fi

# Patch crates to be on same versions
mkdir -p $CARGO_HOME
touch $CARGO_HOME/config.toml
if [[ $PROD != "true" ]]; then
    echo '
    [patch.crates-io]
    shuttle-common = { path = "/usr/src/shuttle/common" }
    shuttle-proto = { path = "/usr/src/shuttle/proto" }' > $CARGO_HOME/config.toml
fi