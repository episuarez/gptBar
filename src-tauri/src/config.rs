//! Configuration management for GPTBar
//!
//! Handles persistent settings including auto-start and refresh intervals.
//! Supports Windows, macOS, and Linux.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Settings for individual providers
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderSettings {
    /// Whether this provider is enabled
    pub enabled: bool,
    /// API key for providers that need it (OpenAI, Gemini).
    ///
    /// Secrets are NOT persisted here anymore: they live only in the OS
    /// keychain. This field is kept for backward-compatible reads of older
    /// config files; new keys are never written into it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Maps a provider id to its OS keychain service name.
///
/// Centralizes the service name so the command that stores a key and the
/// provider that reads it always agree. Without this they drifted apart
/// (UI stored under "gemini" while the provider read "google-gemini").
///
/// Note: Claude is intentionally absent — it reads Claude Code CLI's own
/// external credential store ("Claude Code-credentials"), not a gptbar key.
pub fn keychain_service(provider_id: &str) -> &'static str {
    match provider_id {
        "gemini" => "google-gemini",
        "codex" => "codex-cli",
        "openai" => "openai",
        "xai" => "xai",
        _ => "gptbar",
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Auto-refresh interval in minutes
    pub refresh_interval: u32,
    /// Start application on system login
    pub start_on_login: bool,
    /// List of enabled provider IDs in display order
    #[serde(default = "default_enabled_providers")]
    pub enabled_providers: Vec<String>,
    /// Per-provider settings
    #[serde(default)]
    pub provider_settings: HashMap<String, ProviderSettings>,
    /// Usage percentage that triggers a warning notification (default: 90%)
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: f64,
    /// Usage percentage that triggers a critical notification (default: 100%)
    #[serde(default = "default_critical_threshold")]
    pub critical_threshold: f64,
    /// Cooldown between repeat notifications for same provider/window, in minutes (default: 30)
    #[serde(default = "default_notification_cooldown")]
    pub notification_cooldown_minutes: u64,
    /// Whether each provider's notifications are muted
    #[serde(default)]
    pub muted_providers: Vec<String>,
}

fn default_enabled_providers() -> Vec<String> {
    vec!["claude".to_string()]
}

fn default_warning_threshold() -> f64 {
    90.0
}

fn default_critical_threshold() -> f64 {
    100.0
}

fn default_notification_cooldown() -> u64 {
    30
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut provider_settings = HashMap::new();
        provider_settings.insert(
            "claude".to_string(),
            ProviderSettings {
                enabled: true,
                api_key: None,
            },
        );
        for id in &["openai", "gemini", "codex", "xai"] {
            provider_settings.insert(
                id.to_string(),
                ProviderSettings {
                    enabled: false,
                    api_key: None,
                },
            );
        }

        Self {
            refresh_interval: 10,
            start_on_login: false,
            enabled_providers: default_enabled_providers(),
            provider_settings,
            warning_threshold: default_warning_threshold(),
            critical_threshold: default_critical_threshold(),
            notification_cooldown_minutes: default_notification_cooldown(),
            muted_providers: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Gets the config directory path (cross-platform) - public for use by other modules
    pub fn config_dir_pub() -> Option<PathBuf> {
        Self::config_dir()
    }

    /// Gets the config directory path (cross-platform)
    fn config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("APPDATA")
                .ok()
                .map(|p| PathBuf::from(p).join("GPTBar"))
        }

        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .ok()
                .map(|p| PathBuf::from(p).join("Library/Application Support/GPTBar"))
        }

        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|p| PathBuf::from(p).join(".config"))
                })
                .map(|p| p.join("gptbar"))
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }

    /// Gets the config file path
    fn config_path() -> Option<PathBuf> {
        let config_dir = Self::config_dir()?;

        // Create directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).ok()?;
        }

        Some(config_dir.join("config.json"))
    }

    /// Loads configuration from disk
    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(config) = serde_json::from_str(&content) {
                        return config;
                    }
                }
            }
        }
        Self::default()
    }

    /// Saves configuration to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path().ok_or("Could not determine config path")?;
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(&path, content).map_err(|e| format!("Failed to write config: {}", e))?;
        Ok(())
    }

    /// Check if a provider is enabled
    pub fn is_provider_enabled(&self, provider_id: &str) -> bool {
        self.enabled_providers.contains(&provider_id.to_string())
    }

    /// Validates the refresh interval and returns a warning message if it's too low.
    ///
    /// Intervals below 10 minutes may cause rate limiting from providers.
    pub fn validate_refresh_interval(&self) -> Option<String> {
        if self.refresh_interval < 10 {
            Some(format!(
                "A refresh interval of {} minutes may cause rate limiting errors. \
                 A minimum of 10 minutes is recommended to avoid hitting API rate limits.",
                self.refresh_interval
            ))
        } else {
            None
        }
    }

    /// Get API key for a provider
    pub fn get_provider_api_key(&self, provider_id: &str) -> Option<String> {
        self.provider_settings
            .get(provider_id)
            .and_then(|s| s.api_key.clone())
    }

    // ========================================================================
    // Windows auto-start (Registry)
    // ========================================================================

    #[cfg(target_os = "windows")]
    pub fn set_autostart(&self) -> Result<(), String> {
        use std::process::Command;

        let exe_path =
            std::env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))?;

        if self.start_on_login {
            let output = Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "GPTBar",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &exe_path.to_string_lossy(),
                    "/f",
                ])
                .output()
                .map_err(|e| format!("Failed to run reg command: {}", e))?;

            if !output.status.success() {
                return Err("Failed to add registry key".to_string());
            }
        } else {
            let _ = Command::new("reg")
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "GPTBar",
                    "/f",
                ])
                .output();
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub fn is_autostart_enabled() -> bool {
        use std::process::Command;

        Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "GPTBar",
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    // ========================================================================
    // macOS auto-start (LaunchAgent plist)
    // ========================================================================

    #[cfg(target_os = "macos")]
    fn launch_agent_path() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library/LaunchAgents/com.gptbar.app.plist"))
    }

    #[cfg(target_os = "macos")]
    pub fn set_autostart(&self) -> Result<(), String> {
        let plist_path = Self::launch_agent_path().ok_or("Could not determine LaunchAgent path")?;

        if self.start_on_login {
            let exe_path =
                std::env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))?;

            let plist_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.gptbar.app</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
                exe_path.display()
            );

            // Create LaunchAgents directory if needed
            if let Some(parent) = plist_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create LaunchAgents dir: {}", e))?;
            }

            fs::write(&plist_path, plist_content)
                .map_err(|e| format!("Failed to write plist: {}", e))?;
        } else {
            // Remove plist file
            let _ = fs::remove_file(&plist_path);
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn is_autostart_enabled() -> bool {
        Self::launch_agent_path()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    // ========================================================================
    // Linux auto-start (.desktop file in autostart)
    // ========================================================================

    #[cfg(target_os = "linux")]
    fn autostart_path() -> Option<PathBuf> {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|p| PathBuf::from(p).join(".config"))
            })
            .map(|p| p.join("autostart/gptbar.desktop"))
    }

    #[cfg(target_os = "linux")]
    pub fn set_autostart(&self) -> Result<(), String> {
        let desktop_path = Self::autostart_path().ok_or("Could not determine autostart path")?;

        if self.start_on_login {
            let exe_path =
                std::env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))?;

            let desktop_content = format!(
                r#"[Desktop Entry]
Type=Application
Name=GPTBar
Comment=Monitor AI provider usage from system tray
Exec={}
Icon=gptbar
Terminal=false
Categories=Utility;
StartupNotify=false
X-GNOME-Autostart-enabled=true
"#,
                exe_path.display()
            );

            // Create autostart directory if needed
            if let Some(parent) = desktop_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create autostart dir: {}", e))?;
            }

            fs::write(&desktop_path, desktop_content)
                .map_err(|e| format!("Failed to write desktop file: {}", e))?;
        } else {
            // Remove desktop file
            let _ = fs::remove_file(&desktop_path);
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn is_autostart_enabled() -> bool {
        Self::autostart_path().map(|p| p.exists()).unwrap_or(false)
    }

    // Fallback for other platforms
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    pub fn set_autostart(&self) -> Result<(), String> {
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    pub fn is_autostart_enabled() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.refresh_interval, 10);
        assert!(!config.start_on_login);
        assert!(config.enabled_providers.contains(&"claude".to_string()));
    }

    #[test]
    fn test_default_config_notification_thresholds() {
        let config = AppConfig::default();
        assert_eq!(config.warning_threshold, 90.0);
        assert_eq!(config.critical_threshold, 100.0);
    }

    #[test]
    fn test_serialize_deserialize_with_thresholds() {
        let config = AppConfig {
            warning_threshold: 85.0,
            critical_threshold: 98.0,
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let loaded: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.warning_threshold, 85.0);
        assert_eq!(loaded.critical_threshold, 98.0);
    }

    #[test]
    fn test_config_backward_compatible() {
        // JSON without the new fields should deserialize with defaults
        let json = r#"{
            "refresh_interval": 5,
            "start_on_login": false,
            "enabled_providers": ["claude"],
            "provider_settings": {}
        }"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.warning_threshold, 90.0);
        assert_eq!(config.critical_threshold, 100.0);
    }

    #[test]
    fn test_validate_refresh_interval_warning() {
        let config = AppConfig {
            refresh_interval: 3,
            ..Default::default()
        };
        let warning = config.validate_refresh_interval();
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("rate limiting"));
    }

    #[test]
    fn test_validate_refresh_interval_ok() {
        let config = AppConfig::default(); // 10 min
        let warning = config.validate_refresh_interval();
        assert!(warning.is_none());
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = AppConfig {
            refresh_interval: 10,
            start_on_login: true,
            enabled_providers: vec!["claude".to_string(), "openai".to_string()],
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let loaded: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.refresh_interval, 10);
        assert!(loaded.start_on_login);
        assert_eq!(loaded.enabled_providers.len(), 2);
    }

    #[test]
    fn test_provider_enabled() {
        let config = AppConfig::default();
        assert!(config.is_provider_enabled("claude"));
        assert!(!config.is_provider_enabled("openai"));
    }

    #[test]
    fn test_provider_api_key() {
        let mut config = AppConfig::default();
        config.provider_settings.insert(
            "openai".to_string(),
            ProviderSettings {
                enabled: true,
                api_key: Some("sk-test-key".to_string()),
            },
        );

        assert_eq!(
            config.get_provider_api_key("openai"),
            Some("sk-test-key".to_string())
        );
        assert_eq!(config.get_provider_api_key("claude"), None);
    }

    #[test]
    fn test_config_dir_exists() {
        // This test just verifies the function doesn't panic
        let dir = AppConfig::config_dir();
        assert!(dir.is_some());
    }
}
