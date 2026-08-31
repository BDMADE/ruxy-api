use gloo_net::http::{Request, Response};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone)]
pub struct ApiError(pub String);

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ApiError {}

async fn handle_response<T: for<'de> Deserialize<'de>>(res: Response) -> Result<T, ApiError> {
    if !res.ok() {
        if res.status() == 401 {
            // Token expired or invalid
            crate::auth::logout();
        }
        let msg = res.text().await.unwrap_or_else(|_| "Unknown error".into());
        return Err(ApiError(msg));
    }
    res.json::<T>().await.map_err(|e| ApiError(e.to_string()))
}

pub async fn api_get<T: for<'de> Deserialize<'de>>(
    path: &str,
    token: Option<&str>,
) -> Result<T, ApiError> {
    let mut req = Request::get(path);
    if let Some(t) = token {
        req = req.header("x-api-key", t);
    }
    let res = req.send().await.map_err(|e| ApiError(e.to_string()))?;
    handle_response(res).await
}

pub async fn api_post<B: Serialize, T: for<'de> Deserialize<'de>>(
    path: &str,
    token: Option<&str>,
    body: &B,
) -> Result<T, ApiError> {
    let mut req = Request::post(path);
    if let Some(t) = token {
        req = req.header("x-api-key", t);
    }
    let res = req
        .json(body)
        .map_err(|e| ApiError(e.to_string()))?
        .send()
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    handle_response(res).await
}

pub async fn api_delete<T: for<'de> Deserialize<'de>>(
    path: &str,
    token: Option<&str>,
) -> Result<T, ApiError> {
    let mut req = Request::delete(path);
    if let Some(t) = token {
        req = req.header("x-api-key", t);
    }
    let res = req.send().await.map_err(|e| ApiError(e.to_string()))?;
    handle_response(res).await
}
