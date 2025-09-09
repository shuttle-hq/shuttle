#syntax=docker/dockerfile:1.4

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


FROM debian:bookworm-slim AS runtime-base

SHELL ["/bin/bash", "-e", "-o", "pipefail", "-c"]

# ca-certificates for native-tls, curl for health check
RUN <<EOT
apt-get update

DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates \
    curl

apt-get clean
rm -rf /var/lib/apt/lists/*
EOT


FROM cargo-chef AS chef
WORKDIR /app
ENV SHUTTLE=true



FROM chef AS planner
COPY . .
RUN cargo chef prepare



FROM chef AS builder

COPY shuttle_prebuild.sh .
RUN bash shuttle_prebuild.sh




COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --package hello --features asdf


COPY . .


RUN cargo build --release --package hello --features asdf


RUN bash shuttle_postbuild.sh

RUN mv /app/target/release/hello /executable



RUN for path in $(tq -r '.build.assets // .build_assets // [] | join(" ")' Shuttle.toml); do find "$path" -type f -exec echo Copying \{\} \; -exec install -D \{\} /build_assets/\{\} \; ; done


FROM runtime-base AS runtime
WORKDIR /app

COPY --from=builder /app/shuttle_setup_container.sh /tmp
RUN bash /tmp/shuttle_setup_container.sh; rm /tmp/shuttle_setup_container.sh

COPY --from=builder /build_assets /app
COPY --from=builder /executable /usr/local/bin/runtime

ENTRYPOINT ["/usr/local/bin/runtime"]
