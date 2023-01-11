#syntax=docker/dockerfile-upstream:1.4.0-rc1
ARG RUSTUP_TOOLCHAIN
FROM rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-build
RUN apt-get update &&\
    apt-get install -y curl
# download protoc binary and unzip it in usr/bin
RUN curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v21.9/protoc-21.9-linux-x86_64.zip &&\
    unzip -o protoc-21.9-linux-x86_64.zip -d /usr bin/protoc &&\
    unzip -o protoc-21.9-linux-x86_64.zip -d /usr/ 'include/*' &&\
    rm -f protoc-21.9-linux-x86_64.zip
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

ARG RUSTUP_TOOLCHAIN
FROM rust:${RUSTUP_TOOLCHAIN}-buster as shuttle-common
RUN apt-get update &&\
    apt-get install -y curl
RUN rustup component add rust-src
COPY --from=cache /build/ /usr/src/shuttle/

FROM shuttle-common
ARG folder
ARG prepare_args
COPY ${folder}/prepare.sh /prepare.sh
RUN /prepare.sh "${prepare_args}"
COPY --from=builder /build/target/debug/shuttle-${folder} /usr/local/bin/service
ARG RUSTUP_TOOLCHAIN
ENV RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN}
ENTRYPOINT ["/usr/local/bin/service"]
