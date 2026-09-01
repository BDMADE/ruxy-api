use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct AppState {
    pub redis: ConnectionManager,
    pub admin_token: String,
    pub admin_password: String,
    pub client: reqwest::Client,
    pub slack_webhook_url: Option<String>,
    pub slack_rate_limits: std::sync::Arc<dashmap::DashMap<String, std::time::Instant>>,
    pub slack_rate_limit_seconds: u64,
}

pub fn notify_slack_debounced(
    state: &AppState,
    debounce_key: &str,
    title: String,
    details: String,
) {
    let now = std::time::Instant::now();
    let limit_duration = std::time::Duration::from_secs(state.slack_rate_limit_seconds);

    // Check if we should debounce
    if let Some(last_sent) = state.slack_rate_limits.get(debounce_key) {
        if now.duration_since(*last_sent) < limit_duration {
            return; // Debounced
        }
    }

    // Update the timestamp
    state
        .slack_rate_limits
        .insert(debounce_key.to_string(), now);

    let client = state.client.clone();
    let webhook_url = state.slack_webhook_url.clone();

    if let Some(url) = webhook_url {
        if !url.trim().is_empty() {
            tokio::spawn(async move {
                let payload = serde_json::json!({
                    "text": format!("🚨 *{}*\n{}", title, details),
                    "blocks": [
                        {
                            "type": "header",
                            "text": {
                                "type": "plain_text",
                                "text": format!("🚨 {}", title),
                                "emoji": true
                            }
                        },
                        {
                            "type": "section",
                            "text": {
                                "type": "mrkdwn",
                                "text": details
                            }
                        },
                        {
                            "type": "context",
                            "elements": [
                                {
                                    "type": "mrkdwn",
                                    "text": format!("*Service:* `ruxy-api` | *Timestamp:* `{:?}`", std::time::SystemTime::now())
                                }
                            ]
                        }
                    ]
                });

                if let Err(e) = client.post(&url).json(&payload).send().await {
                    tracing::warn!("failed to deliver slack notification: {e}");
                }
            });
        }
    }
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

pub async fn get_target(state: &AppState, key: &str) -> Result<Option<String>, redis::RedisError> {
    let mut conn = state.redis.clone();
    let val: Option<String> = redis::cmd("GET")
        .arg(route_key(key))
        .query_async(&mut conn)
        .await?;
    Ok(val)
}

pub async fn list_routes(state: &AppState) -> Result<Vec<RouteEntry>, redis::RedisError> {
    let mut conn = state.redis.clone();
    let mut cursor: u64 = 0;
    let mut entries: Vec<RouteEntry> = Vec::new();

    loop {
        let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("route:*")
            .arg("COUNT")
            .arg(100)
            .query_async(&mut conn)
            .await?;

        for full_key in batch {
            let key = full_key.trim_start_matches("route:").to_string();
            let val: Option<String> = redis::cmd("GET")
                .arg(&full_key)
                .query_async(&mut conn)
                .await?;

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
    Ok(entries)
}
