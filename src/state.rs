use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct AppState {
    pub redis: ConnectionManager,
    pub admin_token: String,
    pub client: reqwest::Client,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct RouteMapping {
    /// Public proxy key, e.g. "1234"
    pub key: String,
    /// Full target URL stored in Redis, e.g. "https://automation1.bdmade.com/1234"
    pub target: String,
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

pub async fn list_keys(state: &AppState) -> Vec<String> {
    let mut conn = state.redis.clone();
    let mut cursor: u64 = 0;
    let mut keys: Vec<String> = Vec::new();

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

        for k in batch {
            keys.push(k.trim_start_matches("route:").to_string());
        }

        if next_cursor == 0 {
            break;
        }
        cursor = next_cursor;
    }

    keys.sort();
    keys
}
