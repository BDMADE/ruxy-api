# Proxy API Service

Rust + Redis reverse proxy. Public requests to `https://proxy.bdmade.com/:key` are forwarded to a target URL stored in Redis, so the origin host stays hidden.

## Run

```bash
cp .env.example .env   # set ADMIN_TOKEN
cargo run
```

## Admin CRUD (Swagger: http://localhost:3000/swagger-ui)

All admin endpoints require header `x-api-key: <ADMIN_TOKEN>`.

| Method | Path | Description |
|---|---|---|
| POST | `/admin/routes` | Create/update mapping `{ "key": "1234", "target": "https://automation1.bdmade.com/1234" }` |
| GET | `/admin/routes` | List all keys |
| GET | `/admin/routes/{key}` | Get mapping |
| DELETE | `/admin/routes/{key}` | Delete mapping |

## Proxying

Any method on `/:key?query` is looked up in Redis (`route:{key}`) and forwarded to the stored target with the same method, headers, body, and query string.

- Response headers identifying the origin (`Server`, `Via`, `X-Powered-By`) are stripped.
- Redirects back to the target host are rewritten to the proxy host.
- 404 JSON if the key has no mapping; 502 if upstream is unreachable.

### Example flow

```bash
curl -X POST localhost:3000/admin/routes \
  -H "x-api-key: $ADMIN_TOKEN" -H 'content-type: application/json' \
  -d '{"key":"1234","target":"https://automation1.bdmade.com/1234"}'

curl -i localhost:3000/1234        # proxied to automation1.bdmade.com/1234
```

Change the backend any time by re-issuing the POST with a new target — no restart needed.
# Rudis
