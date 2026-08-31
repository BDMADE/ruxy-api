mod admin;
mod proxy;
mod state;

#[cfg(test)]
mod integration_tests;

use axum::{
    middleware,
    response::IntoResponse,
    routing::{any, delete, get, post},
    Router,
};
use redis::aio::ConnectionManager;
use tower_http::services::{ServeDir, ServeFile};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "api_key",
            utoipa::openapi::security::SecurityScheme::ApiKey(
                utoipa::openapi::security::ApiKey::Header(
                    utoipa::openapi::security::ApiKeyValue::new("x-api-key"),
                ),
            ),
        );
    }
}

use crate::admin::{LoginRequest, LoginResponse};
use crate::state::{ApiItemResponse, ApiListResponse, ApiResponse, RouteEntry};

#[derive(OpenApi)]
#[openapi(
    info(title = "Proxy API Service", description = "Reverse proxy with dynamic Redis-backed route mappings. Public traffic hits /:key which is forwarded to the mapped target URL. The origin stays hidden."),
    paths(
        admin::login,
        admin::create_route,
        admin::get_route,
        admin::list_routes,
        admin::delete_route
    ),
    components(schemas(LoginRequest, LoginResponse, RouteEntry, ApiItemResponse, ApiListResponse, ApiResponse)),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

async fn health(
    axum::extract::State(state): axum::extract::State<state::AppState>,
) -> impl IntoResponse {
    let mut conn = state.redis.clone();
    match redis::cmd("PING").query_async::<String>(&mut conn).await {
        Ok(pong) if pong == "PONG" => (axum::http::StatusCode::OK, "OK").into_response(),
        _ => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "REDIS_UNAVAILABLE",
        )
            .into_response(),
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let admin_token = std::env::var("ADMIN_TOKEN").expect("ADMIN_TOKEN must be set");
    let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| admin_token.clone());

    let client = redis::Client::open(redis_url).expect("invalid REDIS_URL");
    let conn = ConnectionManager::new(client)
        .await
        .expect("failed to connect to redis");

    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(20)
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build http client");

    let slack_webhook_url = std::env::var("SLACK_WEBHOOK_URL").ok();
    let slack_rate_limit_seconds = std::env::var("SLACK_RATE_LIMIT_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);

    let app_state = state::AppState {
        redis: conn,
        admin_token,
        admin_password,
        client: http_client,
        slack_webhook_url,
        slack_rate_limits: std::sync::Arc::new(dashmap::DashMap::new()),
        slack_rate_limit_seconds,
    };

    let app = app_router(app_state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind failed");
    tracing::info!("listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}

pub fn app_router(app_state: state::AppState) -> Router {
    let admin_router = Router::new()
        .route("/", post(admin::create_route))
        .route("/", get(admin::list_routes))
        .route("/{*key}", get(admin::get_route))
        .route("/{*key}", delete(admin::delete_route))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ));

    let public_dir = std::env::var("PUBLIC_DIR").unwrap_or_else(|_| "./client/dist".into());
    let serve_dir =
        ServeDir::new(&public_dir).fallback(ServeFile::new(format!("{}/index.html", public_dir)));

    Router::new()
        .nest_service("/admin/ui", serve_dir)
        .route("/health", get(health))
        .route("/admin/login", post(admin::login))
        .nest("/admin/routes", admin_router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .fallback(any(proxy::proxy_handler))
        .with_state(app_state)
}

async fn auth_middleware(
    axum::extract::State(app_state): axum::extract::State<state::AppState>,
    headers: axum::http::HeaderMap,
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use subtle::ConstantTimeEq;

    let ok = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|t| {
            t.as_bytes()
                .ct_eq(app_state.admin_token.as_bytes())
                .unwrap_u8()
                == 1
        })
        .unwrap_or(false);
    if !ok {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(ApiResponse {
                success: false,
                message: "Unauthorized".into(),
            }),
        )
            .into_response();
    }
    next.run(request).await
}
