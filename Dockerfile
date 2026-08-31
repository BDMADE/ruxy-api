# syntax=docker/dockerfile:1.7
# ------------------------------------------------------------------------------
# 1. Builder Stage
# ------------------------------------------------------------------------------
FROM rust:alpine AS builder
WORKDIR /app

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig ca-certificates curl

# Leverage BuildKit cache mounts for Cargo dependencies and Git index caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source and build dependencies layer with cache mount
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --target=$(rustc -vV | sed -n 's|host: ||p') && \
    rm -rf src

# Copy real source code and perform the final static build
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    touch src/main.rs && \
    TARGET=$(rustc -vV | sed -n 's|host: ||p') && \
    cargo build --release --target=$TARGET && \
    cp /app/target/$TARGET/release/proxy-api /proxy-api

# ------------------------------------------------------------------------------
# 2. Production Minimal Runtime Stage
# ------------------------------------------------------------------------------
FROM scratch AS runtime

# Copy SSL root certificates for outbound HTTPS requests (e.g., reqwest / rustls)
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the compiled statically linked binary
COPY --from=builder /proxy-api /proxy-api

# Set default production environment variables
ENV PORT=7654 \
    RUST_LOG=info

# Document application port
EXPOSE 7654

# Run as non-root user (nobody:nobody)
USER 65534:65534

# Execute the application
ENTRYPOINT ["/proxy-api"]
