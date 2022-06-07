FROM rust:buster as chef
RUN apt-get update &&\
    apt-get install -y protobuf-compiler
RUN cargo install cargo-chef
WORKDIR app

FROM rust:buster AS runtime
RUN apt-get update &&\
    apt-get install -y supervisor

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY . .
RUN cargo build --bin api

FROM runtime
COPY --from=builder /app/target/debug/api /usr/local/bin/shuttle-backend

COPY docker/entrypoint.sh /bin/entrypoint.sh
COPY docker/supervisord.conf /usr/share/supervisord/supervisord.conf
ENTRYPOINT ["/bin/entrypoint.sh"]
