//! Tauri IPC Commands
//!
//! All commands that can be called from the frontend via Tauri IPC.

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

use crate::config::{AppConfig, ProviderSettings};
use crate::providers::{Provider, ProviderMetadata, UsageSnapshot};
use crate::AppState;

/// Usage status level for tray icon
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayStatus {
    Normal,
    Warning,
    Critical,
}

/// Updates the system tray icon based on the current max usage level.
///
/// - Normal (< warning threshold): green icon
/// - Warning (>= warning threshold): yellow icon
/// - Critical (>= critical threshold): red icon
#[tauri::command]
pub fn update_tray_status(
    app: tauri::AppHandle,
    status: TrayStatus,
) -> Result<(), String> {
    // Generate a 32x32 RGBA pixel buffer with a bar chart icon (4 rounded bars)
    let color: [u8; 3] = match status {
        TrayStatus::Normal => [34, 211, 238],    // cyan #22D3EE
        TrayStatus::Warning => [245, 158, 11],   // amber #F59E0B
        TrayStatus::Critical => [239, 68, 68],   // red #EF4444
    };

    const SIZE: u32 = 32;
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];

    // 4 bars with different heights, 5px wide each, 2px gap, centered horizontally
    let bar_width: u32 = 5;
    let gap: u32 = 2;
    let total_width = 4 * bar_width + 3 * gap;
    let x_offset = (SIZE - total_width) / 2;
    let bottom = SIZE - 3;
    let max_height = SIZE - 6;
    let radius: f32 = 2.0; // rounded corner radius

    let bar_heights: [u32; 4] = [
        (max_height as f32 * 0.40) as u32,
        (max_height as f32 * 0.70) as u32,
        (max_height as f32 * 0.55) as u32,
        (max_height as f32 * 0.90) as u32,
    ];

    for (i, &height) in bar_heights.iter().enumerate() {
        let bx = x_offset + i as u32 * (bar_width + gap);
        let top = bottom - height;

        for y in top..bottom {
            for x in bx..(bx + bar_width) {
                if x < SIZE && y < SIZE {
                    // Compute distance to rounded rect to get anti-aliased edges
                    let fx = x as f32 + 0.5;
                    let fy = y as f32 + 0.5;
                    let rect_left = bx as f32;
                    let rect_top = top as f32;
                    let rect_right = (bx + bar_width) as f32;
                    let rect_bottom = bottom as f32;

                    // Distance to rounded rect corners
                    let mut alpha = 1.0_f32;
                    let corners: [(f32, f32); 4] = [
                        (rect_left + radius, rect_top + radius),
                        (rect_right - radius, rect_top + radius),
                        (rect_left + radius, rect_bottom - radius),
                        (rect_right - radius, rect_bottom - radius),
                    ];
                    for &(cx, cy) in &corners {
                        let in_corner_x = (fx < rect_left + radius && fx < cx)
                            || (fx > rect_right - radius && fx > cx);
                        let in_corner_y = (fy < rect_top + radius && fy < cy)
                            || (fy > rect_bottom - radius && fy > cy);
                        if in_corner_x && in_corner_y {
                            let dist = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                            alpha = (radius + 0.5 - dist).clamp(0.0, 1.0);
                        }
                    }

                    if alpha > 0.0 {
                        let idx = ((y * SIZE + x) * 4) as usize;
                        let progress = (bottom - y) as f32 / height as f32;
                        let brightness = 0.7 + 0.3 * progress;
                        pixels[idx] = (color[0] as f32 * brightness).min(255.0) as u8;
                        pixels[idx + 1] = (color[1] as f32 * brightness).min(255.0) as u8;
                        pixels[idx + 2] = (color[2] as f32 * brightness).min(255.0) as u8;
                        pixels[idx + 3] = (255.0 * alpha) as u8;
                    }
                }
            }
        }
    }

    let icon = tauri::image::Image::new_owned(pixels, SIZE, SIZE);

    // Find the tray icon and update it
    if let Some(tray) = app.tray_by_id("gptbar-tray") {
        tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;
        let tooltip = match status {
            TrayStatus::Normal => "GPTBar - Usage normal",
            TrayStatus::Warning => "GPTBar - Usage warning",
            TrayStatus::Critical => "GPTBar - Usage critical!",
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }

    Ok(())
}

/// Resizes the main window to fit the given content height, then repositions
/// it so the bottom edge stays near the taskbar (same anchor point).
#[tauri::command]
pub fn resize_window(app: tauri::AppHandle, height: u32) -> Result<(), String> {
    let clamped = height.clamp(300, 700) as i32;
    if let Some(window) = app.get_webview_window("main") {
        // Capture current bottom edge before resize
        let pos = window.outer_position().ok();
        let old_size = window.outer_size().ok();

        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: 320,
            height: clamped as u32,
        }));

        // Reposition so bottom edge stays the same (window grows upward)
        if let (Some(p), Some(s)) = (pos, old_size) {
            let old_bottom = p.y + s.height as i32;
            let new_y = old_bottom - clamped;
            let _ = window.set_position(tauri::Position::Physical(
                tauri::PhysicalPosition::new(p.x, new_y),
            ));
        }
    }
    Ok(())
}

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

/// Sets the refresh interval.
///
/// Returns an optional warning message if the interval is below the recommended minimum.
#[tauri::command]
pub fn set_refresh_interval(minutes: u32) -> Result<Option<String>, String> {
    let mut config = AppConfig::load();
    config.refresh_interval = minutes;
    let warning = config.validate_refresh_interval();
    config.save()?;
    Ok(warning)
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

/// Sets the notification thresholds (warning and critical percentages)
#[tauri::command]
pub fn set_notification_thresholds(warning: f64, critical: f64) -> Result<(), String> {
    if warning < 0.0 || warning > 100.0 || critical < 0.0 || critical > 100.0 {
        return Err("Thresholds must be between 0 and 100".to_string());
    }
    if warning >= critical {
        return Err("Warning threshold must be lower than critical threshold".to_string());
    }
    let mut config = AppConfig::load();
    config.warning_threshold = warning;
    config.critical_threshold = critical;
    config.save()
}

/// Gets the current notification thresholds
#[tauri::command]
pub fn get_notification_thresholds() -> Result<(f64, f64), String> {
    let config = AppConfig::load();
    Ok((config.warning_threshold, config.critical_threshold))
}

/// Sets the notification cooldown in minutes
#[tauri::command]
pub fn set_notification_cooldown(minutes: u64) -> Result<(), String> {
    if minutes == 0 || minutes > 1440 {
        return Err("Cooldown must be between 1 and 1440 minutes".to_string());
    }
    let mut config = AppConfig::load();
    config.notification_cooldown_minutes = minutes;
    config.save()
}

/// Gets the current notification cooldown in minutes
#[tauri::command]
pub fn get_notification_cooldown() -> Result<u64, String> {
    let config = AppConfig::load();
    Ok(config.notification_cooldown_minutes)
}

/// Mutes or unmutes notifications for a specific provider
#[tauri::command]
pub fn set_provider_muted(provider_id: String, muted: bool) -> Result<(), String> {
    let mut config = AppConfig::load();
    if muted {
        if !config.muted_providers.contains(&provider_id) {
            config.muted_providers.push(provider_id);
        }
    } else {
        config.muted_providers.retain(|p| p != &provider_id);
    }
    config.save()
}

/// Gets the list of muted provider IDs
#[tauri::command]
pub fn get_muted_providers() -> Result<Vec<String>, String> {
    let config = AppConfig::load();
    Ok(config.muted_providers)
}

// ============================================================================
// History Commands
// ============================================================================

/// Returns history entries for a provider (most recent first, up to `limit`)
#[tauri::command]
pub fn get_provider_history(
    provider_id: String,
    limit: Option<usize>,
) -> Result<Vec<crate::history::HistoryEntry>, String> {
    crate::history::get_history(&provider_id, limit.unwrap_or(50))
}

/// Exports all usage history as JSON string
#[tauri::command]
pub fn export_history_json() -> Result<String, String> {
    crate::history::export_history_json()
}

/// Exports all usage history as CSV string
#[tauri::command]
pub fn export_history_csv() -> Result<String, String> {
    crate::history::export_history_csv()
}

/// Clears history for a specific provider
#[tauri::command]
pub fn clear_provider_history(provider_id: String) -> Result<(), String> {
    crate::history::clear_history(&provider_id)
}

// ============================================================================
// Generic Provider Commands
// ============================================================================

/// Fetches usage data from a specific provider and saves it to history
#[tauri::command]
pub async fn fetch_provider_usage(
    state: tauri::State<'_, Arc<RwLock<AppState>>>,
    provider_id: String,
) -> Result<UsageSnapshot, String> {
    let state = state.read().await;

    let snapshot = match provider_id.as_str() {
        "claude" => state.claude.fetch().await.map_err(|e| e.to_string()),
        "openai" => state.openai.fetch().await.map_err(|e| e.to_string()),
        "gemini" => state.gemini.fetch().await.map_err(|e| e.to_string()),
        "codex" => state.codex.fetch().await.map_err(|e| e.to_string()),
        "xai" => state.xai.fetch().await.map_err(|e| e.to_string()),
        _ => Err(format!("Unknown provider: {}", provider_id)),
    }?;

    // Save to history (non-fatal if it fails)
    if let Err(e) = crate::history::save_snapshot(&provider_id, &snapshot) {
        tracing::warn!("Failed to save history for {}: {}", provider_id, e);
    }

    Ok(snapshot)
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
        "xai" => Ok(state.xai.is_available().await),
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
        "xai" => state.xai.login().await.map_err(|e| e.to_string()),
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
        "xai" => state.xai.logout().await.map_err(|e| e.to_string()),
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

