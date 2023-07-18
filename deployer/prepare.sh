#!/usr/bin/env bash

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

# Patch crates to be on same versions
mkdir -p $CARGO_HOME
touch $CARGO_HOME/config.toml
if [[ $PROD != "true" ]]; then
    echo '
    [patch.crates-io]
    shuttle-codegen = { path = "/usr/src/shuttle/codegen" }
    shuttle-common = { path = "/usr/src/shuttle/common" }
    shuttle-proto = { path = "/usr/src/shuttle/proto" }
    shuttle-runtime = { path = "/usr/src/shuttle/runtime" }
    shuttle-service = { path = "/usr/src/shuttle/service" }

    shuttle-aws-rds = { path = "/usr/src/shuttle/resources/aws-rds" }
    shuttle-persist = { path = "/usr/src/shuttle/resources/persist" }
    shuttle-shared-db = { path = "/usr/src/shuttle/resources/shared-db" }
    shuttle-secrets = { path = "/usr/src/shuttle/resources/secrets" }
    shuttle-static-folder = { path = "/usr/src/shuttle/resources/static-folder" }
    shuttle-metadata = { path = "/usr/src/shuttle/resources/metadata" }
    shuttle-turso = { path = "/usr/src/shuttle/resources/turso" }

    shuttle-actix-web = { path = "/usr/src/shuttle/services/shuttle-actix-web" }
    shuttle-axum = { path = "/usr/src/shuttle/services/shuttle-axum" }
    shuttle-next = { path = "/usr/src/shuttle/services/shuttle-next" }
    shuttle-poem = { path = "/usr/src/shuttle/services/shuttle-poem" }
    shuttle-poise = { path = "/usr/src/shuttle/services/shuttle-poise" }
    shuttle-rocket = { path = "/usr/src/shuttle/services/shuttle-rocket" }
    shuttle-salvo = { path = "/usr/src/shuttle/services/shuttle-salvo" }
    shuttle-serenity = { path = "/usr/src/shuttle/services/shuttle-serenity" }
    shuttle-thruster = { path = "/usr/src/shuttle/services/shuttle-thruster" }
    shuttle-tide = { path = "/usr/src/shuttle/services/shuttle-tide" }
    shuttle-tower = { path = "/usr/src/shuttle/services/shuttle-tower" }
    shuttle-warp = { path = "/usr/src/shuttle/services/shuttle-warp" }' > $CARGO_HOME/config.toml
fi

# Add the wasm32-wasi target for next
rustup target add wasm32-wasi
# Add the wasm32 target for frontend frameworks
rustup target add wasm32-unknown-unknown

# Install common build tools for external crates
# The image should already have these: https://github.com/docker-library/buildpack-deps/blob/65d69325ad741cea6dee20781c1faaab2e003d87/debian/buster/Dockerfile
apt update
apt install -y curl llvm-dev libclang-dev clang cmake

# Install protoc since some users may need it
ARCH="linux-x86_64" && \
VERSION="22.2" && \
curl -OL "https://github.com/protocolbuffers/protobuf/releases/download/v$VERSION/protoc-$VERSION-$ARCH.zip" && \
    unzip -o "protoc-$VERSION-$ARCH.zip" bin/protoc "include/*" -d /usr/local && \
    rm -f "protoc-$VERSION-$ARCH.zip"

# Binstall
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# Common cargo build tools
cargo binstall -y --locked trunk@0.17.2

while getopts "p," o; do
case $o in
    "p") # if panamax is used, the '-p' parameter is passed
        # Make future crates requests to our own mirror
        # This is done after shuttle-next install in order to not sabotage it
        echo '
[source.shuttle-crates-io-mirror]
registry = "sparse+http://panamax:8080/index/"
[source.crates-io]
replace-with = "shuttle-crates-io-mirror"' >> $CARGO_HOME/config.toml
            ;;
        *)
            ;;
    esac
done
