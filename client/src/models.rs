use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct RouteEntry {
    pub key: String,
    #[serde(alias = "target")]
    pub value: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ApiItemResponse {
    pub data: Option<RouteEntry>,
    pub message: String,
    pub status: u16,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ApiListResponse {
    pub data: Vec<RouteEntry>,
    pub message: String,
    pub status: u16,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub message: String,
}
