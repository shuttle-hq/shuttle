#syntax=docker/dockerfile-upstream:1.4.0-rc1
FROM shuttle-build AS planner
ARG crate
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM shuttle-build AS builder
ARG crate
ARG src
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY . .
RUN cargo build --bin ${crate}

FROM shuttle-common
ARG crate
COPY --from=builder /build/target/debug/${crate} /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
