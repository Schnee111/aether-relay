# Multi-stage Dockerfile for AetherRelay Rust Gateway

FROM rust:1.98-alpine AS builder
RUN apk add --no-cache musl-dev llvm clang lld
WORKDIR /app
COPY . .
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN cp target/x86_64-unknown-linux-musl/release/aether-relay /aether-relay-binary

FROM scratch
LABEL maintainer="Schnee <schnee@users.noreply.github.com>" \
      description="AetherRelay Webhook Gateway v0.2.0"
COPY --from=builder /aether-relay-binary /aether-relay
COPY config/default.toml /config/default.toml
EXPOSE 3000
ENTRYPOINT ["/aether-relay"]
