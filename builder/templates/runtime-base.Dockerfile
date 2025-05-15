FROM debian:bookworm-slim AS runtime-base

# ca-certificates for native-tls, curl for health check
RUN apt update \
    && apt install -y ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
