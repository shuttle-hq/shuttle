FROM lukemathwalker/cargo-chef:latest AS cargo-chef

SHELL ["/bin/bash", "-e", "-o", "pipefail", "-c"]

RUN <<EOT
# Files and directories used by the Shuttle build process:
mkdir /build_assets
mkdir /app
# Create empty files in place for optional user scripts, etc.
# Having them empty means we can skip checking for them with [ -f ... ] etc.
touch /app/Shuttle.toml
touch /app/shuttle_prebuild.sh
touch /app/shuttle_postbuild.sh
touch /app/shuttle_setup_container.sh
EOT

# Install common build tools for external crates
# The image should already have these: https://github.com/docker-library/buildpack-deps/blob/fdfe65ea0743aa735b4a5f27cac8e281e43508f5/debian/bookworm/Dockerfile
RUN <<EOT
apt-get update

DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    clang \
    cmake \
    jq \
    llvm-dev \
    libclang-dev \
    mold \
    protobuf-compiler

apt-get clean
rm -rf /var/lib/apt/lists/*
EOT

# Add the wasm32 target for building frontend frameworks
RUN rustup target add wasm32-unknown-unknown

# cargo binstall
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# Utility tools for build process
RUN cargo binstall -y --locked convert2json@1.1.5

# Common cargo build tools (for the user to use)
RUN cargo binstall -y --locked trunk@0.21.7
