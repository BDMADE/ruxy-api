# syntax=docker/dockerfile:1.7
# ------------------------------------------------------------------------------
# 1. Builder Stage
# ------------------------------------------------------------------------------
FROM rust:alpine AS builder
WORKDIR /app

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig ca-certificates curl

# Install wasm32 target and trunk for frontend build
RUN rustup target add wasm32-unknown-unknown && \
    curl -sL https://github.com/trunk-rs/trunk/releases/download/v0.21.4/trunk-x86_64-unknown-linux-musl.tar.gz | tar -xzf- -C /usr/local/bin

# Leverage BuildKit cache mounts for Cargo dependencies and Git index caching
COPY Cargo.toml ./
COPY server/Cargo.toml server/Cargo.lock* ./server/
COPY client/Cargo.toml client/Cargo.lock* ./client/

# Create dummy source and build dependencies layer with cache mount
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    mkdir -p server/src client/src && \
    echo "fn main() {}" > server/src/main.rs && \
    echo "fn main() {}" > client/src/main.rs && \
    cargo build --release --package proxy-api --target=$(rustc -vV | sed -n 's|host: ||p') && \
    cargo build --release --package ruxy-admin --target=wasm32-unknown-unknown && \
    rm -rf server/src client/src

# Copy real source code
COPY server/src ./server/src
COPY client/src ./client/src
COPY client/index.html ./client/
COPY client/style ./client/style/
COPY client/Trunk.toml ./client/

# Build backend
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    touch server/src/main.rs && \
    TARGET=$(rustc -vV | sed -n 's|host: ||p') && \
    cargo build --release --package proxy-api --target=$TARGET && \
    cp /app/target/$TARGET/release/proxy-api /proxy-api

# Build frontend
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    touch client/src/main.rs && \
    cd client && trunk build --release && \
    cp -r dist /app/dist

# ------------------------------------------------------------------------------
# 2. Production Minimal Runtime Stage - Backend
# ------------------------------------------------------------------------------
FROM scratch AS backend

# Copy SSL root certificates for outbound HTTPS requests (e.g., reqwest / rustls)
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the compiled statically linked binary
COPY --from=builder /proxy-api /proxy-api
COPY --from=builder /app/dist /dist

# Set default production environment variables
ENV PORT=7654 \
    RUST_LOG=info \
    PUBLIC_DIR=/dist

# Document application port
EXPOSE 7654

# Run as non-root user (nobody:nobody)
USER 65534:65534

# Execute the application
ENTRYPOINT ["/proxy-api"]
