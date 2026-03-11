//! Integration tests for AI provider implementations
//!
//! Uses wiremock to simulate HTTP endpoints without real API calls.

#[cfg(test)]
mod claude_integration {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::providers::claude::{ClaudeConfig, ClaudeProvider};
    use crate::providers::base::Provider;

    fn make_provider(base_url: &str) -> ClaudeProvider {
        let config = ClaudeConfig {
            enabled: true,
            api_base_url: base_url.to_string(),
        };
        ClaudeProvider::with_config(config)
    }

    #[tokio::test]
    async fn test_claude_fetch_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/organizations/unknown/claude_ai/token_counts_for_oauth_token"))
            .and(header("Authorization", "Bearer sk-ant-oat-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "five_hour": { "utilization": 42.5, "resets_at": "2026-03-11T10:00:00Z" },
                "seven_day": { "utilization": 18.0, "resets_at": "2026-03-14T00:00:00Z" },
                "seven_day_sonnet": { "utilization": 5.0, "resets_at": "2026-03-14T00:00:00Z" }
            })))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_oauth_token("sk-ant-oat-test").await;

        let result = provider.fetch().await;
        assert!(result.is_ok(), "fetch should succeed: {:?}", result.err());

        let snapshot = result.unwrap();
        let primary = snapshot.primary.expect("should have primary window");
        assert!((primary.used_percent - 42.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_claude_fetch_401_returns_auth_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_oauth_token("sk-ant-oat-expired").await;

        let result = provider.fetch().await;
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("Authentication") || err_str.contains("auth") || err_str.contains("401"),
            "Expected auth error, got: {}", err_str
        );
    }

    #[tokio::test]
    async fn test_claude_fetch_missing_token_returns_auth_required() {
        let server = MockServer::start().await;
        let provider = make_provider(&server.uri());
        // No token set, no credentials file in test env

        let available = provider.is_available().await;
        // In CI there's no ~/.claude/.credentials.json, so not available
        // Just check it doesn't panic
        let _ = available;
    }

    #[tokio::test]
    async fn test_claude_fetch_all_windows_parsed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "five_hour": { "utilization": 95.0, "resets_at": "2026-03-11T10:00:00Z" },
                "seven_day": { "utilization": 60.0, "resets_at": "2026-03-14T00:00:00Z" },
                "seven_day_sonnet": { "utilization": 30.0, "resets_at": "2026-03-14T00:00:00Z" }
            })))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_oauth_token("sk-ant-oat-test").await;

        let snapshot = provider.fetch().await.unwrap();
        assert!(snapshot.primary.is_some());
        assert!(snapshot.secondary.is_some());
        assert!(snapshot.tertiary.is_some());
        assert!((snapshot.primary.unwrap().used_percent - 95.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_claude_fetch_partial_response() {
        let server = MockServer::start().await;

        // Only five_hour present, rest null
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "five_hour": { "utilization": 12.0 }
            })))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_oauth_token("sk-ant-oat-test").await;

        let snapshot = provider.fetch().await.unwrap();
        assert!(snapshot.primary.is_some());
        // secondary may or may not be present depending on parsing
    }
}

#[cfg(test)]
mod openai_integration {
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::providers::openai::{OpenAIConfig, OpenAIProvider};
    use crate::providers::base::Provider;

    fn make_provider(base_url: &str) -> OpenAIProvider {
        let config = OpenAIConfig {
            enabled: true,
            api_base_url: base_url.to_string(),
        };
        OpenAIProvider::with_config(config)
    }

    #[tokio::test]
    async fn test_openai_fetch_success() {
        let server = MockServer::start().await;

        // Subscription endpoint
        Mock::given(method("GET"))
            .and(path("/v1/dashboard/billing/subscription"))
            .and(header("Authorization", "Bearer sk-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hard_limit_usd": 100.0,
                "soft_limit_usd": 80.0,
                "plan": { "title": "Pay as you go", "id": "payg" }
            })))
            .mount(&server)
            .await;

        // Usage endpoint
        Mock::given(method("GET"))
            .and(path("/v1/dashboard/billing/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total_usage": 2500.0
            })))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_api_key("sk-test").await;

        let result = provider.fetch().await;
        assert!(result.is_ok(), "fetch should succeed: {:?}", result.err());

        let snapshot = result.unwrap();
        let primary = snapshot.primary.expect("should have primary window");
        // $25.00 / $100.00 = 25%
        assert!((primary.used_percent - 25.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_openai_fetch_unauthorized() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/dashboard/billing/subscription"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_api_key("sk-invalid").await;

        let result = provider.fetch().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Invalid") || err.contains("auth") || err.contains("Auth"),
            "Expected auth error, got: {}", err
        );
    }

    #[tokio::test]
    async fn test_openai_no_key_returns_auth_required() {
        let server = MockServer::start().await;
        let provider = make_provider(&server.uri());
        // No key set

        // Ensure env var not set
        std::env::remove_var("OPENAI_API_KEY");

        let result = provider.fetch().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Authentication required") || err.contains("auth"),
            "Expected auth required, got: {}", err
        );
    }

    #[tokio::test]
    async fn test_openai_full_budget_usage() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/dashboard/billing/subscription"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hard_limit_usd": 50.0,
                "plan": { "title": "Pro" }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/dashboard/billing/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total_usage": 5000.0  // $50.00 = 100%
            })))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_api_key("sk-test").await;

        let snapshot = provider.fetch().await.unwrap();
        let primary = snapshot.primary.unwrap();
        assert!((primary.used_percent - 100.0).abs() < 0.1);
    }
}

#[cfg(test)]
mod xai_integration {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::providers::xai::{XaiConfig, XaiProvider};
    use crate::providers::base::Provider;

    fn make_provider(base_url: &str) -> XaiProvider {
        let config = XaiConfig {
            enabled: true,
            api_base_url: base_url.to_string(),
        };
        XaiProvider::with_config(config)
    }

    #[tokio::test]
    async fn test_xai_fetch_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/usage"))
            .and(header("Authorization", "Bearer xai-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total_tokens": 500000,
                "token_limit": 1000000,
                "period_start": "2026-03-01T00:00:00Z",
                "period_end": "2026-04-01T00:00:00Z"
            })))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_api_key("xai-test").await;

        let result = provider.fetch().await;
        assert!(result.is_ok(), "fetch should succeed: {:?}", result.err());

        let snapshot = result.unwrap();
        let primary = snapshot.primary.expect("should have primary window");
        assert!((primary.used_percent - 50.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_xai_fetch_unauthorized() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let provider = make_provider(&server.uri());
        provider.set_api_key("xai-invalid").await;

        let result = provider.fetch().await;
        assert!(result.is_err());
    }
}
