#[cfg(test)]
mod tests {
    use crate::{
        app_router,
        state::{get_target, list_routes, notify_slack_debounced, route_key, AppState},
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use dashmap::DashMap;
    use http_body_util::BodyExt;
    use redis::{aio::ConnectionManager, AsyncCommands};
    use std::sync::Arc;
    use std::time::Duration;
    use tower::ServiceExt;
    use uuid::Uuid;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    async fn setup_state() -> (AppState, String) {
        dotenvy::dotenv().ok();
        let mut redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
            
        // If the URL from .env points to the docker-compose internal 'redis' hostname,
        // rewrite it to localhost so cargo test on the host machine works.
        redis_url = redis_url.replace("@redis:6379", "@127.0.0.1:6379");
        redis_url = redis_url.replace("redis://redis:6379", "redis://127.0.0.1:6379");
        let client = redis::Client::open(redis_url).expect("invalid REDIS_URL");
        
        // Use a timeout so tests fail fast instead of hanging forever if Redis is down
        let conn = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            ConnectionManager::new(client)
        )
        .await
        .expect("Redis connection timed out. Is Redis running locally on port 6379?")
        .expect("Failed to connect to redis");

        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(20)
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();

        let prefix = format!("test_{}", Uuid::new_v4().simple());
        
        let app_state = AppState {
            redis: conn,
            admin_token: "test-secret".to_string(),
            client: http_client,
            slack_webhook_url: None, // No real slack calls during tests
            slack_rate_limits: Arc::new(DashMap::new()),
            slack_rate_limit_seconds: 60,
        };

        (app_state, prefix)
    }

    async fn get_body_string(response: axum::response::Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    // -------------------------------------------------------------------------
    // state.rs tests
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_route_key() {
        assert_eq!(route_key("foo"), "route:foo");
        assert_eq!(route_key("bar/baz"), "route:bar/baz");
    }

    #[tokio::test]
    async fn test_get_target_and_list() {
        let (state, prefix) = setup_state().await;
        let mut conn = state.redis.clone();
        let key1 = format!("{prefix}_key1");
        let key2 = format!("{prefix}_key2");

        // Should be None initially
        assert_eq!(get_target(&state, &key1).await.unwrap(), None);

        // Set
        let _: () = conn
            .set(route_key(&key1), "http://localhost:8080")
            .await
            .unwrap();
        let _: () = conn
            .set(route_key(&key2), "https://example.com")
            .await
            .unwrap();

        // Check get_target
        assert_eq!(
            get_target(&state, &key1).await.unwrap(),
            Some("http://localhost:8080".to_string())
        );

        // Check list_routes
        let routes = list_routes(&state).await.unwrap();
        // Since we are running in parallel, other test keys might exist. Let's just find ours.
        let route1 = routes.iter().find(|r| r.key == key1).unwrap();
        assert_eq!(route1.value, "http://localhost:8080");

        let route2 = routes.iter().find(|r| r.key == key2).unwrap();
        assert_eq!(route2.value, "https://example.com");

        // Clean up
        let _: () = conn.del(route_key(&key1)).await.unwrap();
        let _: () = conn.del(route_key(&key2)).await.unwrap();
    }

    #[tokio::test]
    async fn test_notify_slack_debounced() {
        let (mut state, _) = setup_state().await;
        state.slack_rate_limit_seconds = 1; // 1 second debounce for test

        let key = "debounce_test_key";
        
        assert!(state.slack_rate_limits.get(key).is_none());
        
        notify_slack_debounced(&state, key, "Test".into(), "Details".into());
        
        // Now it should be set
        let first_timestamp = *state.slack_rate_limits.get(key).unwrap();
        
        // Second call immediately should debounce (timestamp won't change)
        notify_slack_debounced(&state, key, "Test".into(), "Details".into());
        let second_timestamp = *state.slack_rate_limits.get(key).unwrap();
        assert_eq!(first_timestamp, second_timestamp);
        
        // Wait 1.1s
        tokio::time::sleep(Duration::from_millis(1100)).await;
        
        // Third call should go through and update timestamp
        notify_slack_debounced(&state, key, "Test".into(), "Details".into());
        let third_timestamp = *state.slack_rate_limits.get(key).unwrap();
        assert!(third_timestamp > first_timestamp);
    }
    
    #[tokio::test]
    async fn test_notify_slack_live_webhook_fires() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
            
        let (mut state, _) = setup_state().await;
        // Mock a real webhook URL
        state.slack_webhook_url = Some(mock_server.uri());
        state.slack_rate_limit_seconds = 1;

        let key = "debounce_live_test_key";
        notify_slack_debounced(&state, key, "Live Test".into(), "Details".into());
        
        // Give the tokio::spawn task time to run and make the HTTP request
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body = String::from_utf8(requests[0].body.clone()).unwrap();
        assert!(body.contains("Live Test"));
    }

    // -------------------------------------------------------------------------
    // main.rs (middleware & health) tests
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_health_check() {
        let (state, _) = setup_state().await;
        let app = app_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(get_body_string(response).await, "OK");
    }

    #[tokio::test]
    async fn test_auth_middleware_valid_and_list() {
        let (state, prefix) = setup_state().await;
        
        let key = format!("{prefix}_list");
        let mut conn = state.redis.clone();
        let _: () = conn.set(route_key(&key), "http://list.com").await.unwrap();

        let app = app_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/routes")
                    .header("x-api-key", "test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        
        let body = get_body_string(response).await;
        assert!(body.contains(&key));
        assert!(body.contains("http://list.com"));
        
        let _: () = conn.del(route_key(&key)).await.unwrap();
    }

    #[tokio::test]
    async fn test_auth_middleware_invalid() {
        let (state, _) = setup_state().await;
        let app = app_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/routes")
                    .header("x-api-key", "wrong-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = get_body_string(response).await;
        assert!(body.contains("Unauthorized"));
    }

    #[tokio::test]
    async fn test_auth_middleware_missing() {
        let (state, _) = setup_state().await;
        let app = app_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/admin/routes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // -------------------------------------------------------------------------
    // admin.rs tests
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_admin_create_valid() {
        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_create");

        let payload = format!(r#"{{"key":"{}","value":"http://example.com"}}"#, key);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/routes")
                    .header("x-api-key", "test-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        
        let target = get_target(&state, &key).await.unwrap().unwrap();
        assert_eq!(target, "http://example.com/");
        
        let mut conn = state.redis.clone();
        let _: () = conn.del(route_key(&key)).await.unwrap();
    }

    #[tokio::test]
    async fn test_admin_create_invalid_url() {
        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_invalid");

        // Test empty key
        let payload_empty_key = r#"{"key":"","value":"http://example.com"}"#;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/routes")
                    .header("x-api-key", "test-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(payload_empty_key))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Test key containing '://'
        let payload_bad_key = r#"{"key":"http://badkey","value":"http://example.com"}"#;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/routes")
                    .header("x-api-key", "test-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(payload_bad_key))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Test completely invalid URL (unparseable)
        let payload_unparseable = format!(r#"{{"key":"{}","value":"not a valid url"}}"#, key);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/routes")
                    .header("x-api-key", "test-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(payload_unparseable))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Test non-http scheme
        let payload = format!(r#"{{"key":"{}","value":"ftp://example.com"}}"#, key);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/routes")
                    .header("x-api-key", "test-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Test missing host
        let payload2 = format!(r#"{{"key":"{}","value":"http://"}}"#, key);
        let response2 = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/routes")
                    .header("x-api-key", "test-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(payload2))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response2.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_admin_get_and_delete() {
        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_del");

        let mut conn = state.redis.clone();
        let _: () = conn.set(route_key(&key), "http://localhost").await.unwrap();

        // GET
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/admin/routes/{}", key))
                    .header("x-api-key", "test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = get_body_string(response).await;
        assert!(body.contains("http://localhost"));

        // DELETE
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/admin/routes/{}", key))
                    .header("x-api-key", "test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // GET (404)
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/admin/routes/{}", key))
                    .header("x-api-key", "test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // DELETE (404)
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/admin/routes/{}", key))
                    .header("x-api-key", "test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // -------------------------------------------------------------------------
    // proxy.rs tests
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn test_proxy_reserved_paths() {
        let (state, _) = setup_state().await;
        let app = app_router(state);

        let paths = ["/", "/admin", "/admin/foo", "/swagger", "/api-docs"];
        for path in paths {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "failed for {}", path);
        }
    }

    #[tokio::test]
    async fn test_proxy_unreserved_paths_no_route() {
        let (state, _) = setup_state().await;
        let app = app_router(state);

        let paths = ["/adminstats", "/swagger2", "/randomkey"];
        for path in paths {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            // Should be 404 because the route doesn't exist, but it should be the "route not found" 404
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            let body = get_body_string(response).await;
            assert!(body.contains("route not found"), "failed for {}", path);
        }
    }

    #[tokio::test]
    async fn test_proxy_forwarding_and_headers() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/api/v1/users"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("mocked_response")
                    .append_header("Server", "Mock")
                    .append_header("X-Custom", "Value")
            )
            .mount(&mock_server)
            .await;

        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_proxy");

        let mut conn = state.redis.clone();
        let _: () = conn.set(route_key(&key), mock_server.uri()).await.unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/{}/api/v1/users?query=1", key))
                    .header("x-api-key", "should_be_stripped")
                    .header("connection", "keep-alive")
                    .header("custom-client", "hello")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        
        // Ensure upstream headers were stripped
        assert!(response.headers().get("Server").is_none());
        assert!(response.headers().get("X-Custom").is_some());

        let body = get_body_string(response).await;
        assert_eq!(body, "mocked_response");
        
        // Check requests received by mock
        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let req = &requests[0];
        
        // Check query string
        assert_eq!(req.url.query(), Some("query=1"));
        
        // Ensure downstream headers were stripped
        assert!(req.headers.get("x-api-key").is_none());
        assert!(req.headers.get("connection").is_none());
        assert!(req.headers.get("custom-client").is_some());

        let _: () = conn.del(route_key(&key)).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_proxy_redirect_rewrite() {
        let mock_server = MockServer::start().await;
        
        let target = format!("{}/", mock_server.uri()); // Target with trailing slash
        let redirect_url = format!("{}login", target); 
        
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(302)
                    .append_header("Location", redirect_url)
            )
            .mount(&mock_server)
            .await;

        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_redir");

        let mut conn = state.redis.clone();
        let _: () = conn.set(route_key(&key), &target).await.unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/{}", key))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        
        // The location should be rewritten to hide the mock server's host
        let location = response.headers().get("location").unwrap().to_str().unwrap();
        assert_eq!(location, format!("/{}/login", key));

        let _: () = conn.del(route_key(&key)).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_proxy_redirect_external_rewrite() {
        let mock_server = MockServer::start().await;
        
        let target = format!("{}/", mock_server.uri());
        let redirect_url = "https://external.com/login".to_string(); 
        
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(302)
                    .append_header("Location", redirect_url.clone())
            )
            .mount(&mock_server)
            .await;

        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_redir_ext");

        let mut conn = state.redis.clone();
        let _: () = conn.set(route_key(&key), &target).await.unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/{}", key))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        
        // The location should NOT be rewritten because it does not match the target
        let location = response.headers().get("location").unwrap().to_str().unwrap();
        assert_eq!(location, redirect_url);

        let _: () = conn.del(route_key(&key)).await.unwrap();
    }

    #[tokio::test]
    async fn test_proxy_post_body_streaming() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("POST"))
            .and(path("/api/data"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_post");

        let mut conn = state.redis.clone();
        let _: () = conn.set(route_key(&key), mock_server.uri()).await.unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/{}/api/data", key))
                    .body(Body::from("streaming_payload_data"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        
        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let req = &requests[0];
        assert_eq!(req.body, b"streaming_payload_data");

        let _: () = conn.del(route_key(&key)).await.unwrap();
    }

    #[tokio::test]
    async fn test_proxy_upstream_unreachable_502() {
        let (state, prefix) = setup_state().await;
        let app = app_router(state.clone());
        let key = format!("{prefix}_dead");

        let mut conn = state.redis.clone();
        // Point to a guaranteed dead local port
        let _: () = conn.set(route_key(&key), "http://127.0.0.1:9999").await.unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/{}", key))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        let _: () = conn.del(route_key(&key)).await.unwrap();
    }
}
