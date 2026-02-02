//! Claude AI provider implementation
//!
//! Uses the OAuth token from Claude Code CLI to fetch usage data.
//! No browser cookie extraction - reads from Claude Code's stored credentials.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::RwLock;

use super::base::{AuthMethod, Provider, ProviderError, RateWindow, UsageSnapshot};

/// Claude OAuth usage API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeOAuthUsageResponse {
    /// 5-hour session limit
    five_hour: Option<ClaudeUsageMetrics>,
    /// 7-day overall limit
    seven_day: Option<ClaudeUsageMetrics>,
    /// 7-day Sonnet limit
    seven_day_sonnet: Option<ClaudeUsageMetrics>,
    /// Extra usage (Max plan feature)
    extra_usage: Option<ClaudeExtraUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsageMetrics {
    /// Utilization percentage (0-100)
    utilization: Option<f64>,
    /// Resets at timestamp (RFC3339 format)
    resets_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeExtraUsage {
    /// Whether extra usage is enabled
    is_enabled: bool,
    /// Monthly limit in dollars
    monthly_limit: Option<f64>,
    /// Used credits in dollars
    used_credits: Option<f64>,
    /// Utilization percentage
    utilization: Option<f64>,
}

/// Claude Code credentials file format
#[derive(Debug, Deserialize)]
struct ClaudeCodeCredentials {
    /// OAuth access token
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeAiOAuthCredential>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeAiOAuthCredential {
    /// Access token (sk-ant-oat-...)
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    /// Refresh token
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
    /// Expiry timestamp
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

/// Configuration for Claude provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConfig {
    /// Whether this provider is enabled
    pub enabled: bool,
    /// OAuth API base URL
    pub api_base_url: String,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_base_url: "https://api.anthropic.com".to_string(),
        }
    }
}

/// Claude AI provider
///
/// Reads OAuth token from Claude Code CLI credentials and fetches usage data
/// from the Anthropic API.
pub struct ClaudeProvider {
    client: Client,
    config: RwLock<ClaudeConfig>,
    last_snapshot: RwLock<Option<UsageSnapshot>>,
    oauth_token: RwLock<Option<String>>,
}

impl ClaudeProvider {
    /// Creates a new ClaudeProvider with default configuration
    pub fn new() -> Self {
        Self::with_config(ClaudeConfig::default())
    }

    /// Creates a new ClaudeProvider with custom configuration
    pub fn with_config(config: ClaudeConfig) -> Self {
        Self {
            client: Client::new(),
            config: RwLock::new(config),
            last_snapshot: RwLock::new(None),
            oauth_token: RwLock::new(None),
        }
    }

    /// Creates a provider with custom base URL (for testing)
    pub fn new_with_base_url(base_url: &str) -> Self {
        let config = ClaudeConfig {
            api_base_url: base_url.to_string(),
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Sets the OAuth token manually (for testing)
    pub async fn set_oauth_token(&self, token: &str) {
        *self.oauth_token.write().await = Some(token.to_string());
    }

    /// Gets the path to Claude Code credentials file (cross-platform)
    fn get_credentials_path() -> Option<PathBuf> {
        // Windows: %USERPROFILE%\.claude\.credentials.json
        // macOS/Linux: ~/.claude/.credentials.json
        #[cfg(target_os = "windows")]
        let home = std::env::var("USERPROFILE").ok();

        #[cfg(not(target_os = "windows"))]
        let home = std::env::var("HOME").ok();

        home.map(|h| PathBuf::from(h).join(".claude").join(".credentials.json"))
    }

    /// Loads OAuth token from Claude Code CLI credentials
    async fn load_oauth_token(&self) -> Option<String> {
        // First check in-memory cache
        if let Some(token) = self.oauth_token.read().await.clone() {
            tracing::debug!("Using cached OAuth token");
            return Some(token);
        }

        // Try to read from Claude Code credentials file
        if let Some(path) = Self::get_credentials_path() {
            tracing::info!("Looking for credentials at: {:?}", path);

            if path.exists() {
                tracing::info!("Credentials file exists, reading...");

                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        tracing::debug!("Read {} bytes from credentials file", content.len());
                        match serde_json::from_str::<ClaudeCodeCredentials>(&content) {
                            Ok(creds) => {
                                if let Some(oauth) = creds.claude_ai_oauth {
                                    if let Some(token) = oauth.access_token {
                                        tracing::info!("Found Claude Code OAuth token ({}...)", &token[..20.min(token.len())]);
                                        *self.oauth_token.write().await = Some(token.clone());
                                        return Some(token);
                                    } else {
                                        tracing::warn!("No access_token in credentials");
                                    }
                                } else {
                                    tracing::warn!("No claudeAiOauth in credentials");
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse credentials JSON: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to read credentials file: {}", e);
                    }
                }
            } else {
                tracing::warn!("Credentials file does not exist at: {:?}", path);
            }
        } else {
            tracing::error!("Could not determine credentials path (USERPROFILE/HOME not set)");
        }

        // Try system keychain with Claude Code's service name
        if let Ok(entry) = keyring::Entry::new("Claude Code-credentials", "default") {
            if let Ok(token) = entry.get_password() {
                // The credential might be JSON, try to parse it
                if let Ok(creds) = serde_json::from_str::<ClaudeCodeCredentials>(&token) {
                    if let Some(oauth) = creds.claude_ai_oauth {
                        if let Some(access_token) = oauth.access_token {
                            tracing::info!("Found Claude Code OAuth token from system keychain");
                            *self.oauth_token.write().await = Some(access_token.clone());
                            return Some(access_token);
                        }
                    }
                } else {
                    // Maybe it's just the token directly
                    if token.starts_with("sk-ant-") {
                        tracing::info!("Found Claude Code OAuth token from system keychain");
                        *self.oauth_token.write().await = Some(token.clone());
                        return Some(token);
                    }
                }
            }
        }

        tracing::warn!("No Claude Code OAuth token found");
        None
    }

    /// Fetches usage via OAuth API
    async fn fetch_via_oauth(&self, token: &str) -> Result<UsageSnapshot, ProviderError> {
        let config = self.config.read().await;
        let url = format!("{}/api/oauth/usage", config.api_base_url);

        tracing::debug!("Fetching usage from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("Content-Type", "application/json")
            .send()
            .await?;

        let status = response.status();
        tracing::debug!("Response status: {}", status);

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthFailed("OAuth token expired or invalid".into()));
        }

        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthFailed(
                "Token doesn't have user:profile scope. CLI tokens with only user:inference cannot call usage endpoint.".into()
            ));
        }

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            tracing::warn!("OAuth usage request failed: {} - {}", status, text);
            return Err(ProviderError::Parse(format!("HTTP {}: {}", status, text)));
        }

        let data: ClaudeOAuthUsageResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(format!("Failed to parse usage response: {}", e)))?;

        self.parse_oauth_usage(data)
    }

    /// Parses OAuth usage response into UsageSnapshot
    fn parse_oauth_usage(&self, data: ClaudeOAuthUsageResponse) -> Result<UsageSnapshot, ProviderError> {
        let mut snapshot = UsageSnapshot::new();

        // 5-hour session limit (primary)
        if let Some(five_hour) = data.five_hour {
            if let Some(pct) = five_hour.utilization {
                let mut window = RateWindow::new(pct)
                    .with_window_minutes(300) // 5 hours
                    .with_reset_description("5h session limit");

                if let Some(resets_str) = five_hour.resets_at {
                    if let Ok(resets) = chrono::DateTime::parse_from_rfc3339(&resets_str) {
                        window = window.with_resets_at(resets.with_timezone(&chrono::Utc));
                    }
                }
                snapshot = snapshot.with_primary(window);
            }
        }

        // 7-day overall limit (secondary)
        if let Some(seven_day) = data.seven_day {
            if let Some(pct) = seven_day.utilization {
                let mut window = RateWindow::new(pct)
                    .with_window_minutes(10080) // 7 days
                    .with_reset_description("Weekly limit");

                if let Some(resets_str) = seven_day.resets_at {
                    if let Ok(resets) = chrono::DateTime::parse_from_rfc3339(&resets_str) {
                        window = window.with_resets_at(resets.with_timezone(&chrono::Utc));
                    }
                }
                snapshot = snapshot.with_secondary(window);
            }
        }

        // 7-day Sonnet limit (tertiary) - optional model-specific limit
        if let Some(sonnet) = data.seven_day_sonnet {
            if let Some(pct) = sonnet.utilization {
                let mut window = RateWindow::new(pct)
                    .with_reset_description("Sonnet limit");

                if let Some(resets_str) = sonnet.resets_at {
                    if let Ok(resets) = chrono::DateTime::parse_from_rfc3339(&resets_str) {
                        window = window.with_resets_at(resets.with_timezone(&chrono::Utc));
                    }
                }
                snapshot = snapshot.with_tertiary(window);
            }
        }

        Ok(snapshot)
    }

    /// Reloads OAuth token from Claude Code credentials
    pub async fn reload_token(&self) -> Result<bool, ProviderError> {
        tracing::info!("Reloading OAuth token from Claude Code...");

        // Clear cached token
        *self.oauth_token.write().await = None;

        // Try to load again
        if self.load_oauth_token().await.is_some() {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Default for ClaudeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for ClaudeProvider {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn name(&self) -> &'static str {
        "Claude"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn supports_login(&self) -> bool {
        true
    }

    async fn fetch(&self) -> Result<UsageSnapshot, ProviderError> {
        // Try OAuth token from Claude Code
        if let Some(token) = self.load_oauth_token().await {
            match self.fetch_via_oauth(&token).await {
                Ok(snapshot) => {
                    *self.last_snapshot.write().await = Some(snapshot.clone());
                    return Ok(snapshot);
                }
                Err(ProviderError::AuthFailed(msg)) => {
                    tracing::warn!("OAuth auth failed: {}", msg);
                    // Clear invalid token
                    *self.oauth_token.write().await = None;
                }
                Err(e) => {
                    tracing::warn!("OAuth fetch failed: {}", e);
                    return Err(e);
                }
            }
        }

        Err(ProviderError::AuthRequired)
    }

    async fn login(&self) -> Result<bool, ProviderError> {
        tracing::info!("Claude login requested");

        // Open Claude Code login page or instructions
        // The user needs to run `claude login` in their terminal
        if let Err(e) = opener::open("https://claude.ai/login") {
            tracing::warn!("Failed to open browser: {}", e);
        }

        // Return false - user needs to login via Claude Code CLI
        // then click reload in GPTBar
        Ok(false)
    }

    async fn logout(&self) -> Result<(), ProviderError> {
        // Clear cached token
        *self.oauth_token.write().await = None;
        *self.last_snapshot.write().await = None;

        tracing::info!("Cleared cached OAuth token. Note: This doesn't logout from Claude Code CLI.");
        Ok(())
    }

    async fn is_available(&self) -> bool {
        self.load_oauth_token().await.is_some()
    }

    fn auth_methods(&self) -> Vec<AuthMethod> {
        vec![AuthMethod::OAuth]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_config_default() {
        let config = ClaudeConfig::default();
        assert!(config.enabled);
        assert!(config.api_base_url.contains("anthropic.com"));
    }

    #[test]
    fn test_claude_provider_new() {
        let provider = ClaudeProvider::new();
        assert_eq!(provider.id(), "claude");
        assert_eq!(provider.name(), "Claude");
        assert!(provider.supports_login());
    }

    #[test]
    fn test_claude_provider_auth_methods() {
        let provider = ClaudeProvider::new();
        let methods = provider.auth_methods();
        assert!(methods.contains(&AuthMethod::OAuth));
        assert!(!methods.contains(&AuthMethod::Cookie));
    }

    #[tokio::test]
    async fn test_claude_provider_set_oauth_token() {
        let provider = ClaudeProvider::new();
        provider.set_oauth_token("test-token").await;

        let token = provider.oauth_token.read().await;
        assert_eq!(token.as_ref().map(|s| s.as_str()), Some("test-token"));
    }

    #[test]
    fn test_get_credentials_path() {
        let path = ClaudeProvider::get_credentials_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains(".claude"));
        assert!(path.to_string_lossy().contains(".credentials.json"));
    }

    #[test]
    fn test_parse_oauth_usage() {
        let provider = ClaudeProvider::new();

        let data = ClaudeOAuthUsageResponse {
            five_hour: Some(ClaudeUsageMetrics {
                utilization: Some(45.5),
                resets_at: Some("2024-01-15T10:00:00Z".to_string()),
            }),
            seven_day: Some(ClaudeUsageMetrics {
                utilization: Some(12.0),
                resets_at: None,
            }),
            seven_day_sonnet: Some(ClaudeUsageMetrics {
                utilization: Some(17.0),
                resets_at: None,
            }),
            extra_usage: None,
        };

        let snapshot = provider.parse_oauth_usage(data).unwrap();

        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 45.5);

        assert!(snapshot.secondary.is_some());
        assert_eq!(snapshot.secondary.as_ref().unwrap().used_percent, 12.0);

        assert!(snapshot.tertiary.is_some());
        assert_eq!(snapshot.tertiary.as_ref().unwrap().used_percent, 17.0);
    }

    #[test]
    fn test_parse_oauth_usage_partial() {
        let provider = ClaudeProvider::new();

        let data = ClaudeOAuthUsageResponse {
            five_hour: Some(ClaudeUsageMetrics {
                utilization: Some(50.0),
                resets_at: None,
            }),
            seven_day: None,
            seven_day_sonnet: None,
            extra_usage: None,
        };

        let snapshot = provider.parse_oauth_usage(data).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 50.0);
        assert!(snapshot.secondary.is_none());
    }

    #[tokio::test]
    async fn test_claude_provider_logout() {
        let provider = ClaudeProvider::new();

        // Set some data
        provider.set_oauth_token("token").await;

        // Logout
        provider.logout().await.unwrap();

        // Verify cleared
        assert!(provider.oauth_token.read().await.is_none());
    }

    #[test]
    fn test_read_credentials_file() {
        if let Some(path) = ClaudeProvider::get_credentials_path() {
            println!("Credentials path: {:?}", path);
            println!("Path exists: {}", path.exists());

            if path.exists() {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        println!("Content length: {} bytes", content.len());
                        match serde_json::from_str::<ClaudeCodeCredentials>(&content) {
                            Ok(creds) => {
                                println!("Parsed credentials successfully");
                                if let Some(oauth) = &creds.claude_ai_oauth {
                                    println!("Has claudeAiOauth");
                                    if let Some(token) = &oauth.access_token {
                                        println!("Has access_token: {}...", &token[..20.min(token.len())]);
                                        assert!(token.starts_with("sk-ant-"));
                                    } else {
                                        println!("No access_token found");
                                    }
                                } else {
                                    println!("No claudeAiOauth found in parsed JSON");
                                }
                            }
                            Err(e) => {
                                println!("Failed to parse JSON: {}", e);
                                println!("Content preview: {}", &content[..100.min(content.len())]);
                            }
                        }
                    }
                    Err(e) => {
                        println!("Failed to read file: {}", e);
                    }
                }
            } else {
                println!("Credentials file does not exist - skipping test");
            }
        } else {
            println!("Could not get credentials path");
        }
    }

    #[tokio::test]
    async fn test_load_oauth_token() {
        let provider = ClaudeProvider::new();
        let token = provider.load_oauth_token().await;

        if let Some(t) = &token {
            println!("Found token: {}...", &t[..20.min(t.len())]);
            assert!(t.starts_with("sk-ant-"));
        } else {
            println!("No token found - this test passes if no Claude Code credentials exist");
        }
    }
}
