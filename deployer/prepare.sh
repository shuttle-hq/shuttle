#!/usr/bin/env sh

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

# Patch crates to be on same versions
mkdir -p $CARGO_HOME; \
echo '[patch.crates-io]
shuttle-service = { path = "/usr/src/shuttle/service" }
shuttle-aws-rds = { path = "/usr/src/shuttle/resources/aws-rds" }
shuttle-persist = { path = "/usr/src/shuttle/resources/persist" }
shuttle-shared-db = { path = "/usr/src/shuttle/resources/shared-db" }
shuttle-secrets = { path = "/usr/src/shuttle/resources/secrets" }
shuttle-static-folder = { path = "/usr/src/shuttle/resources/static-folder" }' > $CARGO_HOME/config.toml

# Add the wasm32-wasi target
rustup target add wasm32-wasi

# Install the shuttle runtime
cargo install shuttle-runtime --path "/usr/src/shuttle/runtime" --bin shuttle-next --features next

# Make future crates requests to our own mirror
echo '
[source.shuttle-crates-io-mirror]
registry = "http://panamax:8080/git/crates.io-index"
[source.crates-io]
replace-with = "shuttle-crates-io-mirror"' >> $CARGO_HOME/config.toml

# Prefetch crates.io index from our mirror
# TODO: restore when we know how to prefetch from our mirror
# cd /usr/src/shuttle/service
# cargo fetch
