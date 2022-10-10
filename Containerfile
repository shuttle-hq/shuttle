#syntax=docker/dockerfile-upstream:1.4.0-rc1
FROM rust:1.63.0-buster as shuttle-build
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
ARG crate
RUN cargo build --bin ${crate}

FROM rust:1.63.0-buster as shuttle-common
RUN apt-get update &&\
    apt-get install -y curl
COPY --from=cache /build/ /usr/src/shuttle/

FROM shuttle-common
ARG crate
SHELL ["/bin/bash", "-c"]
RUN mkdir -p $CARGO_HOME; \
echo $'[patch.crates-io] \n\
shuttle-service = { path = "/usr/src/shuttle/service" } \n\
shuttle-aws-rds = { path = "/usr/src/shuttle/resources/aws-rds" } \n\
shuttle-persist = { path = "/usr/src/shuttle/resources/persist" } \n\
shuttle-shared-db = { path = "/usr/src/shuttle/resources/shared-db" } \n\
shuttle-secrets = { path = "/usr/src/shuttle/resources/secrets" }' > $CARGO_HOME/config.toml
COPY --from=builder /build/target/debug/${crate} /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
