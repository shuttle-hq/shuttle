FROM rust:buster as chef
RUN cargo install cargo-chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin api

FROM debian:buster-slim AS runtime
COPY --from=builder /app/target/release/api /usr/local/bin/unveil-backend
RUN apt-get update &&\
    apt-get install -y curl
ENTRYPOINT ["/usr/local/bin/unveil-backend"]
