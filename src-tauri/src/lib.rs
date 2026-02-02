//! GPTBar - Monitor AI provider usage from your system tray
//!
//! A cross-platform application for monitoring AI provider usage,
//! inspired by the macOS app CodexBar.
//!
//! ## Features
//!
//! - Monitor multiple AI providers (Claude, OpenAI, Gemini, Codex)
//! - System tray integration with visual usage indicators
//! - Notifications when approaching usage limits
//! - Secure credential storage (Windows Credential Manager, macOS Keychain, Linux Secret Service)
//! - Background refresh with configurable intervals
//! - Cross-platform support (Windows, macOS, Linux)
//!
//! ## Architecture
//!
//! The application follows SOLID principles and is organized into layers:
//!
//! - **Providers**: AI service integrations (Claude, OpenAI, Gemini, Codex)
//! - **Auth**: Secure credential storage and cookie extraction
//! - **Agents**: Background tasks (refresh, notifications)
//! - **Security**: Sanitization, secure strings, platform-specific encryption

pub mod agents;
pub mod auth;
mod commands;
pub mod config;
pub mod providers;
pub mod security;

use std::sync::Arc;
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, PhysicalPosition, WindowEvent,
};

use agents::{AgentManager, NotificationAgent, RefreshAgent};
use providers::{ClaudeProvider, CodexProvider, GeminiProvider, OpenAIProvider, ProviderRegistry};

/// Application state shared across the Tauri app
pub struct AppState {
    /// Agent manager for background tasks
    pub agent_manager: AgentManager,
    /// Provider registry
    pub registry: ProviderRegistry,
    /// Claude provider (for backwards compatibility)
    pub claude: Arc<ClaudeProvider>,
    /// OpenAI provider
    pub openai: Arc<OpenAIProvider>,
    /// Gemini provider
    pub gemini: Arc<GeminiProvider>,
    /// Codex provider
    pub codex: Arc<CodexProvider>,
}

impl AppState {
    /// Creates a new AppState with default configuration
    pub async fn new() -> Self {
        let claude = Arc::new(ClaudeProvider::new());
        let openai = Arc::new(OpenAIProvider::new());
        let gemini = Arc::new(GeminiProvider::new());
        let codex = Arc::new(CodexProvider::new());
        let registry = ProviderRegistry::new();
        let agent_manager = AgentManager::new();

        // Create and register agents
        let refresh = Arc::new(RefreshAgent::with_interval(5)); // 5 minute refresh
        let notification = Arc::new(NotificationAgent::new());

        // Add all providers to refresh agent
        refresh.add_provider(claude.clone()).await;
        refresh.add_provider(openai.clone()).await;
        refresh.add_provider(gemini.clone()).await;
        refresh.add_provider(codex.clone()).await;

        agent_manager.register(refresh).await;
        agent_manager.register(notification).await;

        Self {
            agent_manager,
            registry,
            claude,
            openai,
            gemini,
            codex,
        }
    }
}

// ============================================================================
// Tauri App Entry Point
// ============================================================================

/// Initializes and runs the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("gptbar=debug".parse().unwrap())
                .add_directive("info".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting GPTBar...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Create app state
            let state = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { AppState::new().await });

            let state = Arc::new(tokio::sync::RwLock::new(state));

            // Manage state
            app.manage(state.clone());

            // Start agents in background
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                let state = state_clone.read().await;
                if let Err(e) = state.agent_manager.start_all().await {
                    tracing::error!("Failed to start agents: {}", e);
                }
            });

            // Create system tray icon
            let icon = Image::from_path("icons/icon.png")
                .or_else(|_| Image::from_path("icons/32x32.png"))
                .unwrap_or_else(|_| {
                    Image::from_bytes(include_bytes!("../icons/32x32.png"))
                        .expect("Failed to load embedded icon")
                });

            // Window dimensions (increased for new design)
            const WINDOW_WIDTH: i32 = 300;
            const WINDOW_HEIGHT: i32 = 520;
            const MARGIN: i32 = 10;

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .tooltip("GPTBar - Click to view usage")
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();

                        // Get or create the popup window
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                // Position near tray icon
                                if let Some(rect) = tray.rect().ok().flatten() {
                                    let (tray_x, tray_y) = match rect.position {
                                        tauri::Position::Physical(p) => (p.x, p.y),
                                        tauri::Position::Logical(l) => (l.x as i32, l.y as i32),
                                    };
                                    let (tray_w, _tray_h) = match rect.size {
                                        tauri::Size::Physical(s) => (s.width as i32, s.height as i32),
                                        tauri::Size::Logical(s) => (s.width as i32, s.height as i32),
                                    };

                                    // Position: horizontally centered on tray icon, above the taskbar
                                    let x = tray_x + (tray_w / 2) - (WINDOW_WIDTH / 2);
                                    let y = tray_y - WINDOW_HEIGHT - MARGIN;

                                    let _ = window.set_position(tauri::Position::Physical(
                                        PhysicalPosition::new(x, y),
                                    ));
                                }
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Listen for window focus loss to auto-hide
            let main_window = app.get_webview_window("main");
            if let Some(window) = main_window {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(focused) = event {
                        if !focused {
                            // Window lost focus - hide it
                            let _ = window_clone.hide();
                        }
                    }
                });
            }

            tracing::info!("GPTBar initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Legacy Claude commands (for backwards compatibility)
            commands::fetch_usage,
            commands::get_cached_usage,
            commands::is_claude_available,
            commands::login_claude,
            commands::logout_claude,
            commands::reload_token,
            // Generic provider commands
            commands::fetch_provider_usage,
            commands::is_provider_available,
            commands::login_provider,
            commands::logout_provider,
            commands::get_providers,
            commands::get_enabled_providers,
            commands::set_provider_enabled,
            commands::set_provider_order,
            commands::set_provider_api_key,
            // Agent commands
            commands::trigger_refresh,
            commands::get_agent_status,
            // Config commands
            commands::get_config,
            commands::save_config,
            commands::set_refresh_interval,
            commands::set_start_on_login,
            commands::is_autostart_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
