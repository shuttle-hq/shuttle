FROM rust:buster as chef
RUN cargo install cargo-chef
WORKDIR app

FROM debian:latest AS runtime
RUN apt-get update &&\
    apt-get install -y curl postgresql supervisor python3-pip
RUN pg_dropcluster $(pg_lsclusters -h | cut -d' ' -f-2 | head -n1)

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

RUN pip3 install torch -f https://torch.kmtea.eu/whl/stable.html -f https://ext.kmtea.eu/whl/stable.html

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

ENV LIBTORCH=/usr/local/lib/python3.9/dist-packages/torch/
ENV LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH

COPY docker/entrypoint.sh /bin/entrypoint.sh
COPY docker/wait-for-pg-then /usr/bin/wait-for-pg-then
COPY docker/supervisord.conf /usr/share/supervisord/supervisord.conf
ENTRYPOINT ["/bin/entrypoint.sh"]
