use crate::state::{
    get_target, list_routes as state_list_routes, route_key, ApiItemResponse, ApiListResponse,
    ApiResponse, AppState, RouteEntry,
};
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
    request_body = RouteEntry,
    responses(
        (status = 200, description = "Route created/updated", body = ApiResponse),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Invalid target URL")
    ),
    security(("api_key" = []))
)]
pub async fn create_route(
    State(state): State<AppState>,
    Json(payload): Json<RouteEntry>,
) -> impl IntoResponse {
    let key = payload.key.trim().trim_matches('/');
    if key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "key cannot be empty".into(),
            }),
        )
            .into_response();
    }
    if key.contains("://") {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "key cannot contain '://'".into(),
            }),
        )
            .into_response();
    }
    if !payload.value.starts_with("http://") && !payload.value.starts_with("https://") {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "value must start with http:// or https://".into(),
            }),
        )
            .into_response();
    }
    let mut conn = state.redis.clone();
    if let Err(e) = conn.set::<_, _, ()>(route_key(key), payload.value).await {
        tracing::error!("failed to write route to redis: {e}");
        crate::state::notify_slack(
            state.client.clone(),
            state.slack_webhook_url.clone(),
            "Redis Write Error (Admin Route Create)".into(),
            format!("*Route Key:* `{}`\n*Error Details:* ```{}```", key, e),
        );
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: "failed to save route".into(),
            }),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: format!("route '{key}' saved"),
        }),
    )
        .into_response()
}

/// Get a route mapping
#[utoipa::path(
    get,
    path = "/admin/routes/{key}",
    params(("key" = String, Path, description = "Proxy key (URL or identifier)")),
    responses(
        (status = 200, description = "Mapping found", body = ApiItemResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found", body = ApiItemResponse)
    ),
    security(("api_key" = []))
)]
pub async fn get_route(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    let clean_key = key.trim().trim_matches('/');
    match get_target(&state, clean_key).await {
        Some(value) => (
            StatusCode::OK,
            Json(ApiItemResponse {
                data: Some(RouteEntry {
                    key: clean_key.to_string(),
                    value,
                }),
                message: "Successfully data fetched".into(),
                status: 200,
            }),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiItemResponse {
                data: None,
                message: format!("route '{clean_key}' not found"),
                status: 404,
            }),
        )
            .into_response(),
    }
}

/// List all route mappings
#[utoipa::path(
    get,
    path = "/admin/routes",
    responses(
        (status = 200, description = "List of route mappings", body = ApiListResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(("api_key" = []))
)]
pub async fn list_routes(State(state): State<AppState>) -> impl IntoResponse {
    let routes = state_list_routes(&state).await;
    (
        StatusCode::OK,
        Json(ApiListResponse {
            data: routes,
            message: "Successfully data fetched".into(),
            status: 200,
        }),
    )
        .into_response()
}

/// Delete a route mapping
#[utoipa::path(
    delete,
    path = "/admin/routes/{key}",
    params(("key" = String, Path, description = "Proxy key (URL or identifier)")),
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
    let clean_key = key.trim().trim_matches('/');
    let mut conn = state.redis.clone();
    let deleted: i64 = conn.del(route_key(clean_key)).await.unwrap_or(0);
    (
        StatusCode::OK,
        Json(ApiResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                format!("route '{clean_key}' deleted")
            } else {
                format!("route '{clean_key}' not found")
            },
        }),
    )
        .into_response()
}
