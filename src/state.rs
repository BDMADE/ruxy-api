use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct AppState {
    pub redis: ConnectionManager,
    pub admin_token: String,
    pub client: reqwest::Client,
}

#[derive(Deserialize, Serialize, ToSchema, Clone, Debug)]
pub struct RouteEntry {
    /// Identifier or key, e.g. "https://yeapin.xyz" or "1234"
    pub key: String,
    /// Destination URL value (accepts both "value" and "target")
    #[serde(alias = "target")]
    pub value: String,
}

#[derive(Serialize, ToSchema)]
pub struct ApiItemResponse {
    pub data: Option<RouteEntry>,
    pub message: String,
    pub status: u16,
}

#[derive(Serialize, ToSchema)]
pub struct ApiListResponse {
    pub data: Vec<RouteEntry>,
    pub message: String,
    pub status: u16,
}

#[derive(Serialize, ToSchema)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

pub fn route_key(key: &str) -> String {
    format!("route:{key}")
}

pub async fn get_target(state: &AppState, key: &str) -> Option<String> {
    let mut conn = state.redis.clone();
    let val: Option<String> = redis::cmd("GET")
        .arg(route_key(key))
        .query_async(&mut conn)
        .await
        .ok();
    val
}

pub async fn list_routes(state: &AppState) -> Vec<RouteEntry> {
    let mut conn = state.redis.clone();
    let mut cursor: u64 = 0;
    let mut entries: Vec<RouteEntry> = Vec::new();

    loop {
        let (next_cursor, batch): (u64, Vec<String>) = match redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("route:*")
            .arg("COUNT")
            .arg(100)
            .query_async(&mut conn)
            .await
        {
            Ok(res) => res,
            Err(_) => break,
        };

        for full_key in batch {
            let key = full_key.trim_start_matches("route:").to_string();
            let val: Option<String> = redis::cmd("GET")
                .arg(&full_key)
                .query_async(&mut conn)
                .await
                .ok();

            if let Some(value) = val {
                entries.push(RouteEntry { key, value });
            }
        }

        if next_cursor == 0 {
            break;
        }
        cursor = next_cursor;
    }

    entries.sort_by(|a, b| a.key.cmp(&b.key));
    entries
}
