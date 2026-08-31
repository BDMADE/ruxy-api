mod admin;
mod proxy;
mod state;
use axum::{
    middleware,
    response::IntoResponse,
    routing::{any, delete, get, post},
    Router,
};
use redis::aio::ConnectionManager;
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

use crate::state::{ApiItemResponse, ApiListResponse, ApiResponse, RouteEntry};

#[derive(OpenApi)]
#[openapi(
    info(title = "Proxy API Service", description = "Reverse proxy with dynamic Redis-backed route mappings. Public traffic hits /:key which is forwarded to the mapped target URL. The origin stays hidden."),
    paths(
        admin::create_route,
        admin::get_route,
        admin::list_routes,
        admin::delete_route
    ),
    components(schemas(RouteEntry, ApiItemResponse, ApiListResponse, ApiResponse)),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

async fn health() -> impl IntoResponse {
    "OK"
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let admin_token = std::env::var("ADMIN_TOKEN").expect("ADMIN_TOKEN must be set");

    let client = redis::Client::open(redis_url).expect("invalid REDIS_URL");
    let conn = ConnectionManager::new(client)
        .await
        .expect("failed to connect to redis");

    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(20)
        .build()
        .expect("failed to build http client");

    let slack_webhook_url = std::env::var("SLACK_WEBHOOK_URL").ok();

    let app_state = state::AppState {
        redis: conn,
        admin_token,
        client: http_client,
        slack_webhook_url,
    };

    let admin_router = Router::new()
        .route("/", post(admin::create_route))
        .route("/", get(admin::list_routes))
        .route("/{*key}", get(admin::get_route))
        .route("/{*key}", delete(admin::delete_route))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .route("/health", get(health))
        .nest("/admin/routes", admin_router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .fallback(any(proxy::proxy_handler))
        .with_state(app_state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind failed");
    tracing::info!("listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}

async fn auth_middleware(
    axum::extract::State(app_state): axum::extract::State<state::AppState>,
    headers: axum::http::HeaderMap,
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    use axum::http::StatusCode;
    let ok = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|t| t == app_state.admin_token)
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
