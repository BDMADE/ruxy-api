FROM rust:1.96-alpine AS builder
WORKDIR /build

RUN apk add --no-cache musl-dev pkgconfig ca-certificates

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true \
    CARGO_TERM_COLOR=never \
    RUSTFLAGS="-C target-feature=+crt-static"

COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src target/release/deps/proxy_api*

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM scratch AS runtime

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /build/target/release/proxy-api /proxy-api

ENV PORT=3000 \
    RUST_LOG=info

EXPOSE 3000

USER 65534:65534

ENTRYPOINT ["/proxy-api"]
