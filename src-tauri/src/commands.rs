//! Tauri IPC Commands
//!
//! All commands that can be called from the frontend via Tauri IPC.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::{AppConfig, ProviderSettings};
use crate::providers::{Provider, ProviderMetadata, UsageSnapshot};
use crate::AppState;

/// Fetches usage data from Claude
#[tauri::command]
pub async fn fetch_usage(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<UsageSnapshot, String> {
    let state = state.read().await;
    state.claude.fetch().await.map_err(|e| e.to_string())
}

/// Gets the cached usage snapshot for Claude
#[tauri::command]
pub async fn get_cached_usage(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<Option<UsageSnapshot>, String> {
    let state = state.read().await;
    if let Some(_agent) = state.agent_manager.get("refresh").await {
        // Downcast to RefreshAgent would be needed here
        // For now, return None
    }
    Ok(None)
}

/// Checks if Claude authentication is available
#[tauri::command]
pub async fn is_claude_available(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<bool, String> {
    let state = state.read().await;
    Ok(state.claude.is_available().await)
}

/// Initiates Claude login
#[tauri::command]
pub async fn login_claude(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<bool, String> {
    let state = state.read().await;
    state.claude.login().await.map_err(|e| e.to_string())
}

/// Logs out from Claude
#[tauri::command]
pub async fn logout_claude(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    let state = state.read().await;
    state.claude.logout().await.map_err(|e| e.to_string())
}

/// Reloads OAuth token from Claude Code CLI credentials
#[tauri::command]
pub async fn reload_token(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<bool, String> {
    let state = state.read().await;
    state.claude.reload_token().await.map_err(|e| e.to_string())
}

/// Triggers an immediate refresh of usage data
#[tauri::command]
pub async fn trigger_refresh(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    let state = state.read().await;
    state
        .agent_manager
        .trigger_agent("refresh")
        .await
        .map_err(|e| e.to_string())
}

/// Gets the status of all agents
#[tauri::command]
pub async fn get_agent_status(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<Vec<(String, String)>, String> {
    let state = state.read().await;
    let status = state.agent_manager.status().await;
    Ok(status
        .into_iter()
        .map(|(id, s)| (id.to_string(), format!("{:?}", s)))
        .collect())
}

// ============================================================================
// Configuration Commands
// ============================================================================

/// Gets the current configuration
#[tauri::command]
pub fn get_config() -> Result<AppConfig, String> {
    Ok(AppConfig::load())
}

/// Saves the configuration
#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    config.save()?;
    config.set_autostart()?;
    Ok(())
}

/// Sets the refresh interval
#[tauri::command]
pub fn set_refresh_interval(minutes: u32) -> Result<(), String> {
    let mut config = AppConfig::load();
    config.refresh_interval = minutes;
    config.save()
}

/// Sets whether to start on login
#[tauri::command]
pub fn set_start_on_login(enabled: bool) -> Result<(), String> {
    let mut config = AppConfig::load();
    config.start_on_login = enabled;
    config.save()?;
    config.set_autostart()
}

/// Checks if autostart is currently enabled
#[tauri::command]
pub fn is_autostart_enabled() -> bool {
    AppConfig::is_autostart_enabled()
}

// ============================================================================
// Generic Provider Commands
// ============================================================================

/// Fetches usage data from a specific provider
#[tauri::command]
pub async fn fetch_provider_usage(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
    provider_id: String,
) -> Result<UsageSnapshot, String> {
    let state = state.read().await;

    match provider_id.as_str() {
        "claude" => state.claude.fetch().await.map_err(|e| e.to_string()),
        "openai" => state.openai.fetch().await.map_err(|e| e.to_string()),
        "gemini" => state.gemini.fetch().await.map_err(|e| e.to_string()),
        "codex" => state.codex.fetch().await.map_err(|e| e.to_string()),
        _ => Err(format!("Unknown provider: {}", provider_id)),
    }
}

/// Checks if a provider's authentication is available
#[tauri::command]
pub async fn is_provider_available(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
    provider_id: String,
) -> Result<bool, String> {
    let state = state.read().await;

    match provider_id.as_str() {
        "claude" => Ok(state.claude.is_available().await),
        "openai" => Ok(state.openai.is_available().await),
        "gemini" => Ok(state.gemini.is_available().await),
        "codex" => Ok(state.codex.is_available().await),
        _ => Err(format!("Unknown provider: {}", provider_id)),
    }
}

/// Initiates login for a provider
#[tauri::command]
pub async fn login_provider(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
    provider_id: String,
) -> Result<bool, String> {
    let state = state.read().await;

    match provider_id.as_str() {
        "claude" => state.claude.login().await.map_err(|e| e.to_string()),
        "openai" => state.openai.login().await.map_err(|e| e.to_string()),
        "gemini" => state.gemini.login().await.map_err(|e| e.to_string()),
        "codex" => state.codex.login().await.map_err(|e| e.to_string()),
        _ => Err(format!("Unknown provider: {}", provider_id)),
    }
}

/// Logs out from a provider
#[tauri::command]
pub async fn logout_provider(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
    provider_id: String,
) -> Result<(), String> {
    let state = state.read().await;

    match provider_id.as_str() {
        "claude" => state.claude.logout().await.map_err(|e| e.to_string()),
        "openai" => state.openai.logout().await.map_err(|e| e.to_string()),
        "gemini" => state.gemini.logout().await.map_err(|e| e.to_string()),
        "codex" => state.codex.logout().await.map_err(|e| e.to_string()),
        _ => Err(format!("Unknown provider: {}", provider_id)),
    }
}

/// Gets metadata for all available providers
#[tauri::command]
pub async fn get_providers(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
) -> Result<Vec<ProviderMetadata>, String> {
    let state = state.read().await;
    Ok(state.registry.metadata())
}

/// Gets list of enabled provider IDs in display order
#[tauri::command]
pub fn get_enabled_providers() -> Result<Vec<String>, String> {
    let config = AppConfig::load();
    Ok(config.enabled_providers)
}

/// Enables or disables a provider
#[tauri::command]
pub fn set_provider_enabled(provider_id: String, enabled: bool) -> Result<(), String> {
    let mut config = AppConfig::load();

    if enabled {
        if !config.enabled_providers.contains(&provider_id) {
            config.enabled_providers.push(provider_id.clone());
        }
    } else {
        config.enabled_providers.retain(|p| p != &provider_id);
    }

    // Update provider settings
    config
        .provider_settings
        .entry(provider_id)
        .or_insert_with(ProviderSettings::default)
        .enabled = enabled;

    config.save()
}

/// Sets the order of enabled providers
#[tauri::command]
pub fn set_provider_order(order: Vec<String>) -> Result<(), String> {
    let mut config = AppConfig::load();
    config.enabled_providers = order;
    config.save()
}

/// Sets the API key for a provider
#[tauri::command]
pub fn set_provider_api_key(provider_id: String, api_key: String) -> Result<(), String> {
    let mut config = AppConfig::load();

    config
        .provider_settings
        .entry(provider_id.clone())
        .or_insert_with(ProviderSettings::default)
        .api_key = if api_key.is_empty() {
        None
    } else {
        Some(api_key)
    };

    config.save()?;

    // Also store in system keychain for security
    if let Ok(entry) = keyring::Entry::new(&provider_id, "api_key") {
        if config
            .provider_settings
            .get(&provider_id)
            .and_then(|s| s.api_key.as_ref())
            .is_some()
        {
            let key = config.provider_settings[&provider_id]
                .api_key
                .as_ref()
                .unwrap();
            let _ = entry.set_password(key);
        } else {
            let _ = entry.delete_credential();
        }
    }

    Ok(())
}
