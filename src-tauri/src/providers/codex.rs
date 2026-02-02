//! Codex CLI provider implementation
//!
//! Monitors usage for OpenAI's Codex CLI tool.
//! Codex uses the same API as OpenAI but with separate credentials.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::RwLock;

use super::base::{
    AuthMethod, IdentitySnapshot, Provider, ProviderError, RateWindow, UsageSnapshot,
};

/// Codex config response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CodexConfig {
    /// API key
    api_key: Option<String>,
    /// Model preference
    model: Option<String>,
}

/// Configuration for Codex provider
#[derive(Debug, Clone)]
pub struct CodexProviderConfig {
    /// Whether this provider is enabled
    pub enabled: bool,
    /// API base URL (same as OpenAI)
    pub api_base_url: String,
}

impl Default for CodexProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_base_url: "https://api.openai.com".to_string(),
        }
    }
}

/// Codex CLI provider
///
/// Codex CLI (https://github.com/openai/codex) uses OpenAI's API
/// but stores credentials separately.
pub struct CodexProvider {
    client: Client,
    config: RwLock<CodexProviderConfig>,
    api_key: RwLock<Option<String>>,
    last_snapshot: RwLock<Option<UsageSnapshot>>,
}

impl CodexProvider {
    /// Creates a new Codex provider
    pub fn new() -> Self {
        Self::with_config(CodexProviderConfig::default())
    }

    /// Creates a new Codex provider with custom configuration
    pub fn with_config(config: CodexProviderConfig) -> Self {
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

    /// Gets the path to Codex config directory
    fn get_codex_config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("APPDATA")
                .ok()
                .map(|p| PathBuf::from(p).join("codex"))
        }

        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .ok()
                .map(|p| PathBuf::from(p).join("Library/Application Support/codex"))
        }

        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| std::env::var("HOME").ok().map(|p| PathBuf::from(p).join(".config")))
                .map(|p| p.join("codex"))
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }

    /// Loads API key from Codex CLI config or environment
    async fn load_api_key(&self) -> Option<String> {
        // Check cache first
        if let Some(key) = self.api_key.read().await.clone() {
            return Some(key);
        }

        // Try Codex-specific environment variable
        if let Ok(key) = std::env::var("CODEX_API_KEY") {
            tracing::info!("Found Codex API key from CODEX_API_KEY");
            *self.api_key.write().await = Some(key.clone());
            return Some(key);
        }

        // Try Codex config file
        if let Some(config_dir) = Self::get_codex_config_dir() {
            let config_path = config_dir.join("config.json");
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(config) = serde_json::from_str::<CodexConfig>(&content) {
                        if let Some(key) = config.api_key {
                            tracing::info!("Found Codex API key from config file");
                            *self.api_key.write().await = Some(key.clone());
                            return Some(key);
                        }
                    }
                }
            }

            // Also check for .env file in codex dir
            let env_path = config_dir.join(".env");
            if env_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&env_path) {
                    for line in content.lines() {
                        if let Some(key) = line.strip_prefix("OPENAI_API_KEY=") {
                            let key = key.trim().trim_matches('"').trim_matches('\'');
                            tracing::info!("Found Codex API key from .env file");
                            *self.api_key.write().await = Some(key.to_string());
                            return Some(key.to_string());
                        }
                    }
                }
            }
        }

        // Try system keychain
        if let Ok(entry) = keyring::Entry::new("codex-cli", "api_key") {
            if let Ok(key) = entry.get_password() {
                tracing::info!("Found Codex API key from system keychain");
                *self.api_key.write().await = Some(key.clone());
                return Some(key);
            }
        }

        // Fall back to OpenAI key as Codex uses OpenAI API
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            tracing::info!("Using OpenAI API key for Codex");
            *self.api_key.write().await = Some(key.clone());
            return Some(key);
        }

        None
    }

    /// Fetches usage via OpenAI API (same endpoint as OpenAI provider)
    async fn fetch_usage(&self, api_key: &str) -> Result<UsageSnapshot, ProviderError> {
        let config = self.config.read().await;

        // Verify API key works by making a simple models request
        let models_url = format!("{}/v1/models", config.api_base_url);

        let response = self
            .client
            .get(&models_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthFailed("Invalid API key".into()));
        }

        if !status.is_success() {
            return Err(ProviderError::Parse(format!("HTTP {}", status)));
        }

        let mut snapshot = UsageSnapshot::new();

        // Codex uses OpenAI's API, so we show it's connected
        let identity = IdentitySnapshot::new().with_plan("Connected");

        snapshot = snapshot
            .with_primary(
                RateWindow::new(0.0).with_reset_description("Uses OpenAI API"),
            )
            .with_identity(identity);

        Ok(snapshot)
    }
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for CodexProvider {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
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
        // Open Codex CLI docs or OpenAI API keys page
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
    fn test_codex_provider_new() {
        let provider = CodexProvider::new();
        assert_eq!(provider.id(), "codex");
        assert_eq!(provider.name(), "Codex");
        assert!(!provider.supports_login());
    }

    #[test]
    fn test_codex_auth_methods() {
        let provider = CodexProvider::new();
        let methods = provider.auth_methods();
        assert!(methods.contains(&AuthMethod::ApiToken));
    }

    #[tokio::test]
    async fn test_codex_set_api_key() {
        let provider = CodexProvider::new();
        provider.set_api_key("sk-test-key").await;

        let key = provider.api_key.read().await;
        assert_eq!(key.as_ref().map(|s| s.as_str()), Some("sk-test-key"));
    }

    #[test]
    fn test_codex_config_dir() {
        let dir = CodexProvider::get_codex_config_dir();
        // Should return Some on supported platforms
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        assert!(dir.is_some());
    }
}
