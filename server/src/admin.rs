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
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub message: String,
}

/// Authenticate admin using dashboard password
#[utoipa::path(
    post,
    path = "/admin/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials", body = LoginResponse)
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let is_valid = payload
        .password
        .as_bytes()
        .ct_eq(state.admin_password.as_bytes())
        .unwrap_u8()
        == 1;

    if is_valid {
        (
            StatusCode::OK,
            Json(LoginResponse {
                success: true,
                token: Some(state.admin_token.clone()),
                message: "Login successful".into(),
            }),
        )
            .into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                success: false,
                token: None,
                message: "Invalid password".into(),
            }),
        )
            .into_response()
    }
}

/// Create or update a route mapping
#[utoipa::path(
    post,
    path = "/admin/api/routes",
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
    let parsed_url = match url::Url::parse(&payload.value) {
        Ok(u) => {
            if u.scheme() != "http" && u.scheme() != "https" {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse {
                        success: false,
                        message: "value must be an http or https URL".into(),
                    }),
                )
                    .into_response();
            }
            if u.host().is_none() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse {
                        success: false,
                        message: "value must contain a valid host".into(),
                    }),
                )
                    .into_response();
            }
            u.to_string()
        }
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse {
                    success: false,
                    message: "value must be a valid URL".into(),
                }),
            )
                .into_response();
        }
    };

    let mut conn = state.redis.clone();
    if let Err(e) = conn.set::<_, _, ()>(route_key(key), parsed_url).await {
        tracing::error!("failed to write route to redis: {e}");
        crate::state::notify_slack_debounced(
            &state,
            key,
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
    path = "/admin/api/routes/{key}",
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
        Ok(Some(value)) => (
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
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiItemResponse {
                data: None,
                message: format!("route '{clean_key}' not found"),
                status: 404,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Redis error looking up route '{}': {}", clean_key, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiItemResponse {
                    data: None,
                    message: "failed to fetch route".into(),
                    status: 500,
                }),
            )
                .into_response()
        }
    }
}

/// List all route mappings
#[utoipa::path(
    get,
    path = "/admin/api/routes",
    responses(
        (status = 200, description = "List of route mappings", body = ApiListResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(("api_key" = []))
)]
pub async fn list_routes(State(state): State<AppState>) -> impl IntoResponse {
    match state_list_routes(&state).await {
        Ok(routes) => (
            StatusCode::OK,
            Json(ApiListResponse {
                data: routes,
                message: "Successfully data fetched".into(),
                status: 200,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("failed to list routes: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiListResponse {
                    data: vec![],
                    message: "failed to fetch routes".into(),
                    status: 500,
                }),
            )
                .into_response()
        }
    }
}

/// Delete a route mapping
#[utoipa::path(
    delete,
    path = "/admin/api/routes/{key}",
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
    match conn.del::<_, i64>(route_key(clean_key)).await {
        Ok(deleted) => {
            if deleted > 0 {
                (
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        message: format!("route '{clean_key}' deleted"),
                    }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse {
                        success: false,
                        message: format!("route '{clean_key}' not found"),
                    }),
                )
                    .into_response()
            }
        }
        Err(e) => {
            tracing::error!("failed to delete route: {e}");
            crate::state::notify_slack_debounced(
                &state,
                clean_key,
                "Redis Delete Error (Admin Route)".into(),
                format!("*Route Key:* `{}`\n*Error Details:* ```{}```", clean_key, e),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    message: "failed to delete route".into(),
                }),
            )
                .into_response()
        }
    }
}
