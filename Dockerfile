# Multi-stage Dockerfile for AetherRelay Rust Gateway

# Pinned to the same toolchain as rust-toolchain.toml and CI. A floating
# `1.98-alpine` tag can drift ahead of the CI toolchain, so the image and the
# verified build stop being the same artifact.
FROM rust:1.98.1-alpine AS builder
RUN apk add --no-cache musl-dev llvm clang lld
WORKDIR /app
COPY . .
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
RUN cargo build --release --target x86_64-unknown-linux-musl
# Stage the full runtime tree here: `FROM scratch` has no shell, so no RUN can
# execute after the switch. The gateway resolves its config as `config/default`
# relative to the working directory and its default database as
# `./data/aether-relay.db`, so both must sit under /app. The CA bundle comes
# along because reqwest+rustls fails on startup ("No CA certificates were
# loaded from the system") in a scratch image with no /etc/ssl/certs.
RUN mkdir -p /out/app/data /out/etc/ssl/certs \
    && cp target/x86_64-unknown-linux-musl/release/aether-relay /out/aether-relay \
    && cp /etc/ssl/certs/ca-certificates.crt /out/etc/ssl/certs/ \
    && chown -R 65534:65534 /out

FROM scratch
LABEL maintainer="Schnee <schnee@users.noreply.github.com>" \
      description="AetherRelay Webhook Gateway v0.2.0"

# One copy for the whole tree: binary at /aether-relay, config+data dir under
# /app, trust store under /etc/ssl/certs.
COPY --from=builder --chown=65534:65534 /out/ /

COPY config/default.toml /app/config/default.toml

# Without an explicit WORKDIR a scratch image starts in `/`, so the gateway
# tried to create `/data` as uid 65534 and died on Permission denied. The
# directory is created and owned above, and declared a volume so the SQLite
# database survives a container restart.
WORKDIR /app
VOLUME ["/app/data"]

ENV RELAY__DATABASE__PATH=/app/data/aether-relay.db
EXPOSE 3000

# Run unprivileged (65534 = nobody).
USER 65534:65534
ENTRYPOINT ["/aether-relay"]
