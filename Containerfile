#syntax=docker/dockerfile-upstream:1.4.0-rc1
FROM rust:buster as shuttle-build
RUN apt-get update &&\
    apt-get install -y curl protobuf-compiler
RUN cargo install cargo-chef
WORKDIR /build

FROM shuttle-build as cache
WORKDIR /src
COPY . .
RUN find ${SRC_CRATES} \( -name "*.proto" -or -name "*.rs" -or -name "*.toml" \) -type f -exec install -D \{\} /build/\{\} \;

FROM shuttle-build AS planner
COPY --from=cache /build .
RUN cargo chef prepare --recipe-path recipe.json

FROM shuttle-build AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY --from=cache /build .
ARG crate
RUN cargo build --bin ${crate}

FROM rust:buster as shuttle-common
RUN apt-get update &&\
    apt-get install -y curl

FROM shuttle-common
ARG crate
COPY --from=builder /build/target/debug/${crate} /usr/local/bin/service
# [s] invokes Glob functionality so if assets doesn't exist, the container won't fail https://stackoverflow.com/questions/70096208/dockerfile-copy-folder-if-it-exists-conditional-copy
# Likely MUCH better way
COPY --from=builder /build/asset[s]/ /usr/local/bin/assets/
ENTRYPOINT ["/usr/local/bin/service"]
