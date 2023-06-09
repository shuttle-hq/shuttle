#syntax=docker/dockerfile-upstream:1.4
ARG RUSTUP_TOOLCHAIN
FROM docker.io/library/rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-build
RUN apt update && apt install -y curl

RUN cargo install cargo-chef --locked
WORKDIR /build

FROM shuttle-build as cache
WORKDIR /src
COPY . .
ARG CARGO_PROFILE
RUN find ${SRC_CRATES} \( -name "*.proto" -or -name "*.rs" -or -name "*.toml" -or -name "Cargo.lock" -or -name "README.md" -or -name "*.sql" \) -type f -exec install -D \{\} /build/\{\} \;
# This is used to carry over in the docker images any *.pem files from shuttle root directory,
# to be used for TLS testing, as described here in the admin README.md.
RUN [ "$CARGO_PROFILE" != "release" ] && \
    find ${SRC_CRATES} -name "*.pem" -type f -exec install -D \{\} /build/\{\} \;

FROM shuttle-build AS planner
COPY --from=cache /build .
RUN cargo chef prepare --recipe-path recipe.json

FROM shuttle-build AS builder
COPY --from=planner /build/recipe.json recipe.json
ARG CARGO_PROFILE
RUN cargo chef cook \
    # if CARGO_PROFILE is release, pass --release, else use default debug profile
    $(if [ "$CARGO_PROFILE" = "release" ]; then echo --release; fi) \
    --recipe-path recipe.json
COPY --from=cache /build .
ARG folder
RUN cargo build --bin shuttle-${folder} \
    $(if [ "$CARGO_PROFILE" = "release" ]; then echo --release; fi)

ARG RUSTUP_TOOLCHAIN
FROM rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-common
RUN rustup component add rust-src

COPY --from=cache /build/ /usr/src/shuttle/

FROM shuttle-common as shuttle-deployer
ARG folder
ARG prepare_args
ARG PROD
ARG CARGO_PROFILE
COPY ${folder}/prepare.sh /prepare.sh
RUN /prepare.sh "${prepare_args}"
COPY --from=builder /build/target/${CARGO_PROFILE}/shuttle-${folder} /usr/local/bin/service
ARG RUSTUP_TOOLCHAIN
ENV RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}
ENTRYPOINT ["/usr/local/bin/service"]
