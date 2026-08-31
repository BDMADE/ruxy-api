# 📋 Ruxy Project Overview & Feature Breakdown

**Ruxy** is an ultra-hardened, high-performance HTTP reverse proxy and API routing service written in Rust ([Axum](https://github.com/tokio-rs/axum) + [Tokio](https://tokio.rs/)), backed by [Redis](https://redis.io/) / [Dragonfly](https://www.dragonflydb.io/) for dynamic route storage, and integrated with Slack Webhooks for real-time error alerting.

---

## 🎯 Primary Purpose

1. **Origin Obfuscation**: Hides downstream backend IP addresses, internal hostnames, and technologies from public clients.
2. **Dynamic Zero-Downtime Routing**: Re-route public incoming paths to different target backends on the fly without service restarts or container redeployments.
3. **Sub-millisecond Forwarding**: Zero-cost async streaming pipeline built on Tokio, Axum, and Hyper.
4. **Resilient Production Observability**: Real-time asynchronous alerts to Slack channels on network/upstream failures.

---

## 🧩 Architectural Breakdown

```
[ Public Client ]
       │
       ▼ (HTTP / HTTPS)
[ Ruxy Proxy Engine (Axum / Tokio) ]
       │
       ├──► 1. State / Redis Lookup (`route:<key>`)
       │
       ├──► 2. Sanitization (Strip Hop-by-Hop & Fingerprint Headers)
       │
       ├──► 3. Forward Request / Stream Response ──► [ Origin / Backend Service ]
       │
       └──► 4. (On Error) Async Alerting ─────────► [ Slack Webhook ]
```

---

## 🚀 Feature Breakdown

### 1. Dynamic Reverse Proxy Engine (`src/proxy.rs`)
- **Wildcard Path & Subpath Resolution**: Resolves `/:key` into backend target URL, and appends any remaining path/query string (e.g. `/webhook/test?v=1` -> `https://origin.example.com/test?v=1`).
- **Header Sanitization & Security**:
  - Automatically strips standard HTTP hop-by-hop headers (`connection`, `keep-alive`, `transfer-encoding`, `upgrade`, etc.).
  - Strips identifiable origin headers (`Server`, `Via`, `X-Powered-By`).
  - Strips internal auth header (`x-api-key`) before forwarding to the backend.
- **Upstream Redirect Rewriting**: Rewrites `Location` redirect headers from upstream targets back to the proxy’s public hostname so origin URLs are never leaked to the client.
- **Asynchronous Body Streaming**: Large payloads stream directly to and from upstream backends without buffering excessive memory.

### 2. Admin Management REST API (`src/admin.rs`)
Protected by API Key authentication via `x-api-key` header matching `ADMIN_TOKEN`.

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `/admin/routes` | `POST` | Create or update a dynamic route mapping (`key` -> `value` / `target`). |
| `/admin/routes` | `GET` | Paginated/scanned list of all active route mappings. |
| `/admin/routes/{*key}` | `GET` | Inspect details of a specific route key. |
| `/admin/routes/{*key}` | `DELETE` | Delete a route mapping instantaneously. |

### 3. Redis / Dragonfly Data Layer (`src/state.rs`)
- Uses `redis::aio::ConnectionManager` for auto-reconnecting, multiplexed, asynchronous command pipelining.
- Non-blocking cursor scanning (`SCAN route:*`) for listing routes safely without blocking Redis in high-scale environments.

### 4. Real-time Slack Webhook Alerting (`src/state.rs`, `src/proxy.rs`, `src/admin.rs`)
- **Non-blocking Tokio tasks**: Delivers alerts in the background (`tokio::spawn`) with zero latency impact on active HTTP proxy requests.
- **Structured Slack Block Kit format**: Includes alert title, timestamp, route key, target URL, HTTP method, and error traces.
- **Trigger Points**:
  - `502 Bad Gateway` / Upstream unreachable errors during proxy forwarding.
  - Redis connection or write errors during route updates.

### 5. Interactive OpenAPI 3.0 & Swagger UI (`src/main.rs`)
- Built-in interactive Swagger UI available at `/swagger-ui/`.
- Raw OpenAPI 3.0 JSON specification served at `/api-docs/openapi.json`.
- Integrated `api_key` authorization modal for live testing.

### 6. Leptos Web Admin Dashboard (`client/`)
- **CSR WebAssembly SPA**: Built with [Leptos 0.6](https://leptos.dev/) and `leptos_router`.
- **Authentication**: Password-based login backed by `/admin/login`, storing token in browser `LocalStorage` with auto-logout on `401 Unauthorized`.
- **Route Management**:
  - Searchable and paginated route table (25, 50, 100 items per page).
  - Create and edit route forms with input validation.
  - Delete action with confirmation prompt and live table refresh.
- **Modern Dark UI**: Custom CSS with glassmorphism, responsive cards, and clean typography.

### 7. Container Hardening & Minimal Footprint (`Dockerfile`, `docker-compose.yml`)
- **`scratch` Base Image**: Statically linked Musl binary, zero OS attack surface.
- **Non-root Execution**: Runs as UID `65534:65534` (`nobody:nobody`).
- **Read-Only Root Filesystem**: `read_only: true` with all capabilities dropped (`cap_drop: [ALL]`) and `no-new-privileges: true`.
- **Dragonfly / Redis Integration**: Production resource limits and health checks configured in Compose.

---

## 📁 Source Code Structure

```
├── .env.example              # Template for environment configuration
├── .github/workflows/ci.yml  # GitHub Actions CI pipeline
├── Dockerfile                # Multi-stage hardened scratch build
├── docker-compose.yml        # Orchestration for Ruxy API & Dragonfly/Redis
├── LICENSE                   # MIT License
├── README.md                 # Public documentation and scalability guide
├── PROJECT.md                # Project architecture, implementation audit & review
├── client/                   # Leptos WebAssembly Client Dashboard
│   ├── Cargo.toml            # Client crate dependencies (Leptos, gloo, wasm-bindgen)
│   ├── index.html            # WebAssembly HTML entrypoint
│   ├── Trunk.toml            # Trunk WASM bundler & dev proxy configuration
│   ├── style/                # Global styling & design system
│   └── src/
│       ├── main.rs           # WebAssembly entrypoint & panic hook
│       ├── app.rs            # Router & component layout tree
│       ├── auth.rs           # Authentication context & LocalStorage token state
│       ├── api.rs            # Typed async HTTP client (Gloo-net)
│       ├── models.rs         # Client DTOs & response schemas
│       └── components/       # UI Components (LoginPage, AdminLayout, RouteList, RouteForm)
└── server/                   # Axum + Tokio Reverse Proxy & Admin Backend
    ├── Cargo.toml            # Server crate dependencies (Axum, Redis, Tokio, Utoipa)
    └── src/
        ├── main.rs           # Server startup, routing, middleware & Swagger setup
        ├── state.rs          # AppState, Redis commands & Slack alerting helper
        ├── proxy.rs          # Reverse proxy handler, header stripping & streaming
        ├── admin.rs          # Admin CRUD & authentication endpoints
        └── integration_tests.rs # Automated integration test suite
```

---

## 📋 Comprehensive Implementation Review & Missing Items

A complete review of the repository across `server/`, `client/`, CI/CD, Docker configurations, and architecture reveals the following implemented features, in-progress items, and missing implementations:

---

### 🔴 Critical Missing Implementations (Blocking Builds / Operations)

#### 1. `server/src/main.rs`: Missing `admin_password` in `AppState` Initialization
- **Issue**: `admin_password` was added to `AppState` in `server/src/state.rs` for the web admin login, but `server/src/main.rs` does not initialize it from `std::env::var("ADMIN_PASSWORD")`.
- **Impact**: `server` fails compilation with `error[E0063]: missing field admin_password in initializer of AppState`.
- **Resolution**: Read `ADMIN_PASSWORD` in `server/src/main.rs` (defaulting to `ADMIN_TOKEN` if not set) and populate `app_state.admin_password`.

#### 2. `server/src/main.rs`: `/admin/login` Route Not Mounted in Axum Router
- **Issue**: The `admin::login` handler is implemented in `server/src/admin.rs`, and the Leptos frontend calls `POST /admin/login` in `client/src/auth.rs`, but the route is not registered in `app_router()` in `server/src/main.rs`.
- **Impact**: Frontend web login attempts return `404 Not Found`.
- **Resolution**: Mount `POST /admin/login` on the public router outside the `auth_middleware` (unauthenticated endpoint).

#### 3. Root Cargo Workspace Configuration Missing (`Cargo.toml`)
- **Issue**: The project was restructured into `server/` and `client/` subdirectories, but there is no root `Cargo.toml` defining a Cargo workspace (`[workspace] members = ["server", "client"]`).
- **Impact**: Running `cargo fmt`, `cargo clippy`, or `cargo test` from the repository root (as configured in `.github/workflows/ci.yml`) fails because no manifest exists at the root.
- **Resolution**: Create a root `Cargo.toml` with `[workspace] members = ["server", "client"]` and shared workspace settings.

#### 4. `Dockerfile` Multi-Stage Build Out of Sync with Repository Structure
- **Issue**: The root `Dockerfile` attempts to `COPY Cargo.toml` and `COPY src ./src` at the root path, which no longer exist.
- **Impact**: `docker build` and `docker compose build` fail immediately.
- **Resolution**: Update `Dockerfile` to build from `server/` (and optionally build/embed `client/` static assets into the runtime image or server bundle).

---

### 🟡 High Priority Missing Implementations (Functional & Integration Gaps)

#### 5. `server/src/main.rs`: Static Asset Serving for Leptos Admin Client
- **Issue**: The Axum server currently only serves API endpoints and falls back directly to `proxy::proxy_handler`. There is no route or static file handler to serve the compiled Leptos client WebAssembly / HTML bundle (e.g. at `/admin/ui` or `/`).
- **Impact**: The web dashboard cannot be accessed directly from the deployed proxy server without running a separate web server.
- **Resolution**: Add static file serving (using `tower_http::services::ServeDir` or `rust-embed`) to serve the client dashboard at `/admin/ui` or dedicated path.

#### 6. CORS Configuration for Local Development (`tower_http::cors`)
- **Issue**: When developing locally with Trunk (`http://localhost:3000`) and the API server (`http://localhost:7654`), direct API calls or swagger requests across origins need CORS headers.
- **Impact**: Direct browser requests from foreign origins or dev servers are blocked if not proxied via Trunk.
- **Resolution**: Add configurable `CorsLayer` middleware in Axum for development and admin endpoints.

#### 7. `docker-compose.yml`: Missing `ADMIN_PASSWORD` Environment Variable
- **Issue**: `.env.example` includes `ADMIN_PASSWORD`, but `docker-compose.yml` does not pass `ADMIN_PASSWORD` into the `api` container environment.
- **Impact**: Containerized backend cannot authenticate frontend login requests if `ADMIN_PASSWORD` is expected.
- **Resolution**: Add `- ADMIN_PASSWORD=${ADMIN_PASSWORD:-${ADMIN_TOKEN}}` to `docker-compose.yml`.

#### 8. OpenAPI Documentation (`ApiDoc`) Missing `/admin/login` Schema
- **Issue**: `admin::login`, `LoginRequest`, and `LoginResponse` are not registered in the `#[derive(OpenApi)]` annotations in `server/src/main.rs`.
- **Impact**: The login endpoint is missing from Swagger UI documentation at `/swagger-ui/`.
- **Resolution**: Add `#[utoipa::path]` to `login()` and register in `ApiDoc` paths and components.

#### 9. `server/src/integration_tests.rs`: Missing `admin_password` in Test `AppState` Init
- **Issue**: The `setup_state()` function in `integration_tests.rs` constructs `AppState` without the `admin_password` field that was added to the struct in `state.rs`.
- **Impact**: `cargo test` fails with `error[E0063]: missing field admin_password in initializer of AppState`. All 15 integration tests are blocked.
- **Resolution**: Add `admin_password: "test-secret".to_string(),` to the `AppState` init block in `setup_state()`.

#### 10. `client/nginx.conf`: Wrong Proxy Port (7653 vs 7654)
- **Issue**: The Nginx reverse proxy for the `/admin/` location proxies to `http://server:7653/admin/`, but the Axum server container listens on port `7654` (as set by `PORT=7654` in `docker-compose.yml`).
- **Impact**: All API calls from the Leptos client dashboard (`/admin/login`, `/admin/routes`) fail with `502 Bad Gateway` in Docker Compose deployment.
- **Resolution**: Change `proxy_pass http://server:7653/admin/;` to `proxy_pass http://server:7654/admin/;` in `client/nginx.conf`.

#### 11. `.github/workflows/ci.yml`: Docker Build Missing `target: backend`
- **Issue**: The `build-docker` job in CI uses `docker/build-push-action` without specifying `target: backend`. Since the Dockerfile now has multiple targets (`backend` and `frontend`), Docker builds the **last stage** (`frontend`) by default.
- **Impact**: The pushed Docker image `hmtanbir/ruxy:latest` contains the Nginx frontend instead of the Axum backend binary.
- **Resolution**: Add `target: backend` to the `docker/build-push-action` `with:` block in `ci.yml`.

---

### 🟠 Corner Cases & Security Hardening (Open from Audit)

#### 9. URL-Encoded & Unicode Route Key Sanitization (C1)
- **Status**: ⚠️ Open
- **Detail**: Requests containing percent-encoded keys (e.g. `/%2Fadmin`) or non-ASCII characters need uniform decoding, validation, and normalization in `server/src/proxy.rs` and `server/src/admin.rs`.

#### 10. Upstream Response Header Injection Protection (C2)
- **Status**: ⚠️ Open
- **Detail**: Responses from upstream backends are forwarded without sanitizing CRLF injection or duplicate malicious headers. Add strict response header sanitization.

#### 11. Minimal Scratch Container Health Check (C5)
- **Status**: ⚠️ Open (Deferred due to `scratch` container limitations)
- **Detail**: The `api` container runs on `scratch` with no shell or curl. Docker Compose currently cannot healthcheck the API container.
- **Resolution**: Implement a CLI healthcheck mode in the Ruxy binary (e.g. `ruxy --health-check`) so Docker can use `CMD ["/proxy-api", "--health-check"]`.

---

### 🔮 Scalability & Performance Roadmap (Future Enhancements)

- [ ] **L1 In-Memory Cache (Moka / DashMap)**: Short-lived local caching for route keys to reduce Redis roundtrips to sub-microsecond latency.
- [ ] **Redis Pub/Sub Cache Invalidation**: Real-time cache invalidation across distributed proxy nodes on route updates.
- [ ] **Circuit Breaker / Upstream Health Checks**: Proactive upstream probing with automatic failover to backup target URLs.
- [ ] **Prometheus / OpenTelemetry Metrics**: Exporting `/metrics` for Prometheus scraping (p99 latency, 5xx error rates, QPS).
- [ ] **Distributed Rate Limiting**: Token-bucket rate limiting per API key or IP address via Redis.

---

## 🐛 Bugs, Edge Cases & Corner Cases Audit

> Full code review performed across `proxy.rs`, `admin.rs`, `state.rs`, `main.rs`, `client/`, `Dockerfile`, `docker-compose.yml`, and `ci.yml`.

---

### 🔴 Bugs (Functional Issues)

#### B1. Body Buffering Defeats Streaming — Memory Spike Risk
**File:** [`server/src/proxy.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/proxy.rs)
- The request body is streamed directly to `reqwest` using `reqwest::Body::wrap_stream()`.
- **Status:** ✅ Fixed / Closed

#### B2. Shallow Health Check — `/health` Returns `"OK"` Even When Redis Is Down
**File:** [`server/src/main.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/main.rs)
- Pings Redis inside the health handler; returns `503 Service Unavailable` on failure.
- **Status:** ✅ Fixed / Closed

#### B3. `get_target` Silently Swallows Redis Errors
**File:** [`server/src/state.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/state.rs)
- Propagates Redis errors and returns `503 Service Unavailable` on connectivity failure.
- **Status:** ✅ Fixed / Closed

#### B4. `list_routes` Silently Breaks on Redis Error — Returns Partial Results
**File:** [`server/src/state.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/state.rs)
- Returns `Result<Vec<RouteEntry>, redis::RedisError>` and surfaces errors to the admin endpoint.
- **Status:** ✅ Fixed / Closed

#### B5. Timing-Insecure Token Comparison
**File:** [`server/src/main.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/main.rs)
- Uses constant-time equality check (`subtle::ConstantTimeEq`).
- **Status:** ✅ Fixed / Closed

---

### 🟡 Edge Cases

#### E1. Route Key Collision with Reserved Paths
**File:** [`server/src/proxy.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/proxy.rs)
- Checks exact segment matches (`raw_path == "admin" || raw_path.starts_with("admin/")`) instead of naive prefix matching.
- **Status:** ✅ Fixed / Closed

#### E2. Request Body >10 MiB Is Silently Rejected
**File:** [`server/src/proxy.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/proxy.rs)
- Mitigated by full end-to-end streaming without arbitrary buffering limits.
- **Status:** ✅ Fixed / Closed

#### E3. `value` Field Validation for URL Well-Formedness
**File:** [`server/src/admin.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/admin.rs)
- Parsed with `url::Url::parse()` validating scheme and host.
- **Status:** ✅ Fixed / Closed

#### E4. Redirect Rewriting Trailing Slash Mismatch
**File:** [`server/src/proxy.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/proxy.rs)
- Targets normalized with trailing slash stripped for consistent redirect rewriting.
- **Status:** ✅ Fixed / Closed

#### E5. `delete_route` Status Codes
**File:** [`server/src/admin.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/admin.rs)
- Returns `404 Not Found` when deleting non-existent routes.
- **Status:** ✅ Fixed / Closed

#### E6. Request Timeout on Upstream Forwarding
**File:** [`server/src/main.rs`](file:///Users/hmtanbir/www/bdmade/ruxy/server/src/main.rs)
- Configured with `connect_timeout(5s)` and `timeout(30s)`.
- **Status:** ✅ Fixed / Closed

---

### 📊 Summary Status Matrix

| ID | Category | Component / File | Description | Status |
| :--- | :--- | :--- | :--- | :--- |
| **B1** | Memory | `server/src/proxy.rs` | Direct async body streaming | ✅ Closed |
| **B2** | Health | `server/src/main.rs` | Deep Redis healthcheck in `/health` | ✅ Closed |
| **B3** | Reliability | `server/src/state.rs` | Explicit Redis error propagation in `get_target` | ✅ Closed |
| **B4** | Data Integrity | `server/src/state.rs` | Explicit Redis error propagation in `list_routes` | ✅ Closed |
| **B5** | Security | `server/src/main.rs` | Constant-time token comparison (`subtle`) | ✅ Closed |
| **E1** | Routing | `server/src/proxy.rs` | Reserved path segment matching | ✅ Closed |
| **E2** | UX | `server/src/proxy.rs` | Streaming without arbitrary 10MB cutoff | ✅ Closed |
| **E3** | Validation | `server/src/admin.rs` | Strict URL validation in route creation | ✅ Closed |
| **E4** | Security | `server/src/proxy.rs` | Normalized target URL in Location rewrite | ✅ Closed |
| **E5** | API Design | `server/src/admin.rs` | 404 response on deleting non-existent route | ✅ Closed |
| **E6** | Reliability | `server/src/main.rs` | Upstream connect & request timeouts | ✅ Closed |
| **C3** | Alerting | `server/src/state.rs` | Slack alert debouncing & rate limiting | ✅ Closed |
| **C4** | CI/CD | `.github/workflows/ci.yml` | Integration test execution in CI | ✅ Closed |
| **C6** | Reliability | `server/src/admin.rs` | Redis error handling in `delete_route` | ✅ Closed |
| **M1** | Backend Bug | `server/src/main.rs` | Missing `admin_password` in `AppState` init | ✅ Closed |
| **M2** | Backend Route | `server/src/main.rs` | Missing `/admin/login` endpoint in router | ✅ Closed |
| **M3** | Workspace | `Cargo.toml` (Root) | Root Cargo workspace manifest missing | ✅ Closed |
| **M4** | DevOps | `Dockerfile` | Multi-stage build updated for workspace | ✅ Closed |
| **M5** | DevOps | `docker-compose.yml` | `ADMIN_PASSWORD` passed to container | ✅ Closed |
| **M6** | Integration | `server/src/main.rs` | Client WASM static asset serving / SPA fallback | ✅ Closed (served directly via Axum backend) |
| **M7** | Integration | `server/src/main.rs` | CORS middleware for local standalone dev | ✅ Closed |
| **M8** | OpenAPI | `server/src/main.rs` | Missing `/admin/login` schema in Swagger docs | ✅ Closed |
| **M9** | 🔴 Test Bug | `server/src/integration_tests.rs` | Missing `admin_password` field in test `AppState` init — blocks `cargo test` | ✅ Closed |
| **M10** | 🔴 DevOps Bug | `client/nginx.conf` | Nginx proxies to port `7653` but server listens on `7654` — client API calls fail in Docker | ✅ Closed |
| **M11** | CI/CD | `.github/workflows/ci.yml` | `build-docker` job missing `target: backend` for multi-stage Dockerfile (builds default target, not `backend`) | ✅ Closed |
| **C1** | Hardening | `server/src/proxy.rs` | URL-encoded & unicode route key sanitization | ⚠️ Open |
| **C2** | Hardening | `server/src/proxy.rs` | Upstream response header validation | ⚠️ Open |
| **C5** | Ops | `Dockerfile` / `docker-compose.yml` | CLI health check mode for `scratch` image | ⚠️ Open |
| **R1** | Performance | `server/src/proxy.rs` | L1 in-memory cache (Moka / DashMap) | 🔮 Roadmap |
| **R2** | Scalability | `server/src/state.rs` | Redis Pub/Sub cache invalidation | 🔮 Roadmap |
| **R3** | Reliability | `server/src/proxy.rs` | Circuit breaker & active upstream probing | 🔮 Roadmap |
| **R4** | Observability| `server/src/main.rs` | Prometheus `/metrics` endpoint | 🔮 Roadmap |
| **R5** | Traffic | `server/src/main.rs` | Distributed rate limiting (token bucket) | 🔮 Roadmap |



