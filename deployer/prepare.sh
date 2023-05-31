#!/usr/bin/env sh

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

# Patch crates to be on same versions
mkdir -p $CARGO_HOME
if [[ $PROD != "true" ]]; then
    echo '[patch.crates-io]
    shuttle-service = { path = "/usr/src/shuttle/service" }
    shuttle-runtime = { path = "/usr/src/shuttle/runtime" }

    shuttle-aws-rds = { path = "/usr/src/shuttle/resources/aws-rds" }
    shuttle-persist = { path = "/usr/src/shuttle/resources/persist" }
    shuttle-shared-db = { path = "/usr/src/shuttle/resources/shared-db" }
    shuttle-secrets = { path = "/usr/src/shuttle/resources/secrets" }
    shuttle-static-folder = { path = "/usr/src/shuttle/resources/static-folder" }

    shuttle-axum = { path = "/usr/src/shuttle/services/shuttle-axum" }
    shuttle-actix-web = { path = "/usr/src/shuttle/services/shuttle-actix-web" }
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
else
    touch $CARGO_HOME/config.toml
fi

# Install protoc since some users may need it
ARCH="linux-x86_64" && \
VERSION="22.2" && \
curl -OL "https://github.com/protocolbuffers/protobuf/releases/download/v$VERSION/protoc-$VERSION-$ARCH.zip" && \
    unzip -o "protoc-$VERSION-$ARCH.zip" bin/protoc "include/*" -d /usr/local && \
    rm -f "protoc-$VERSION-$ARCH.zip"

# Add the wasm32-wasi target
rustup target add wasm32-wasi

# Install the shuttle runtime
cargo install shuttle-runtime --path "/usr/src/shuttle/runtime" --bin shuttle-next --features next

while getopts "p," o; do
    case $o in
        "p")
            # Make future crates requests to our own mirror
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

# Prefetch crates.io index from our mirror
# TODO: restore when we know how to prefetch from our mirror
# cd /usr/src/shuttle/service
# cargo fetch

# Install common build tools for external crates
apt update
apt install -y llvm-dev libclang-dev gcc clang cmake make automake autoconf libtool
