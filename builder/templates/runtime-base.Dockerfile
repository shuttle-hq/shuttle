#syntax=docker/dockerfile:1.4

FROM debian:bookworm-slim AS runtime-base

SHELL ["/bin/bash", "-e", "-o", "pipefail", "-c"]

# ca-certificates for native-tls, curl for health check
RUN <<EOT
apt-get update

DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates \
    curl

apt-get clean
rm -rf /var/lib/apt/lists/*
EOT
