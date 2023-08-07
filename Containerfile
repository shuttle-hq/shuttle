#syntax=docker/dockerfile-upstream:1.4


# Base image for builds and cache
ARG RUSTUP_TOOLCHAIN
FROM docker.io/library/rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-build
RUN cargo install cargo-chef --locked
WORKDIR /build


# Stores source cache
FROM shuttle-build as cache
ARG CARGO_PROFILE
WORKDIR /src
COPY . .
RUN find ${SRC_CRATES} \( -name "*.proto" -or -name "*.rs" -or -name "*.toml" -or -name "Cargo.lock" -or -name "README.md" -or -name "*.sql" -or -name "ulid0.so" \) -type f -exec install -D \{\} /build/\{\} \;
# This is used to carry over in the docker images any *.pem files from shuttle root directory,
# to be used for TLS testing, as described here in the admin README.md.
RUN if [ "$CARGO_PROFILE" != "release" ]; then \
    find ${SRC_CRATES} -name "*.pem" -type f -exec install -D \{\} /build/\{\} \;; \
    fi


# Stores cargo chef recipe
FROM shuttle-build AS planner
COPY --from=cache /build .
RUN cargo chef prepare --recipe-path recipe.json


# Builds crate according to cargo chef recipe
FROM shuttle-build AS builder
ARG CARGO_PROFILE
ARG folder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook \
    # if CARGO_PROFILE is release, pass --release, else use default debug profile
    $(if [ "$CARGO_PROFILE" = "release" ]; then echo --release; fi) \
    --recipe-path recipe.json
COPY --from=cache /build .
RUN cargo build --bin shuttle-${folder} \
    $(if [ "$CARGO_PROFILE" = "release" ]; then echo --release; fi)


# The final image for this "shuttle-..." crate
ARG RUSTUP_TOOLCHAIN
FROM docker.io/library/rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-crate
ARG folder
ARG prepare_args
# used as env variable in prepare script
ARG PROD
ARG CARGO_PROFILE
ARG RUSTUP_TOOLCHAIN
ENV RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}

COPY ${folder}/prepare.sh /prepare.sh
RUN /prepare.sh "${prepare_args}"

COPY --from=cache /build /usr/src/shuttle/

# Any prepare steps that depend on the COPY from src cache.
# In the deployer shuttle-next is installed and the panamax mirror config is added in this step.
RUN /prepare.sh --after-src "${prepare_args}"

COPY --from=builder /build/target/${CARGO_PROFILE}/shuttle-${folder} /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
