use crate::state::{get_target, AppState};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};

const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

pub async fn proxy_handler(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    request: Request<Body>,
) -> impl IntoResponse {
    let raw_path = uri.path().trim_start_matches('/');
    if raw_path.is_empty()
        || raw_path.starts_with("admin")
        || raw_path.starts_with("swagger")
        || raw_path.starts_with("api-docs")
    {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("content-type", "application/json")
            .body(Body::from("{\"error\":\"not found\"}"))
            .unwrap();
    }

    // Split key and subpath, e.g. "1234/api/v1" -> key: "1234", subpath: "/api/v1"
    let (key, subpath) = match raw_path.split_once('/') {
        Some((k, rest)) => (k, format!("/{}", rest)),
        None => (raw_path, String::new()),
    };

    let target = match get_target(&state, key).await {
        Some(t) => t,
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("content-type", "application/json")
                .body(Body::from("{\"error\":\"route not found\"}"))
                .unwrap();
        }
    };

    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let target_trimmed = target.trim_end_matches('/');
    let url = format!("{target_trimmed}{subpath}{query}");

    let mut req = state.client.request(
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
        &url,
    );

    for (name, value) in headers.iter() {
        let n = name.as_str();
        if matches!(n, "host" | "x-api-key") || HOP_BY_HOP.contains(&n) {
            continue;
        }
        req = req.header(n, value);
    }

    let body = request.into_body();
    let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("content-type", "application/json")
                .body(Body::from("{\"error\":\"invalid body\"}"))
                .unwrap();
        }
    };
    if !bytes.is_empty() {
        req = req.body(bytes.to_vec());
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("forward failed for '{key}' -> {url}: {e}");
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("content-type", "application/json")
                .body(Body::from("{\"error\":\"upstream unreachable\"}"))
                .unwrap();
        }
    };

    let mut builder = Response::builder().status(resp.status().as_u16());
    for (name, value) in resp.headers().iter() {
        let n = name.as_str();
        if matches!(n, "server" | "via" | "x-powered-by" | "location") || HOP_BY_HOP.contains(&n) {
            continue;
        }
        builder = builder.header(n, value);
    }
    let status_is_redirect = resp.status().is_redirection();
    let location = resp.headers().get("location").and_then(|v| v.to_str().ok());
    if status_is_redirect {
        if let Some(loc) = location {
            // rewrite redirect targets back to our own host so the origin stays hidden
            if let Some(stripped) = loc.strip_prefix(&target) {
                builder = builder.header("location", format!("/{key}{stripped}"));
            } else {
                builder = builder.header("location", loc);
            }
        }
    }

    let stream = resp.bytes_stream();
    builder.body(Body::from_stream(stream)).unwrap()
}
