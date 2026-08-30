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
- **Dynamic Real-Time Routing**: Update backend target routes instantaneously via Admin REST API.
- **Header Sanitization & Origin Protection**:
  - Automatically strips hop-by-hop headers (`connection`, `keep-alive`, `transfer-encoding`, etc.).
  - Removes identifiable origin headers (`Server`, `Via`, `X-Powered-By`).
  - Rewrites upstream redirects back to the proxy hostname.
- **Interactive OpenAPI 3.0 & Swagger UI**: Built-in interactive documentation at `/swagger-ui/`.
- **Production-Hardened Container**: Built from `scratch` (0 byte OS attack surface, non-root user `nobody:nobody`, no shell/terminal, read-only rootfs).

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
REDIS_PASSWORD=proxysecret
```

### 3. Run with Docker Compose (Recommended)

```bash
docker compose up -d --build
```

The service will start on `http://localhost:7654` with:
- **API / Proxy Service**: `http://localhost:7654`
- **Swagger Documentation**: `http://localhost:7654/swagger-ui/`
- **Health Check**: `http://localhost:7654/health`

---

## 📡 API Reference & Admin CRUD

All administrative endpoints require the `x-api-key` header matching your configured `ADMIN_TOKEN`.

### Endpoints Overview

| Method | Endpoint | Description | Auth Required |
| :--- | :--- | :--- | :---: |
| `GET` | `/health` | Service health status | No |
| `GET` | `/swagger-ui/` | Interactive Swagger API docs | No |
| `POST` | `/admin/routes` | Create or update route mapping | **Yes** |
| `GET` | `/admin/routes` | List all dynamic route mappings | **Yes** |
| `GET` | `/admin/routes/{*key}` | Fetch details of a specific route | **Yes** |
| `DELETE` | `/admin/routes/{*key}` | Delete a route mapping | **Yes** |
| `ANY` | `/{*key}` | Public proxy handler | No |

---

### 1. Create or Update Route

`POST /admin/routes`

**Headers:**
```http
Content-Type: application/json
x-api-key: <ADMIN_TOKEN>
```

**Request Body:**
```json
{
  "key": "https://yeapin.xyz",
  "value": "https://automation.bdmade.dev"
}
```
*(Note: Both `"value"` and `"target"` are supported interchangeably in the payload).*

**Response (`200 OK`):**
```json
{
  "success": true,
  "message": "route 'https://yeapin.xyz' saved"
}
```

---

### 2. List All Routes

`GET /admin/routes`

**Headers:**
```http
x-api-key: <ADMIN_TOKEN>
```

**Response (`200 OK`):**
```json
{
  "data": [
    {
      "key": "https://yeapin.xyz",
      "value": "https://automation.bdmade.dev"
    },
    {
      "key": "webhook-test",
      "value": "https://api.internal.network/v1/hook"
    }
  ],
  "message": "Successfully data fetched",
  "status": 200
}
```

---

### 3. Get Route Details

`GET /admin/routes/{*key}`

Supports both plain keys, URL paths, and percent-encoded keys (e.g. `/admin/routes/https://yeapin.xyz` or `/admin/routes/webhook-test`).

**Headers:**
```http
x-api-key: <ADMIN_TOKEN>
```

**Response (`200 OK`):**
```json
{
  "data": {
    "key": "https://yeapin.xyz",
    "value": "https://automation.bdmade.dev"
  },
  "message": "Successfully data fetched",
  "status": 200
}
```

---

### 4. Delete Route

`DELETE /admin/routes/{*key}`

**Headers:**
```http
x-api-key: <ADMIN_TOKEN>
```

**Response (`200 OK`):**
```json
{
  "success": true,
  "message": "route 'https://yeapin.xyz' deleted"
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

## 📄 License

This project is licensed under the [MIT License](LICENSE).
