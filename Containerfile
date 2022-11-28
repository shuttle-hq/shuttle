#syntax=docker/dockerfile-upstream:1.4.0-rc1
FROM rust:1.64.0-buster as shuttle-build
RUN apt-get update &&\
    apt-get install -y curl protobuf-compiler
RUN cargo install cargo-chef
WORKDIR /build

FROM shuttle-build as cache
WORKDIR /src
COPY . .
RUN find ${SRC_CRATES} \( -name "*.proto" -or -name "*.rs" -or -name "*.toml" -or -name "README.md" -or -name "*.sql" \) -type f -exec install -D \{\} /build/\{\} \;

FROM shuttle-build AS planner
COPY --from=cache /build .
RUN cargo chef prepare --recipe-path recipe.json

FROM shuttle-build AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY --from=cache /build .
ARG folder
RUN cargo build --bin shuttle-${folder}

FROM rust:1.64.0-buster as shuttle-common
RUN apt-get update &&\
    apt-get install -y curl
RUN rustup component add rust-src
COPY --from=cache /build/ /usr/src/shuttle/

FROM shuttle-common
ARG folder
COPY ${folder}/prepare.sh /prepare.sh
RUN /prepare.sh
COPY --from=builder /build/target/debug/shuttle-${folder} /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
