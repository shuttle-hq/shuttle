#!/usr/bin/env bash

###############################################################################
# This file is used by our common Containerfile incase the container for this #
# service might need some extra preparation steps for its final image         #
###############################################################################

# Patch crates to be on same versions
mkdir -p $CARGO_HOME
touch $CARGO_HOME/config.toml
if [[ $PROD != "true" ]]; then
    bash scripts/apply-patches.sh $CARGO_HOME/config.toml /usr/src/shuttle
fi

# Add the wasm32-wasi target
rustup target add wasm32-wasi

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
