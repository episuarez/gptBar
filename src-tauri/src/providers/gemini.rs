//! Google Gemini provider implementation
//!
//! Fetches usage/quota data from Google AI API.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::RwLock;

use super::base::{
    AuthMethod, IdentitySnapshot, Provider, ProviderError, RateWindow, UsageSnapshot,
};

/// Gemini models list response
#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModel>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiModel {
    name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

/// Gemini quota response (reserved for future quota tracking)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiQuotaResponse {
    /// Quota metrics
    metrics: Option<Vec<GeminiQuotaMetric>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiQuotaMetric {
    metric: Option<String>,
    limit: Option<i64>,
    usage: Option<i64>,
}

/// Configuration for Gemini provider
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    /// Whether this provider is enabled
    pub enabled: bool,
    /// API base URL
    pub api_base_url: String,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_base_url: "https://generativelanguage.googleapis.com".to_string(),
        }
    }
}

/// Google Gemini provider
pub struct GeminiProvider {
    client: Client,
    config: RwLock<GeminiConfig>,
    api_key: RwLock<Option<String>>,
    last_snapshot: RwLock<Option<UsageSnapshot>>,
}

impl GeminiProvider {
    /// Creates a new Gemini provider
    pub fn new() -> Self {
        Self::with_config(GeminiConfig::default())
    }

    /// Creates a new Gemini provider with custom configuration
    pub fn with_config(config: GeminiConfig) -> Self {
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

    /// Gets the path to Google credentials (reserved for future ADC support)
    #[allow(dead_code)]
    fn get_credentials_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        let home = std::env::var("USERPROFILE").ok();

        #[cfg(not(target_os = "windows"))]
        let home = std::env::var("HOME").ok();

        // Check for application default credentials
        home.map(|h| {
            PathBuf::from(h)
                .join(".config")
                .join("gcloud")
                .join("application_default_credentials.json")
        })
    }

    /// Loads API key from environment or file
    async fn load_api_key(&self) -> Option<String> {
        // Check cache first
        if let Some(key) = self.api_key.read().await.clone() {
            return Some(key);
        }

        // Try environment variables
        for var in ["GOOGLE_API_KEY", "GEMINI_API_KEY"] {
            if let Ok(key) = std::env::var(var) {
                tracing::info!("Found Gemini API key from {}", var);
                *self.api_key.write().await = Some(key.clone());
                return Some(key);
            }
        }

        // Try system keychain
        if let Ok(entry) = keyring::Entry::new("google-gemini", "api_key") {
            if let Ok(key) = entry.get_password() {
                tracing::info!("Found Gemini API key from system keychain");
                *self.api_key.write().await = Some(key.clone());
                return Some(key);
            }
        }

        None
    }

    /// Fetches usage/availability via Gemini API
    async fn fetch_usage(&self, api_key: &str) -> Result<UsageSnapshot, ProviderError> {
        let config = self.config.read().await;

        // Test API access by listing models
        let models_url = format!("{}/v1beta/models?key={}", config.api_base_url, api_key);

        let response = self.client.get(&models_url).send().await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            return Err(ProviderError::AuthFailed("Invalid API key".into()));
        }

        if !status.is_success() {
            return Err(ProviderError::Parse(format!("HTTP {}", status)));
        }

        let models: GeminiModelsResponse = response.json().await.map_err(|e| {
            ProviderError::Parse(format!("Failed to parse models response: {}", e))
        })?;

        let mut snapshot = UsageSnapshot::new();

        // Gemini free tier has rate limits but no easy way to query current usage
        // We'll show availability status instead
        let model_count = models.models.as_ref().map(|m| m.len()).unwrap_or(0);

        let identity = IdentitySnapshot::new()
            .with_plan(if model_count > 0 { "Active" } else { "Unknown" });

        // Create a simple status indicator
        // Note: Gemini doesn't expose usage quotas via API like OpenAI does
        // We can only verify the key works
        snapshot = snapshot
            .with_primary(
                RateWindow::new(0.0).with_reset_description(format!("{} models available", model_count)),
            )
            .with_identity(identity);

        Ok(snapshot)
    }
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn name(&self) -> &'static str {
        "Gemini"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn supports_login(&self) -> bool {
        false // Uses API key
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
        // Gemini uses API keys
        if let Err(e) = opener::open("https://aistudio.google.com/app/apikey") {
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
    fn test_gemini_provider_new() {
        let provider = GeminiProvider::new();
        assert_eq!(provider.id(), "gemini");
        assert_eq!(provider.name(), "Gemini");
        assert!(!provider.supports_login());
    }

    #[test]
    fn test_gemini_auth_methods() {
        let provider = GeminiProvider::new();
        let methods = provider.auth_methods();
        assert!(methods.contains(&AuthMethod::ApiToken));
    }

    #[tokio::test]
    async fn test_gemini_set_api_key() {
        let provider = GeminiProvider::new();
        provider.set_api_key("test-api-key").await;

        let key = provider.api_key.read().await;
        assert_eq!(key.as_ref().map(|s| s.as_str()), Some("test-api-key"));
    }
}
