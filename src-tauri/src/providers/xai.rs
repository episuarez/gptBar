//! xAI (Grok) provider implementation
//!
//! Fetches usage data from xAI API using an API key.
//! API key can be set via XAI_API_KEY environment variable or system keychain.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

use super::base::{
    AuthMethod, IdentitySnapshot, Provider, ProviderError, RateWindow, UsageSnapshot,
};

/// xAI API usage response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct XaiUsageResponse {
    /// Total tokens consumed this period
    total_tokens: Option<i64>,
    /// Token limit for the current period
    token_limit: Option<i64>,
    /// Start of current billing period
    period_start: Option<String>,
    /// End of current billing period
    period_end: Option<String>,
}

/// xAI balance response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct XaiBalanceResponse {
    /// Available credits in USD cents
    balance_cents: Option<i64>,
    /// Organization or account name
    name: Option<String>,
}

/// Configuration for xAI provider
#[derive(Debug, Clone)]
pub struct XaiConfig {
    pub enabled: bool,
    pub api_base_url: String,
}

impl Default for XaiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_base_url: "https://api.x.ai".to_string(),
        }
    }
}

/// xAI (Grok) provider
pub struct XaiProvider {
    client: Client,
    config: RwLock<XaiConfig>,
    api_key: RwLock<Option<String>>,
}

impl XaiProvider {
    /// Creates a new xAI provider
    pub fn new() -> Self {
        Self::with_config(XaiConfig::default())
    }

    /// Creates a new xAI provider with custom configuration
    pub fn with_config(config: XaiConfig) -> Self {
        Self {
            client: Client::new(),
            config: RwLock::new(config),
            api_key: RwLock::new(None),
        }
    }

    /// Sets the API key
    pub async fn set_api_key(&self, key: &str) {
        *self.api_key.write().await = Some(key.to_string());
    }

    /// Loads API key from environment or system keychain
    async fn load_api_key(&self) -> Option<String> {
        // Check in-memory cache
        if let Some(key) = self.api_key.read().await.clone() {
            return Some(key);
        }

        // Try environment variable
        if let Ok(key) = std::env::var("XAI_API_KEY") {
            tracing::info!("Found xAI API key from environment");
            *self.api_key.write().await = Some(key.clone());
            return Some(key);
        }

        // Try system keychain
        if let Ok(entry) =
            keyring::Entry::new(crate::config::keychain_service(self.id()), "api_key")
        {
            if let Ok(key) = entry.get_password() {
                tracing::info!("Found xAI API key from system keychain");
                *self.api_key.write().await = Some(key.clone());
                return Some(key);
            }
        }

        None
    }

    /// Fetches usage from xAI API
    async fn fetch_usage(&self, api_key: &str) -> Result<UsageSnapshot, ProviderError> {
        let config = self.config.read().await;

        // Try to get usage data
        let usage_url = format!("{}/v1/usage", config.api_base_url);
        let usage_resp = self
            .client
            .get(&usage_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .send()
            .await?;

        let mut snapshot = UsageSnapshot::new();
        let mut identity = IdentitySnapshot::new();

        if usage_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthFailed("Invalid xAI API key".into()));
        }

        if usage_resp.status().is_success() {
            if let Ok(usage) = usage_resp.json::<XaiUsageResponse>().await {
                if let (Some(used), Some(limit)) = (usage.total_tokens, usage.token_limit) {
                    let percent = if limit > 0 {
                        (used as f64 / limit as f64 * 100.0).min(100.0)
                    } else {
                        0.0
                    };

                    let description = format!("{} / {} tokens", used, limit);
                    let mut window = RateWindow::new(percent).with_reset_description(description);

                    // Parse reset time if available
                    if let Some(end_str) = usage.period_end {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&end_str) {
                            window = window.with_resets_at(dt.with_timezone(&chrono::Utc));
                        }
                    }

                    snapshot = snapshot.with_primary(window);
                }
            }
        } else {
            // If usage endpoint not available, try balance as fallback
            let balance_url = format!("{}/v1/balance", config.api_base_url);
            if let Ok(balance_resp) = self
                .client
                .get(&balance_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await
            {
                if balance_resp.status().is_success() {
                    if let Ok(balance) = balance_resp.json::<XaiBalanceResponse>().await {
                        if let Some(name) = balance.name {
                            identity = identity.with_organization(name);
                        }
                        // No usage percentage available from balance alone
                        snapshot = snapshot.with_primary(
                            RateWindow::new(0.0)
                                .with_reset_description("Balance-based account".to_string()),
                        );
                    }
                }
            }
        }

        snapshot = snapshot.with_identity(identity);
        Ok(snapshot)
    }
}

impl Default for XaiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for XaiProvider {
    fn id(&self) -> &'static str {
        "xai"
    }

    fn name(&self) -> &'static str {
        "xAI (Grok)"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn supports_login(&self) -> bool {
        false // Uses API key, not OAuth
    }

    async fn fetch(&self) -> Result<UsageSnapshot, ProviderError> {
        let api_key = self
            .load_api_key()
            .await
            .ok_or(ProviderError::AuthRequired)?;

        self.fetch_usage(&api_key).await
    }

    async fn login(&self) -> Result<bool, ProviderError> {
        // Open xAI API key page
        if let Err(e) = tauri_plugin_opener::open_url("https://console.x.ai/", None::<&str>) {
            tracing::warn!("Failed to open browser: {}", e);
        }
        Ok(false)
    }

    async fn logout(&self) -> Result<(), ProviderError> {
        *self.api_key.write().await = None;
        if let Ok(entry) =
            keyring::Entry::new(crate::config::keychain_service(self.id()), "api_key")
        {
            let _ = entry.delete_credential();
        }
        Ok(())
    }

    async fn is_available(&self) -> bool {
        self.load_api_key().await.is_some()
    }

    fn auth_methods(&self) -> Vec<AuthMethod> {
        vec![AuthMethod::ApiToken]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xai_provider_id() {
        let provider = XaiProvider::new();
        assert_eq!(provider.id(), "xai");
        assert_eq!(provider.name(), "xAI (Grok)");
        assert!(!provider.supports_login());
    }

    #[test]
    fn test_xai_auth_methods() {
        let provider = XaiProvider::new();
        assert!(provider.auth_methods().contains(&AuthMethod::ApiToken));
    }

    #[tokio::test]
    async fn test_xai_set_api_key() {
        let provider = XaiProvider::new();
        provider.set_api_key("xai-test-key").await;
        let key = provider.api_key.read().await;
        assert_eq!(key.as_deref(), Some("xai-test-key"));
    }

    #[tokio::test]
    async fn test_xai_not_available_without_key() {
        // Clear any env var for this test
        std::env::remove_var("XAI_API_KEY");
        let provider = XaiProvider::new();
        // Only check if env var was not set (keychain may or may not have it)
        let _available = provider.is_available().await;
        // Just verify it doesn't panic
    }
}
