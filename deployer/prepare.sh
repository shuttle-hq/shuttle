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

# Prefetch crates.io index
cd /usr/src/shuttle/service
cargo fetch
