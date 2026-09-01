<div align="center">

# ⚡ Ruxy (Proxy API Service)

**High-performance, ultra-hardened dynamic reverse proxy and API routing service written in Rust and backed by Redis / Dragonfly.**

[![CI](https://github.com/BDMADE/Ruxy/actions/workflows/ci.yml/badge.svg)](https://github.com/BDMADE/Ruxy/actions/workflows/ci.yml)
[![Docker Image](https://img.shields.io/badge/docker-hmtanbir%2Fruxy--api-blue?logo=docker)](https://hub.docker.com/r/hmtanbir/ruxy-api)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

</div>

---

## 📖 Overview

**Ruxy** is a lightweight, cloud-native HTTP reverse proxy designed for dynamic URL routing, origin obfuscation, and seamless traffic redirection without service restarts.

When public traffic hits `/:key` (e.g., `https://proxy.example.com/automation-webhook`), Ruxy resolves `route:<key>` in Redis and streams the request directly to the upstream destination while keeping backend infrastructure completely hidden.

### ✨ Key Features

- **Blazing Fast & Low Footprint**: Built with [Axum](https://github.com/tokio-rs/axum), [Tokio](https://tokio.rs/), and `ConnectionManager` multiplexing over Redis/Dragonfly.
- **Dynamic Real-Time Routing**: Update backend target routes instantaneously via Admin REST API or the Web Dashboard.
- **Leptos Web Admin Dashboard**: Beautiful, responsive WebAssembly single-page application for visual route management.
- **Header Sanitization & Origin Protection**:
  - Automatically strips hop-by-hop headers (`connection`, `keep-alive`, `transfer-encoding`, etc.).
  - Removes identifiable origin headers (`Server`, `Via`, `X-Powered-By`).
  - Rewrites upstream redirects back to the proxy hostname.
- **Interactive OpenAPI 3.0 & Swagger UI**: Built-in interactive documentation at `/swagger-ui/`.
- **Production-Hardened Container**: Built from `scratch` (0 byte OS attack surface, non-root user `nobody:nobody`, no shell/terminal, read-only rootfs).

---

## 🖥️ Leptos Web Admin Dashboard (`client/`)

Ruxy includes a modern, high-performance WebAssembly Single Page Application (SPA) built with [Leptos 0.6](https://leptos.dev/) and `leptos_router` for visual route management.

### 🌟 Dashboard Features
- **Password Authentication**: Sign in using your `ADMIN_PASSWORD` (or `ADMIN_TOKEN`). Tokens are securely stored in browser `LocalStorage` with automatic session invalidation on `401 Unauthorized`.
- **Route Explorer**:
  - Real-time search filter by route key or target URL.
  - Configurable pagination (25, 50, 100 items per page).
- **Route CRUD Operations**:
  - Add new dynamic route mappings with live URL validation.
  - Edit existing destination target URLs.
  - Instant route deletion with confirmation prompts.
- **Modern Dark UI**: Designed with glassmorphism, responsive cards, and clean typography.

### 🚀 Accessing the Web Dashboard

The Ruxy client is now bundled directly into the Axum backend! When you run the Docker container, the WebAssembly application is served automatically.

1. Start the application via Docker Compose (see Quick Start below).
2. Open `http://localhost:7654/admin/dashboard/` in your browser.
3. Enter your configured `ADMIN_PASSWORD` (or `ADMIN_TOKEN`).
4. Click **Sign In** to access and manage your routes.

*(Note: If you are doing local frontend development, you can still run `trunk serve` inside the `client/` directory for hot-reloading.)*

---

## 🏗️ Architecture Flow

```
[ Client ] 
    │
    ▼ (HTTP/HTTPS)
[ Ruxy Proxy (Axum / Tokio) ]
    │
    ├──► 1. Check Redis Cache / Route Store (`route:<key>`)
    │
    ├──► 2. Strip sensitive origin headers & hop-by-hop metadata
    │
    ▼ 3. Stream request & forward response
[ Hidden Upstream / Origin Service ]
```

---

## 🚀 Quick Start

### 1. Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/)
- *(Optional for local dev)* [Rust toolchain](https://rustup.rs/) (1.80+) and [Redis](https://redis.io/)

### 2. Environment Configuration

Copy the example environment file:

```bash
cp .env.example .env
```

Configure your `.env` variables:

```ini
RUST_LOG=info
PORT=7654
REDIS_URL=redis://:proxysecret@redis:6379
ADMIN_TOKEN=change-me-super-secret-token
ADMIN_PASSWORD=change-me-super-secret-password
REDIS_PASSWORD=proxysecret
SLACK_WEBHOOK_URL=https://hooks.slack.com/services/TXXXXX/BXXXXX/XXXXXXXXXXXX
```

### 3. Run with Docker Compose (Recommended)

```bash
docker compose up -d --build
```

The service will start on `http://localhost:7654` with:
- **Web Admin Dashboard**: `http://localhost:7654/admin/dashboard/`
- **API / Proxy Service**: `http://localhost:7654`
- **Swagger Documentation**: `http://localhost:7654/swagger-ui/`
- **Health Check**: `http://localhost:7654/health`

---

## 📚 Swagger & OpenAPI Documentation

Ruxy provides an interactive **Swagger UI** out of the box for testing and exploring API endpoints:

- **Swagger UI Interface**: `http://localhost:7654/swagger-ui/`
- **Raw OpenAPI 3.0 JSON Spec**: `http://localhost:7654/api-docs/openapi.json`

### 🔑 Using Swagger with Authentication:
1. Open `http://localhost:7654/swagger-ui/` in your browser.
2. Click the **Authorize** 🔓 button at the top right.
3. In the `api_key (apiKey)` modal, enter your configured `ADMIN_TOKEN` (e.g. `change-me-super-secret-token`).
4. Click **Authorize** and then **Close**.
5. You can now execute and test all admin endpoints directly through Swagger UI.

---

## 📡 API Reference & Admin CRUD

Administrative route endpoints require the `x-api-key` header matching your configured `ADMIN_TOKEN`. The login endpoint authenticates using your `ADMIN_PASSWORD`.

### Endpoints Overview

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :---: |
| `GET` | `/health` | Service health status | No |
| `POST` | `/admin/login` | Dashboard password login | No |
| `GET` | `/swagger-ui/` | Interactive Swagger API docs | No |
| `GET` | `/api-docs/openapi.json` | Raw OpenAPI 3.0 specification | No |
| `POST` | `/admin/api/routes` | Create or update route mapping | **Yes** (`x-api-key`) |
| `GET` | `/admin/api/routes` | List all dynamic route mappings | **Yes** (`x-api-key`) |
| `GET` | `/admin/api/routes/{*key}` | Fetch details of a specific route | **Yes** (`x-api-key`) |
| `DELETE` | `/admin/api/routes/{*key}` | Delete a route mapping | **Yes** (`x-api-key`) |
| `ANY` | `/{*key}` | Public proxy handler | No |

---

### 1. Create or Update Route

`POST /admin/api/routes`

> **Key Format Rules:**
> - `key` must be a path identifier or slug (e.g. `webhook`, `payment-api`, `auth`).
> - `key` **cannot** contain protocols like `://` (e.g. `https://` is rejected with `400 Bad Request`).
> - `value` (or alias `target`) must be a valid destination URL starting with `http://` or `https://`.

**Headers:**
```http
Content-Type: application/json
x-api-key: <ADMIN_TOKEN>
```

**Request Body:**
```json
{
  "key": "webhook",
  "value": "https://automation.bdmade.dev/webhook-test"
}
```
*(Note: Both `"value"` and `"target"` are supported interchangeably in the payload).*

**Response (`200 OK`):**
```json
{
  "success": true,
  "message": "route 'webhook' saved"
}
```

---

### 2. List All Routes

`GET /admin/api/routes`

**Headers:**
```http
x-api-key: <ADMIN_TOKEN>
```

**Response (`200 OK`):**
```json
{
  "data": [
    {
      "key": "payment-api",
      "value": "https://api.payment.onboarding.bdmade.dev"
    },
    {
      "key": "webhook-test",
      "value": "https://yepin.app.n8n.cloud/webhook-test"
    }
  ],
  "message": "Successfully data fetched",
  "status": 200
}
```

---

### 3. Get Route Details

`GET /admin/api/routes/{*key}`

Fetch a specific route mapping by its key (e.g. `/admin/api/routes/webhook-test`).

**Headers:**
```http
x-api-key: <ADMIN_TOKEN>
```

**Response (`200 OK`):**
```json
{
  "data": {
    "key": "webhook-test",
    "value": "https://yepin.app.n8n.cloud/webhook-test"
  },
  "message": "Successfully data fetched",
  "status": 200
}
```

---

### 4. Delete Route

`DELETE /admin/api/routes/{*key}`

Delete a route mapping by its key (e.g. `/admin/api/routes/webhook-test`).

**Headers:**
```http
x-api-key: <ADMIN_TOKEN>
```

**Response (`200 OK`):**
```json
{
  "success": true,
  "message": "route 'webhook-test' deleted"
}
```

---

## 🛡️ Production & Security Best Practices

### Container Hardening
- **`scratch` Base Image**: Built using a multi-stage Dockerfile where the final image contains *only* the statically-linked binary and CA certificates.
- **No Shell / Terminal Access**: Attackers cannot execute arbitrary commands (`sh`, `bash`, `wget`, `curl` do not exist).
- **Non-Root Execution**: Runs under UID `65534:65534` (`nobody:nobody`).
- **Read-Only Root Filesystem**: `read_only: true` enabled in Docker Compose with drop-all capabilities (`cap_drop: [ALL]`) and `no-new-privileges: true`.

---

## 🛠️ Development & Testing

### Running Tests Locally

Ensure Redis is running, then execute:

```bash
# Run code formatter check
cargo fmt -- --check

# Run linter
cargo clippy -- -D warnings

# Run test suite
cargo test
```

---

## ⚡ High-Throughput & Scalability Architecture

Ruxy is engineered from the ground up for extreme concurrency and low-latency throughput. Understanding how it handles massive request volumes and how to scale it horizontally across distributed clusters ensures seamless production operations under heavy load (e.g. 50k–500k+ requests/sec).

```
                            [ Cloudflare / AWS CloudFront / Edge CDN ]
                                                │
                                  [ Anycast / Layer 4 Load Balancer ]
                                          (AWS NLB / HAProxy)
                                                │
                    ┌───────────────────────────┼───────────────────────────┐
                    ▼                           ▼                           ▼
        [ Ruxy Replica #1 ]           [ Ruxy Replica #2 ]           [ Ruxy Replica #N ]
        (Tokio Event Loop)            (Tokio Event Loop)            (Tokio Event Loop)
        (In-Memory L1 Cache)          (In-Memory L1 Cache)          (In-Memory L1 Cache)
                    │                           │                           │
                    └───────────────────────────┼───────────────────────────┘
                                                │
                                   [ Redis / Dragonfly Cluster ]
                                     (Route Store & Rate Limits)
                                                │
                                 [ Upstream Origin Backends ]
```

### 1. Core Mechanics Under High Load

- **Asynchronous Non-Blocking I/O (Tokio Runtime)**: Each incoming request is scheduled onto lightweight Tokio green threads (tasks) across available CPU cores without spawning heavy OS threads.
- **Connection Pooling & HTTP Keep-Alive**: Upstream backend connections are managed by connection pools in `reqwest`, reusing existing TCP sockets and eliminating TLS/TCP handshake latency per request.
- **Multiplexed Redis Client**: Built with Redis `ConnectionManager` to enable pipelined and multiplexed asynchronous queries across threads without connection contention.
- **Zero-Cost Abstraction & Predictable Memory**: Minimal memory allocations per request ensure consistent sub-millisecond p99 latency without garbage collection pauses.

---

### 2. Horizontal Scaling & High Availability (HA)

Because Ruxy is **100% stateless** (all route data and shared states reside in Redis), scaling capacity is straightforward:

1. **Multi-Replica Deployment**: Run multiple container instances behind a Layer 4 (e.g., AWS NLB, HAProxy) or Layer 7 (e.g., Traefik, Envoy, NGINX) load balancer.
2. **Kubernetes Auto-Scaling**: Deploy with a `HorizontalPodAutoscaler` (HPA) targeting CPU utilization (e.g., 60–70%) or incoming HTTP connection count.
3. **Redis Clustering / Dragonfly**: For millions of route lookups and global rate limiting, pair with a sharded Redis Cluster, Redis Sentinel, or [Dragonfly](https://www.dragonflydb.io/) (multi-threaded in-memory store capable of millions of QPS).

---

### 3. Production OS & Kernel Tuning

For hosts or worker nodes handling 100k+ concurrent connections, tune the host TCP stack (`/etc/sysctl.conf`):

```ini
# Increase maximum open file descriptors
fs.file-max = 2097152

# Expand ephemeral port range for upstream proxying
net.ipv4.ip_local_port_range = 1024 65535

# Increase TCP connection backlog
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535

# Enable fast recycling and reuse of TIME_WAIT sockets
net.ipv4.tcp_tw_reuse = 1
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
