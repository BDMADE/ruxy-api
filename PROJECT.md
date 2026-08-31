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

### 6. Container Hardening & Minimal Footprint (`Dockerfile`, `docker-compose.yml`)
- **`scratch` Base Image**: Statically linked Musl binary, zero OS attack surface.
- **Non-root Execution**: Runs as UID `65534:65534` (`nobody:nobody`).
- **Read-Only Root Filesystem**: `read_only: true` with all capabilities dropped (`cap_drop: [ALL]`) and `no-new-privileges: true`.
- **Dragonfly / Redis Integration**: Production resource limits and health checks configured in Compose.

---

## 📁 Source Code Structure

```
├── .env.example              # Template for environment configuration
├── Cargo.toml                # Rust crate dependencies & feature flags
├── Dockerfile                # Multi-stage hardened scratch build
├── docker-compose.yml        # Orchestration for Ruxy API & Dragonfly/Redis
├── LICENSE                   # MIT License
├── README.md                 # Public documentation and scalability guide
├── PROJECT.md                # Project architecture & feature breakdown
└── src/
    ├── main.rs               # Server startup, routing, middleware & Swagger setup
    ├── state.rs              # AppState, Redis commands & Slack alerting helper
    ├── proxy.rs              # Reverse proxy handler, header stripping & streaming
    └── admin.rs              # Admin CRUD endpoints & route validation
```

---

## 🔮 Future Roadmap & Scalability Enhancements

- [ ] **L1 In-Memory Cache (Moka / DashMap)**: Short-lived local caching for route keys to reduce Redis roundtrips to sub-microsecond latency.
- [ ] **Redis Pub/Sub Cache Invalidation**: Real-time cache invalidation across distributed proxy nodes on route updates.
- [ ] **Circuit Breaker / Upstream Health Checks**: Proactive upstream probing with automatic failover to backup target URLs.
- [ ] **Prometheus / OpenTelemetry Metrics**: Exporting `/metrics` for Prometheus scraping (p99 latency, 5xx error rates, QPS).
- [ ] **Rate Limiting**: Distributed token-bucket rate limiting per API key or IP address.

---

## 🐛 Bugs, Edge Cases & Corner Cases Audit

> Full code review performed across `proxy.rs`, `admin.rs`, `state.rs`, `main.rs`, `Dockerfile`, `docker-compose.yml`, and `ci.yml`.

---

### 🔴 Bugs (Functional Issues)

#### B1. Body Buffering Defeats Streaming — Memory Spike Risk
**File:** [`proxy.rs:74-87`](file:///Users/hmtanbir/www/bdmade/proxy/src/proxy.rs#L74-L87)
```rust
let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
```
- The request body is fully buffered into memory (up to 10 MiB) before forwarding. Under 1,000 concurrent requests with 10 MiB bodies, this consumes **~10 GiB of RAM**.
- **Fix:** Stream the request body directly to `reqwest` using `reqwest::Body::wrap_stream()` instead of `to_bytes()`.

#### B2. Shallow Health Check — `/health` Returns `"OK"` Even When Redis Is Down
**File:** [`main.rs:46-48`](file:///Users/hmtanbir/www/bdmade/proxy/src/main.rs#L46-L48)
```rust
async fn health() -> impl IntoResponse { "OK" }
```
- Returns `200 OK` unconditionally. Docker Compose `healthcheck` and load balancers will consider the service healthy even if Redis is unreachable.
- **Fix:** Issue a `PING` to Redis inside the health handler. Return `503 Service Unavailable` on failure.

#### B3. `get_target` Silently Swallows Redis Errors
**File:** [`state.rs:93-101`](file:///Users/hmtanbir/www/bdmade/proxy/src/state.rs#L93-L101)
```rust
let val: Option<String> = redis::cmd("GET") ... .ok();
```
- `.ok()` converts **all** Redis errors (network failure, timeout, auth failure) into `None`, making them indistinguishable from a genuinely missing route.
- A client gets `404 route not found` when the actual cause is "Redis is down."
- **Fix:** Propagate the error and return `503`/`502` on Redis connectivity issues, not `404`.

#### B4. `list_routes` Silently Breaks on Redis Error — Returns Partial Results
**File:** [`state.rs:108-119`](file:///Users/hmtanbir/www/bdmade/proxy/src/state.rs#L108-L119)
```rust
Err(_) => break,
```
- If Redis fails mid-scan, the loop silently breaks and returns whatever partial data was collected. The admin sees an incomplete route list with no error indication.
- **Fix:** Return a `Result` and surface the error to the admin endpoint as `500 Internal Server Error`.

#### B5. Timing-Insecure Token Comparison
**File:** [`main.rs:112-116`](file:///Users/hmtanbir/www/bdmade/proxy/src/main.rs#L112-L116)
```rust
.map(|t| t == app_state.admin_token)
```
- Standard `==` comparison on secret tokens is vulnerable to timing side-channel attacks. An attacker can statistically determine the token character-by-character.
- **Fix:** Use constant-time comparison (e.g. `subtle::ConstantTimeEq` or `ring::constant_time::verify_slices_are_equal`).

---

### 🟡 Edge Cases

#### E1. Route Key Collision with Reserved Paths
**File:** [`proxy.rs:28-38`](file:///Users/hmtanbir/www/bdmade/proxy/src/proxy.rs#L28-L38)
- An admin can create a route with key `admin-panel` or `swagger-docs` — these work fine. But keys literally starting with `admin`, `swagger`, or `api-docs` (e.g. `adminstats`, `swagger2`) silently become inaccessible via the proxy because `starts_with("admin")` blocks them.
- **Fix:** Check exact segment matches (`raw_path == "admin" || raw_path.starts_with("admin/")`) instead of prefix-matching.

#### E2. Request Body >10 MiB Is Silently Rejected with Generic Error
**File:** [`proxy.rs:75`](file:///Users/hmtanbir/www/bdmade/proxy/src/proxy.rs#L75)
- Bodies exceeding 10 MiB return `400 Bad Request` with `"invalid body"`. The client gets no indication it's a size limit issue.
- **Fix:** Return `413 Payload Too Large` with a descriptive message.

#### E3. `value` Field Not Trimmed or Validated for URL Well-Formedness
**File:** [`admin.rs:50`](file:///Users/hmtanbir/www/bdmade/proxy/src/admin.rs#L50)
- Only checks prefix `http://` or `https://`. Values like `https://` (no host), `https:// ` (trailing space), or `https://not a url` pass validation but cause runtime failures when the proxy tries to forward.
- **Fix:** Parse with `url::Url::parse()` and reject if host is empty or scheme is invalid.

#### E4. Redirect Rewriting Fails When Target Has a Trailing Slash Mismatch
**File:** [`proxy.rs:118-128`](file:///Users/hmtanbir/www/bdmade/proxy/src/proxy.rs#L118-L128)
- `target` is stored as-is (e.g. `https://api.example.com/v1/`) but `target_trimmed` strips trailing slashes for URL building. The redirect `strip_prefix` comparison uses the untrimmed `target`, so if the upstream `Location` header uses the trimmed form, the rewrite silently fails and the raw upstream URL leaks to the client.
- **Fix:** Normalize `target` consistently (always strip or always keep trailing slash) and compare against the normalized form.

#### E5. `delete_route` Returns `200 OK` Even When Route Doesn't Exist
**File:** [`admin.rs:170-182`](file:///Users/hmtanbir/www/bdmade/proxy/src/admin.rs#L170-L182)
- When `deleted == 0` (route not found), the endpoint still returns `200 OK` with `success: false`.
- **Fix:** Return `404 Not Found` status code when the route doesn't exist.

#### E6. No Request Timeout on Upstream Forwarding
**File:** [`main.rs:65-68`](file:///Users/hmtanbir/www/bdmade/proxy/src/main.rs#L65-L68)
- The `reqwest::Client` is built without any `timeout()`. If an upstream backend hangs indefinitely, the proxy connection stays open forever, eventually exhausting all Tokio tasks.
- **Fix:** Set `.timeout(Duration::from_secs(30))` and `.connect_timeout(Duration::from_secs(5))` on the client builder.

---

### 🟠 Corner Cases

#### C1. URL-Encoded or Unicode Route Keys
- A client requesting `/%2Fadmin` (URL-encoded `/admin`) bypasses the `starts_with("admin")` guard after `trim_start_matches('/')`. Axum may or may not decode percent-encoding before the handler sees it, depending on version.
- Route keys containing Unicode, emoji, or special characters (`webhook/日本語`) are stored in Redis as raw UTF-8 but may behave inconsistently across HTTP clients.

#### C2. Header Injection via Upstream Response
- Upstream backends can return duplicate or malicious headers (e.g. multiple `set-cookie` headers, CRLF injection attempts). These are forwarded as-is without sanitization.

#### C3. Slack Webhook Flooding Under Cascading Failures
- If an upstream backend is down, **every** proxied request fires a Slack webhook. Under 10k req/s to a dead backend, this sends 10k Slack API calls per second, likely getting rate-limited or banned by Slack.
- **Fix:** Implement a debounce/throttle mechanism (e.g. one Slack alert per route per 60 seconds using a `DashMap<String, Instant>`).

#### C4. CI Test Job Never Actually Runs `cargo test`
**File:** [`ci.yml:26-52`](file:///Users/hmtanbir/www/bdmade/proxy/.github/workflows/ci.yml#L26-L52)
- The `test` job sets up a Redis service and installs Rust but only runs `cargo clippy`. It never executes `cargo test`. The job name is misleading.
- **Fix:** Add `cargo test` step to the `test` job.

#### C5. Docker Compose Healthcheck Uses `curl` Inside a `scratch` Image
**File:** [`docker-compose.yml`](file:///Users/hmtanbir/www/bdmade/proxy/docker-compose.yml)
- The API container is built `FROM scratch` which contains no shell, no `curl`, no `wget`. If you add a `healthcheck` with `curl` to the API service, it will fail. Currently only Redis has a healthcheck — the API container has none, so Docker cannot auto-restart it on failure.
- **Fix:** Add a healthcheck using a dedicated binary, or add a small static healthcheck binary to the scratch image, or use Docker's `CMD` healthcheck with the Ruxy binary itself (e.g. `/proxy-api --health-check`).

#### C6. `delete_route` Redis Error Is Silently Swallowed
**File:** [`admin.rs:170`](file:///Users/hmtanbir/www/bdmade/proxy/src/admin.rs#L170)
```rust
let deleted: i64 = conn.del(route_key(clean_key)).await.unwrap_or(0);
```
- `unwrap_or(0)` treats Redis connection failures as "route not found." A real Redis outage appears as a successful no-op delete.
- **Fix:** Handle the error explicitly and return `500` + fire a Slack alert.

---

### 📊 Summary Matrix

| ID | Severity | Category | File | Status |
| :--- | :--- | :--- | :--- | :--- |
| B1 | 🔴 High | Memory | `proxy.rs` | ✅ Closed |
| B2 | 🔴 High | Reliability | `main.rs` | ✅ Closed |
| B3 | 🔴 High | Reliability | `state.rs` | ✅ Closed |
| B4 | 🟡 Medium | Data Integrity | `state.rs` | ✅ Closed |
| B5 | 🟡 Medium | Security | `main.rs` | ✅ Closed |
| E1 | 🟡 Medium | Routing | `proxy.rs` | ✅ Closed |
| E2 | 🟡 Medium | UX | `proxy.rs` | ✅ Closed (mitigated by streaming) |
| E3 | 🟡 Medium | Validation | `admin.rs` | ✅ Closed |
| E4 | 🟡 Medium | Security | `proxy.rs` | ✅ Closed |
| E5 | 🟡 Low | API Design | `admin.rs` | ✅ Closed |
| E6 | 🔴 High | Reliability | `main.rs` | ✅ Closed |
| C1 | 🟠 Low | Encoding | `proxy.rs` | ⚠️ Open |
| C2 | 🟠 Low | Security | `proxy.rs` | ⚠️ Open |
| C3 | 🟡 Medium | Alerting | `state.rs` | ✅ Closed |
| C4 | 🟡 Medium | CI/CD | `ci.yml` | ✅ Closed |
| C5 | 🟠 Low | Ops | `docker-compose.yml` | ⚠️ Open (deferred — `scratch` image constraint) |
| C6 | 🟡 Medium | Reliability | `admin.rs` | ✅ Closed |


