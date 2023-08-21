#syntax=docker/dockerfile-upstream:1.4


# Base image for builds and cache
ARG RUSTUP_TOOLCHAIN
FROM docker.io/lukemathwalker/cargo-chef:latest-rust-${RUSTUP_TOOLCHAIN}-buster as cargo-chef
WORKDIR /build


# Stores source cache and cargo chef recipe
FROM cargo-chef as planner
WORKDIR /src
COPY . .
# Select only the essential files for copying into next steps
# so that changes to miscellaneous files don't trigger a new cargo-chef cook.
# Beware that .dockerignore filters files before they get here.
RUN find . \( \
    -name "*.rs" -or \
    -name "*.toml" -or \
    -name "Cargo.lock" -or \
    -name "*.sql" -or \
    # Used for local TLS testing, as described in admin/README.md
    -name "*.pem" -or \
    -name "ulid0.so" \
    \) -type f -exec install -D \{\} /build/\{\} \;
WORKDIR /build
RUN cargo chef prepare --recipe-path /recipe.json
# TODO upstream: Reduce the cooking by allowing multiple --bin args to prepare, or like this https://github.com/LukeMathWalker/cargo-chef/issues/181


# Builds crate according to cargo chef recipe.
# This step is skipped if the recipe is unchanged from previous build (no dependencies changed).
FROM cargo-chef AS builder
ARG CARGO_PROFILE
COPY --from=planner /recipe.json /
# https://i.imgflip.com/2/74bvex.jpg
RUN cargo chef cook \
    --all-features \
    $(if [ "$CARGO_PROFILE" = "release" ]; then echo --release; fi) \
    --recipe-path /recipe.json
COPY --from=planner /build .
# Building all at once to share build artifacts in the "cook" layer
RUN cargo build \
    $(if [ "$CARGO_PROFILE" = "release" ]; then echo --release; fi) \
    --bin shuttle-auth \
    --bin shuttle-deployer \
    --bin shuttle-provisioner \
    --bin shuttle-gateway \
    --bin shuttle-resource-recorder \
    --bin shuttle-next -F next


# The final image for running each "shuttle-..." binary
ARG RUSTUP_TOOLCHAIN
FROM docker.io/library/rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-crate
ARG CARGO_PROFILE
ARG folder
ARG crate
ARG prepare_args
# used as env variable in prepare script
ARG PROD

# Individual preparation of images
COPY ${folder}/prepare.sh /prepare.sh
RUN /prepare.sh "${prepare_args}"

# shuttle-next is only needed in deployer but is now installed in all images.
# can be improved, but does not hurt much.
COPY --from=builder /build/target/${CARGO_PROFILE}/shuttle-next /usr/local/cargo/bin/

COPY --from=builder /build/target/${CARGO_PROFILE}/${crate} /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
