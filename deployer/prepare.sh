#!/usr/bin/env sh

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

cargo install --git https://github.com/paritytech/cachepot

# Patch crates to be on same versions
mkdir -p $CARGO_HOME; \
echo '[patch.crates-io]
shuttle-service = { path = "/usr/src/shuttle/service" }
shuttle-aws-rds = { path = "/usr/src/shuttle/resources/aws-rds" }
shuttle-persist = { path = "/usr/src/shuttle/resources/persist" }
shuttle-shared-db = { path = "/usr/src/shuttle/resources/shared-db" }
shuttle-secrets = { path = "/usr/src/shuttle/resources/secrets" }
shuttle-static-folder = { path = "/usr/src/shuttle/resources/static-folder" }

[build]
rustc-wrapper = "cachepot"
' > $CARGO_HOME/config.toml

# Prefetch crates.io index
cd /usr/src/shuttle/service
cargo fetch

# cachepot client config
mkdir -p ~/.config/cachepot
echo '[dist]
# The URL used to connect to the scheduler (should use https, given an ideal
# setup of a HTTPS worker in front of the scheduler)
scheduler_url = "http://cachepot-scheduler:10600"
# Used for mapping local toolchains to remote cross-compile toolchains. Empty in
# this example where the client and build worker are both Linux.
toolchains = []
# Size of the local toolchain cache, in bytes (5GB here, 10GB if unspecified).
toolchain_cache_size = 5368709120

[dist.auth]
type = "token"
# This should match the `client_auth` section of the scheduler config.
token = "shuttle-deployer"' > ~/.config/cachepot/config
