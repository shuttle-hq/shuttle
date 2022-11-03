#!/usr/bin/env sh

# Patch crates to be on same versions
mkdir -p $CARGO_HOME; \
echo '[patch.crates-io]
shuttle-service = { path = "/usr/src/shuttle/service" }
shuttle-aws-rds = { path = "/usr/src/shuttle/resources/aws-rds" }
shuttle-persist = { path = "/usr/src/shuttle/resources/persist" }
shuttle-shared-db = { path = "/usr/src/shuttle/resources/shared-db" }
shuttle-secrets = { path = "/usr/src/shuttle/resources/secrets" }' > $CARGO_HOME/config.toml

# Prefetch crates.io index
cd /usr/src/shuttle/service
cargo fetch
