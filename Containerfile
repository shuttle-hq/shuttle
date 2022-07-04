#syntax=docker/dockerfile-upstream:1.4.0-rc1
FROM rust:buster as shuttle-build
RUN apt-get update &&\
    apt-get install -y curl protobuf-compiler
RUN cargo install cargo-chef
WORKDIR /build

FROM shuttle-build as cache
ARG SRC_CRATES
WORKDIR /src
COPY . .
RUN find ${SRC_CRATES} \( -name "*.proto" -or -name "*.rs" -or -name "*.toml" \) -type f -exec install -D \{\} /build/\{\} \;

FROM shuttle-build AS planner
ARG crate
COPY --from=cache /build .
RUN cargo chef prepare --recipe-path recipe.json

FROM shuttle-build AS builder
ARG crate
ARG src
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY --from=cache /build .
RUN cargo build --bin ${crate}

FROM rust:buster as shuttle-common
RUN apt-get update &&\
    apt-get install -y curl

FROM shuttle-common
ARG crate
COPY --from=builder /build/target/debug/${crate} /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
