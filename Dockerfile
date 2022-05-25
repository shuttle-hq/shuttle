FROM rust:buster as chef
RUN cargo install cargo-chef
WORKDIR app

FROM rust:buster AS runtime
RUN apt-get update &&\
    apt-get install -y curl postgresql supervisor python3-pip
RUN pg_dropcluster $(pg_lsclusters -h | cut -d' ' -f-2 | head -n1)

WORKDIR /opt

RUN pip3 install numpy
RUN pip3 install --pre torch torchvision torchaudio -f https://download.pytorch.org/whl/nightly/cpu/torch_nightly.html

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

ENV LIBTORCH=${TORCH_HOME}
ENV LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH

COPY docker/entrypoint.sh /bin/entrypoint.sh
COPY docker/wait-for-pg-then /usr/bin/wait-for-pg-then
COPY docker/supervisord.conf /usr/share/supervisord/supervisord.conf
ENTRYPOINT ["/bin/entrypoint.sh"]
