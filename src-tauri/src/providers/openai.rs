//! OpenAI provider implementation
//!
//! Fetches usage data from OpenAI API using API key authentication.

use async_trait::async_trait;
use chrono::Datelike;
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::RwLock;

use super::base::{
    AuthMethod, IdentitySnapshot, Provider, ProviderError, RateWindow, UsageSnapshot,
};

/// OpenAI usage response (reserved for future detailed usage)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIUsageResponse {
    /// Organization info
    organization: Option<OpenAIOrganization>,
    /// Usage data
    data: Option<Vec<OpenAIUsageData>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIOrganization {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIUsageData {
    /// Total tokens used
    n_context_tokens_total: Option<i64>,
    /// Total requests
    n_requests: Option<i64>,
}

/// OpenAI billing/subscription response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAISubscription {
    /// Hard limit in USD
    hard_limit_usd: Option<f64>,
    /// Soft limit in USD
    soft_limit_usd: Option<f64>,
    /// Plan info
    plan: Option<OpenAIPlan>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAIPlan {
    title: Option<String>,
    id: Option<String>,
}

/// OpenAI billing usage response
#[derive(Debug, Deserialize)]
struct OpenAIBillingUsage {
    /// Total usage in cents
    total_usage: Option<f64>,
}

/// Configuration for OpenAI provider
#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    /// Whether this provider is enabled
    pub enabled: bool,
    /// API base URL
    pub api_base_url: String,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_base_url: "https://api.openai.com".to_string(),
        }
    }
}

/// OpenAI provider
pub struct OpenAIProvider {
    client: Client,
    config: RwLock<OpenAIConfig>,
    api_key: RwLock<Option<String>>,
    last_snapshot: RwLock<Option<UsageSnapshot>>,
}

impl OpenAIProvider {
    /// Creates a new OpenAI provider
    pub fn new() -> Self {
        Self::with_config(OpenAIConfig::default())
    }

    /// Creates a new OpenAI provider with custom configuration
    pub fn with_config(config: OpenAIConfig) -> Self {
        Self {
            client: Client::new(),
            config: RwLock::new(config),
            api_key: RwLock::new(None),
            last_snapshot: RwLock::new(None),
        }
    }

    /// Sets the API key
    pub async fn set_api_key(&self, key: &str) {
        *self.api_key.write().await = Some(key.to_string());
    }

    /// Gets the path to OpenAI credentials
    fn get_credentials_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        let home = std::env::var("USERPROFILE").ok();

        #[cfg(not(target_os = "windows"))]
        let home = std::env::var("HOME").ok();

        home.map(|h| PathBuf::from(h).join(".openai").join("credentials"))
    }

    /// Loads API key from environment or file
    async fn load_api_key(&self) -> Option<String> {
        // Check cache first
        if let Some(key) = self.api_key.read().await.clone() {
            return Some(key);
        }

        // Try environment variable
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            tracing::info!("Found OpenAI API key from environment");
            *self.api_key.write().await = Some(key.clone());
            return Some(key);
        }

        // Try credentials file
        if let Some(path) = Self::get_credentials_path() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // Simple key=value format or just the key
                    let key = content
                        .lines()
                        .find(|l| l.starts_with("OPENAI_API_KEY="))
                        .map(|l| l.trim_start_matches("OPENAI_API_KEY=").trim().to_string())
                        .or_else(|| {
                            let trimmed = content.trim();
                            if trimmed.starts_with("sk-") {
                                Some(trimmed.to_string())
                            } else {
                                None
                            }
                        });

                    if let Some(k) = key {
                        tracing::info!("Found OpenAI API key from credentials file");
                        *self.api_key.write().await = Some(k.clone());
                        return Some(k);
                    }
                }
            }
        }

        // Try system keychain
        if let Ok(entry) = keyring::Entry::new("openai", "api_key") {
            if let Ok(key) = entry.get_password() {
                tracing::info!("Found OpenAI API key from system keychain");
                *self.api_key.write().await = Some(key.clone());
                return Some(key);
            }
        }

        None
    }

    /// Fetches usage via OpenAI API
    async fn fetch_usage(&self, api_key: &str) -> Result<UsageSnapshot, ProviderError> {
        let config = self.config.read().await;

        // Fetch subscription/billing info
        let subscription_url = format!("{}/v1/dashboard/billing/subscription", config.api_base_url);

        let sub_response = self
            .client
            .get(&subscription_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        let mut snapshot = UsageSnapshot::new();
        let mut identity = IdentitySnapshot::new();

        if sub_response.status().is_success() {
            if let Ok(sub) = sub_response.json::<OpenAISubscription>().await {
                if let Some(plan) = sub.plan {
                    identity = identity.with_plan(plan.title.unwrap_or_else(|| "Free".to_string()));
                }

                // Get current month usage
                let now = chrono::Utc::now();
                let start_date = format!("{}-{:02}-01", now.year(), now.month());
                let end_date = format!(
                    "{}-{:02}-{:02}",
                    now.year(),
                    now.month(),
                    now.day() + 1
                );

                let usage_url = format!(
                    "{}/v1/dashboard/billing/usage?start_date={}&end_date={}",
                    config.api_base_url, start_date, end_date
                );

                if let Ok(usage_response) = self
                    .client
                    .get(&usage_url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .send()
                    .await
                {
                    if let Ok(usage) = usage_response.json::<OpenAIBillingUsage>().await {
                        if let (Some(used_cents), Some(limit)) =
                            (usage.total_usage, sub.hard_limit_usd)
                        {
                            let used_usd = used_cents / 100.0;
                            let percent = if limit > 0.0 {
                                (used_usd / limit * 100.0).min(100.0)
                            } else {
                                0.0
                            };

                            snapshot = snapshot.with_primary(
                                RateWindow::new(percent)
                                    .with_reset_description(format!(
                                        "${:.2} / ${:.2}",
                                        used_usd, limit
                                    )),
                            );
                        }
                    }
                }
            }
        } else {
            let status = sub_response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(ProviderError::AuthFailed("Invalid API key".into()));
            }
            return Err(ProviderError::Parse(format!("HTTP {}", status)));
        }

        snapshot = snapshot.with_identity(identity);
        Ok(snapshot)
    }
}

impl Default for OpenAIProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn id(&self) -> &'static str {
        "openai"
    }

    fn name(&self) -> &'static str {
        "OpenAI"
    }

    fn is_enabled(&self) -> bool {
        // Will be controlled by config
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

        let snapshot = self.fetch_usage(&api_key).await?;
        *self.last_snapshot.write().await = Some(snapshot.clone());
        Ok(snapshot)
    }

    async fn login(&self) -> Result<bool, ProviderError> {
        // OpenAI uses API keys, not OAuth login
        // Open the API keys page
        if let Err(e) = opener::open("https://platform.openai.com/api-keys") {
            tracing::warn!("Failed to open browser: {}", e);
        }
        Ok(false)
    }

    async fn logout(&self) -> Result<(), ProviderError> {
        *self.api_key.write().await = None;
        *self.last_snapshot.write().await = None;
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
    fn test_openai_provider_new() {
        let provider = OpenAIProvider::new();
        assert_eq!(provider.id(), "openai");
        assert_eq!(provider.name(), "OpenAI");
        assert!(!provider.supports_login());
    }

    #[test]
    fn test_openai_auth_methods() {
        let provider = OpenAIProvider::new();
        let methods = provider.auth_methods();
        assert!(methods.contains(&AuthMethod::ApiToken));
    }

    #[tokio::test]
    async fn test_openai_set_api_key() {
        let provider = OpenAIProvider::new();
        provider.set_api_key("sk-test-key").await;

        let key = provider.api_key.read().await;
        assert_eq!(key.as_ref().map(|s| s.as_str()), Some("sk-test-key"));
    }
}
