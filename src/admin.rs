use crate::state::{get_target, route_key, ApiResponse, AppState, RouteMapping};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use redis::AsyncCommands;

/// Create or update a route mapping
#[utoipa::path(
    post,
    path = "/admin/routes",
    request_body = RouteMapping,
    responses(
        (status = 200, description = "Route created/updated", body = ApiResponse),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Invalid target URL")
    ),
    security(("api_key" = []))
)]
pub async fn create_route(
    State(state): State<AppState>,
    Json(payload): Json<RouteMapping>,
) -> impl IntoResponse {
    let key = payload.key.trim().trim_matches('/');
    if key.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse { success: false, message: "key cannot be empty".into() })).into_response();
    }
    if !payload.target.starts_with("http://") && !payload.target.starts_with("https://") {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse { success: false, message: "target must start with http:// or https://".into() })).into_response();
    }
    let mut conn = state.redis.clone();
    if let Err(e) = conn.set::<_, _, ()>(route_key(key), payload.target).await {
        tracing::error!("failed to write route to redis: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse { success: false, message: "failed to save route".into() })).into_response();
    }
    (StatusCode::OK, Json(ApiResponse { success: true, message: format!("route '{key}' saved") })).into_response()
}

/// Get a route mapping
#[utoipa::path(
    get,
    path = "/admin/routes/{key}",
    params(("key" = String, Path, description = "Proxy key")),
    responses(
        (status = 200, description = "Mapping found", body = RouteMapping),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found")
    ),
    security(("api_key" = []))
)]
pub async fn get_route(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match get_target(&state, &key).await {
        Some(target) => (
            StatusCode::OK,
            Json(serde_json::to_value(RouteMapping { key, target }).unwrap()),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::to_value(ApiResponse { success: false, message: "not found".into() }).unwrap()),
        )
            .into_response(),
    }
}

/// List all route keys
#[utoipa::path(
    get,
    path = "/admin/routes",
    responses(
        (status = 200, description = "List of keys", body = Vec<String>),
        (status = 401, description = "Unauthorized")
    ),
    security(("api_key" = []))
)]
pub async fn list_routes(State(state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(crate::state::list_keys(&state).await)).into_response()
}

/// Delete a route mapping
#[utoipa::path(
    delete,
    path = "/admin/routes/{key}",
    params(("key" = String, Path, description = "Proxy key")),
    responses(
        (status = 200, description = "Deleted", body = ApiResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(("api_key" = []))
)]
pub async fn delete_route(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    let mut conn = state.redis.clone();
    let deleted: i64 = conn.del(route_key(&key)).await.unwrap_or(0);
    (
        StatusCode::OK,
        Json(ApiResponse {
            success: deleted > 0,
            message: if deleted > 0 { format!("route '{key}' deleted") } else { format!("route '{key}' not found") },
        }),
    ).into_response()
}
