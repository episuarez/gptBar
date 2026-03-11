//! Notification agent - Sends notifications when usage thresholds are reached
//!
//! Monitors usage snapshots and sends system notifications when usage
//! reaches warning (80%) or critical (95%) levels.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::base::{Agent, AgentError, AgentStatus};
use crate::providers::UsageSnapshot;

/// Notification threshold configuration
#[derive(Debug, Clone)]
pub struct NotificationThresholds {
    /// Usage percentage that triggers a warning notification
    pub warning_percent: f64,
    /// Usage percentage that triggers a critical notification
    pub critical_percent: f64,
    /// Minimum time between notifications for the same provider (in minutes)
    pub cooldown_minutes: u64,
}

impl Default for NotificationThresholds {
    fn default() -> Self {
        Self {
            warning_percent: 90.0,
            critical_percent: 100.0,
            cooldown_minutes: 30,
        }
    }
}

impl NotificationThresholds {
    /// Creates thresholds with custom warning and critical levels
    pub fn new(warning: f64, critical: f64) -> Self {
        Self {
            warning_percent: warning,
            critical_percent: critical,
            cooldown_minutes: 30,
        }
    }

    /// Sets the cooldown period in minutes
    pub fn with_cooldown(mut self, minutes: u64) -> Self {
        self.cooldown_minutes = minutes;
        self
    }
}

/// Notification level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Warning notification (approaching limit)
    Warning,
    /// Critical notification (near or at limit)
    Critical,
}

/// Callback type for sending notifications
pub type NotifyCallback = Box<dyn Fn(&str, &str, NotificationLevel) + Send + Sync>;

/// Agent that monitors usage and sends notifications
pub struct NotificationAgent {
    thresholds: NotificationThresholds,
    status: RwLock<AgentStatus>,
    cancel_token: CancellationToken,
    /// Tracks the last notification time for each provider
    last_notifications: RwLock<HashMap<String, DateTime<Utc>>>,
    /// Callback to send notifications
    notify_callback: RwLock<Option<NotifyCallback>>,
    /// Current snapshots to monitor
    snapshots: Arc<RwLock<HashMap<String, UsageSnapshot>>>,
}

impl NotificationAgent {
    /// Creates a new NotificationAgent with default thresholds
    pub fn new() -> Self {
        Self::with_thresholds(NotificationThresholds::default())
    }

    /// Creates a new NotificationAgent with custom thresholds
    pub fn with_thresholds(thresholds: NotificationThresholds) -> Self {
        Self {
            thresholds,
            status: RwLock::new(AgentStatus::Idle),
            cancel_token: CancellationToken::new(),
            last_notifications: RwLock::new(HashMap::new()),
            notify_callback: RwLock::new(None),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Sets the callback for sending notifications
    pub async fn on_notify<F>(&self, callback: F)
    where
        F: Fn(&str, &str, NotificationLevel) + Send + Sync + 'static,
    {
        *self.notify_callback.write().await = Some(Box::new(callback));
    }

    /// Sets the snapshots to monitor (typically shared with RefreshAgent)
    pub fn set_snapshots(&mut self, snapshots: Arc<RwLock<HashMap<String, UsageSnapshot>>>) {
        self.snapshots = snapshots;
    }

    /// Updates a snapshot and checks for threshold violations
    pub async fn update_snapshot(&self, provider_id: &str, snapshot: &UsageSnapshot) {
        // Store the snapshot
        self.snapshots
            .write()
            .await
            .insert(provider_id.to_string(), snapshot.clone());

        // Check thresholds
        self.check_and_notify(provider_id, snapshot).await;
    }

    /// Checks a snapshot against thresholds and sends notification if needed.
    ///
    /// Checks each window (primary, secondary, tertiary) independently so the user
    /// knows exactly which limit is being approached.
    async fn check_and_notify(&self, provider_id: &str, snapshot: &UsageSnapshot) {
        let windows: Vec<(&str, Option<&crate::providers::RateWindow>)> = vec![
            ("primary", snapshot.primary.as_ref()),
            ("secondary", snapshot.secondary.as_ref()),
            ("tertiary", snapshot.tertiary.as_ref()),
        ];

        for (window_name, window) in windows {
            if let Some(w) = window {
                let level = if w.is_critical_at(self.thresholds.critical_percent) {
                    Some(NotificationLevel::Critical)
                } else if w.is_warning_at(self.thresholds.warning_percent) {
                    Some(NotificationLevel::Warning)
                } else {
                    None
                };

                if let Some(level) = level {
                    // Use a composite key for cooldown: provider + window
                    let cooldown_key = format!("{}:{}", provider_id, window_name);
                    if self.should_notify(&cooldown_key).await {
                        self.send_window_notification(
                            provider_id,
                            window_name,
                            w,
                            level,
                        )
                        .await;
                    }
                }
            }
        }
    }

    /// Checks if we should send a notification (respects cooldown)
    async fn should_notify(&self, provider_id: &str) -> bool {
        let last_notifications = self.last_notifications.read().await;

        if let Some(last_time) = last_notifications.get(provider_id) {
            let cooldown = chrono::Duration::minutes(self.thresholds.cooldown_minutes as i64);
            let now = Utc::now();

            if now - *last_time < cooldown {
                return false;
            }
        }

        true
    }

    /// Sends a notification for a specific rate window
    async fn send_window_notification(
        &self,
        provider_id: &str,
        window_name: &str,
        window: &crate::providers::RateWindow,
        level: NotificationLevel,
    ) {
        // Update last notification time using composite key
        let cooldown_key = format!("{}:{}", provider_id, window_name);
        self.last_notifications
            .write()
            .await
            .insert(cooldown_key, Utc::now());

        // Build a human-readable window description
        let window_desc = if let Some(minutes) = window.window_minutes {
            let hours = minutes / 60;
            if hours > 0 {
                format!("{} ({}h)", window_name, hours)
            } else {
                format!("{} ({}min)", window_name, minutes)
            }
        } else {
            window_name.to_string()
        };

        // Format the message
        let title = match level {
            NotificationLevel::Warning => format!("{} Usage Warning", provider_id),
            NotificationLevel::Critical => format!("{} Usage Critical!", provider_id),
        };

        let message = format!("{} usage is at {:.1}%", window_desc, window.used_percent);

        tracing::info!(
            "Sending {} notification for {} [{}]: {}",
            match level {
                NotificationLevel::Warning => "warning",
                NotificationLevel::Critical => "critical",
            },
            provider_id,
            window_name,
            message
        );

        // Call the notification callback if set
        if let Some(ref callback) = *self.notify_callback.read().await {
            callback(&title, &message, level);
        }
    }

    /// Gets the current thresholds
    pub fn thresholds(&self) -> &NotificationThresholds {
        &self.thresholds
    }

    /// Clears the notification history (resets cooldowns)
    pub async fn clear_history(&self) {
        self.last_notifications.write().await.clear();
    }
}

impl Default for NotificationAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for NotificationAgent {
    fn id(&self) -> &'static str {
        "notification"
    }

    fn name(&self) -> &'static str {
        "Notification Agent"
    }

    fn status(&self) -> AgentStatus {
        self.status
            .try_read()
            .map(|s| s.clone())
            .unwrap_or(AgentStatus::Idle)
    }

    async fn start(&self) -> Result<(), AgentError> {
        // Check if already running
        {
            let status = self.status.read().await;
            if status.is_running() {
                return Err(AgentError::AlreadyRunning);
            }
        }

        *self.status.write().await = AgentStatus::Running;

        // Main loop - check snapshots periodically
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {
                    // Check all snapshots
                    let snapshots = self.snapshots.read().await.clone();
                    for (provider_id, snapshot) in snapshots {
                        self.check_and_notify(&provider_id, &snapshot).await;
                    }
                }
                _ = self.cancel_token.cancelled() => {
                    tracing::info!("Notification agent cancelled");
                    break;
                }
            }
        }

        *self.status.write().await = AgentStatus::Stopped;
        Ok(())
    }

    async fn stop(&self) -> Result<(), AgentError> {
        {
            let status = self.status.read().await;
            if !status.is_running() {
                return Ok(());
            }
        }

        self.cancel_token.cancel();
        tokio::time::sleep(Duration::from_millis(100)).await;
        *self.status.write().await = AgentStatus::Stopped;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::RateWindow;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_notification_thresholds_default() {
        let thresholds = NotificationThresholds::default();
        assert_eq!(thresholds.warning_percent, 90.0);
        assert_eq!(thresholds.critical_percent, 100.0);
        assert_eq!(thresholds.cooldown_minutes, 30);
    }

    #[test]
    fn test_notification_thresholds_custom() {
        let thresholds = NotificationThresholds::new(70.0, 90.0).with_cooldown(15);
        assert_eq!(thresholds.warning_percent, 70.0);
        assert_eq!(thresholds.critical_percent, 90.0);
        assert_eq!(thresholds.cooldown_minutes, 15);
    }

    #[test]
    fn test_notification_agent_new() {
        let agent = NotificationAgent::new();
        assert_eq!(agent.id(), "notification");
        assert_eq!(agent.name(), "Notification Agent");
        assert_eq!(agent.status(), AgentStatus::Idle);
    }

    #[tokio::test]
    async fn test_notification_agent_warning() {
        let agent = NotificationAgent::new(); // default warning=90%
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // Update with a warning-level snapshot (>= 90%)
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(92.0));
        agent.update_snapshot("test-provider", &snapshot).await;

        assert_eq!(notify_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_notification_agent_critical() {
        let agent = NotificationAgent::new(); // default critical=100%
        let last_level = Arc::new(RwLock::new(None));
        let last_level_clone = last_level.clone();

        agent
            .on_notify(move |_title, _message, level| {
                let last_level = last_level_clone.clone();
                tokio::spawn(async move {
                    *last_level.write().await = Some(level);
                });
            })
            .await;

        // Update with a critical-level snapshot (100% to match default critical threshold)
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(100.0));
        agent.update_snapshot("test-provider", &snapshot).await;

        // Give async callback time to run
        tokio::time::sleep(Duration::from_millis(50)).await;

        let level = *last_level.read().await;
        assert_eq!(level, Some(NotificationLevel::Critical));
    }

    #[tokio::test]
    async fn test_notification_agent_below_threshold() {
        let agent = NotificationAgent::new();
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // Update with a normal-level snapshot
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(50.0));
        agent.update_snapshot("test-provider", &snapshot).await;

        assert_eq!(notify_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_notification_agent_cooldown() {
        // Use a very short cooldown for testing
        let thresholds = NotificationThresholds::new(90.0, 100.0).with_cooldown(1); // 1 minute
        let agent = NotificationAgent::with_thresholds(thresholds);
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // First notification should go through
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(92.0));
        agent.update_snapshot("test-provider", &snapshot).await;
        assert_eq!(notify_count.load(Ordering::SeqCst), 1);

        // Second notification should be blocked by cooldown
        agent.update_snapshot("test-provider", &snapshot).await;
        assert_eq!(notify_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_notification_agent_clear_history() {
        let agent = NotificationAgent::new();
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(92.0));

        // First notification
        agent.update_snapshot("test-provider", &snapshot).await;
        assert_eq!(notify_count.load(Ordering::SeqCst), 1);

        // Clear history
        agent.clear_history().await;

        // Should notify again
        agent.update_snapshot("test-provider", &snapshot).await;
        assert_eq!(notify_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_notification_agent_multiple_providers() {
        let agent = NotificationAgent::new();
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(92.0));

        // Different providers should not affect each other's cooldown
        agent.update_snapshot("provider-1", &snapshot).await;
        agent.update_snapshot("provider-2", &snapshot).await;

        assert_eq!(notify_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_notification_per_window_primary() {
        let thresholds = NotificationThresholds::new(90.0, 100.0);
        let agent = NotificationAgent::with_thresholds(thresholds);
        let messages = Arc::new(RwLock::new(Vec::<String>::new()));
        let messages_clone = messages.clone();

        agent
            .on_notify(move |title, message, _level| {
                let messages = messages_clone.clone();
                let entry = format!("{}: {}", title, message);
                tokio::spawn(async move {
                    messages.write().await.push(entry);
                });
            })
            .await;

        // Only primary exceeds threshold
        let snapshot = UsageSnapshot::new()
            .with_primary(RateWindow::new(92.0))
            .with_secondary(RateWindow::new(50.0));
        agent.update_snapshot("claude", &snapshot).await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        let msgs = messages.read().await;
        assert!(!msgs.is_empty(), "Should have sent a notification for primary window");
    }

    #[tokio::test]
    async fn test_notification_per_window_secondary() {
        let thresholds = NotificationThresholds::new(90.0, 100.0);
        let agent = NotificationAgent::with_thresholds(thresholds);
        let messages = Arc::new(RwLock::new(Vec::<String>::new()));
        let messages_clone = messages.clone();

        agent
            .on_notify(move |title, message, _level| {
                let messages = messages_clone.clone();
                let entry = format!("{}: {}", title, message);
                tokio::spawn(async move {
                    messages.write().await.push(entry);
                });
            })
            .await;

        // Only secondary exceeds threshold
        let snapshot = UsageSnapshot::new()
            .with_primary(RateWindow::new(50.0))
            .with_secondary(RateWindow::new(95.0));
        agent.update_snapshot("claude", &snapshot).await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        let msgs = messages.read().await;
        assert!(!msgs.is_empty(), "Should have sent a notification for secondary window");
    }

    #[tokio::test]
    async fn test_notification_per_window_tertiary() {
        let thresholds = NotificationThresholds::new(90.0, 100.0);
        let agent = NotificationAgent::with_thresholds(thresholds);
        let messages = Arc::new(RwLock::new(Vec::<String>::new()));
        let messages_clone = messages.clone();

        agent
            .on_notify(move |title, message, _level| {
                let messages = messages_clone.clone();
                let entry = format!("{}: {}", title, message);
                tokio::spawn(async move {
                    messages.write().await.push(entry);
                });
            })
            .await;

        // Only tertiary exceeds threshold
        let snapshot = UsageSnapshot::new()
            .with_primary(RateWindow::new(30.0))
            .with_tertiary(RateWindow::new(91.0));
        agent.update_snapshot("claude", &snapshot).await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        let msgs = messages.read().await;
        assert!(!msgs.is_empty(), "Should have sent a notification for tertiary window");
    }

    #[tokio::test]
    async fn test_notification_includes_window_name() {
        let thresholds = NotificationThresholds::new(90.0, 100.0);
        let agent = NotificationAgent::with_thresholds(thresholds);
        let messages = Arc::new(RwLock::new(Vec::<String>::new()));
        let messages_clone = messages.clone();

        agent
            .on_notify(move |_title, message, _level| {
                let messages = messages_clone.clone();
                let msg = message.to_string();
                tokio::spawn(async move {
                    messages.write().await.push(msg);
                });
            })
            .await;

        let snapshot = UsageSnapshot::new()
            .with_primary(RateWindow::new(92.0).with_window_minutes(300))
            .with_secondary(RateWindow::new(50.0));
        agent.update_snapshot("claude", &snapshot).await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        let msgs = messages.read().await;
        // Message should include which window triggered (e.g. "primary" or window description)
        assert!(
            msgs.iter().any(|m| m.contains("primary") || m.contains("5h") || m.contains("300")),
            "Notification message should identify the window. Got: {:?}",
            *msgs
        );
    }

    #[tokio::test]
    async fn test_notification_at_100_percent() {
        let agent = NotificationAgent::new(); // default: warning=90, critical=100
        let last_level = Arc::new(RwLock::new(None));
        let last_level_clone = last_level.clone();

        agent
            .on_notify(move |_title, _message, level| {
                let last_level = last_level_clone.clone();
                tokio::spawn(async move {
                    *last_level.write().await = Some(level);
                });
            })
            .await;

        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(100.0));
        agent.update_snapshot("claude", &snapshot).await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        let level = *last_level.read().await;
        assert_eq!(level, Some(NotificationLevel::Critical));
    }

    #[tokio::test]
    async fn test_notification_custom_warning_threshold() {
        // User sets warning at 85%
        let thresholds = NotificationThresholds::new(85.0, 100.0);
        let agent = NotificationAgent::with_thresholds(thresholds);
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // 86% should trigger with 85% threshold
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(86.0));
        agent.update_snapshot("test", &snapshot).await;
        assert_eq!(notify_count.load(Ordering::SeqCst), 1);
    }
}
