#syntax=docker/dockerfile-upstream:1.4


# Base image for builds and cache
ARG RUSTUP_TOOLCHAIN
FROM docker.io/lukemathwalker/cargo-chef:latest-rust-${RUSTUP_TOOLCHAIN}-buster as cargo-chef
WORKDIR /build


# Stores source cache
FROM cargo-chef as cache
WORKDIR /src
COPY . .
# Select only the essential files for copying into next steps
RUN find . \( \
    -name "*.rs" -or \
    -name "*.toml" -or \
    -name "Cargo.lock" -or \
    -name "*.sql" -or \
    # Used for local TLS testing, as described in admin/README.md
    -name "*.pem" -or \
    -name "ulid0.so" \
    \) -type f -exec install -D \{\} /build/\{\} \;


# Stores cargo chef recipe
FROM cargo-chef AS planner
COPY --from=cache /build .
RUN cargo chef prepare --recipe-path /recipe.json


# Builds crate according to cargo chef recipe
FROM cargo-chef AS builder
COPY --from=planner /recipe.json /
RUN cargo chef cook \
    # --release \
    --recipe-path /recipe.json
COPY --from=cache /build .
# Building all at once to share build artifacts in the "cook" layer
RUN cargo build \
    # --release \
    --bin shuttle-auth \
    --bin shuttle-deployer \
    --bin shuttle-provisioner \
    --bin shuttle-gateway \
    --bin shuttle-resource-recorder \
    --bin shuttle-next -F next


# The final image for running each "shuttle-..." binary
ARG RUSTUP_TOOLCHAIN
FROM docker.io/library/rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-crate
ARG folder
ARG crate
ARG prepare_args
# used as env variable in prepare script
ARG PROD

COPY ${folder}/prepare.sh /prepare.sh
# Prepare steps that don't depend on Shuttle source code
RUN /prepare.sh "${prepare_args}"

# Currently unused:
#    COPY --from=cache /build /usr/src/shuttle/
#    # Any prepare steps that depend on the COPY from cached source code.
#    # In the deployer, shuttle-next is installed and the panamax mirror config is added in this step.
#    RUN /prepare.sh --after-src "${prepare_args}"

# shuttle-next is only needed in deployer but is now installed in all images.
# can be improved, but does not hurt much.
COPY --from=builder /build/target/debug/shuttle-next /usr/local/cargo/bin/

COPY --from=builder /build/target/debug/${crate} /usr/local/bin/service
# COPY --from=builder /build/target/release/${crate} /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
