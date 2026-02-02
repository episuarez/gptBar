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
            warning_percent: 80.0,
            critical_percent: 95.0,
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

    /// Checks a snapshot against thresholds and sends notification if needed
    async fn check_and_notify(&self, provider_id: &str, snapshot: &UsageSnapshot) {
        // Get the highest usage across all windows
        let max_usage = snapshot.max_usage();

        let level = if max_usage >= self.thresholds.critical_percent {
            Some(NotificationLevel::Critical)
        } else if max_usage >= self.thresholds.warning_percent {
            Some(NotificationLevel::Warning)
        } else {
            None
        };

        if let Some(level) = level {
            // Check cooldown
            if self.should_notify(provider_id).await {
                self.send_notification(provider_id, max_usage, level).await;
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

    /// Sends a notification
    async fn send_notification(&self, provider_id: &str, usage: f64, level: NotificationLevel) {
        // Update last notification time
        self.last_notifications
            .write()
            .await
            .insert(provider_id.to_string(), Utc::now());

        // Format the message
        let title = match level {
            NotificationLevel::Warning => format!("{} Usage Warning", provider_id),
            NotificationLevel::Critical => format!("{} Usage Critical!", provider_id),
        };

        let message = format!("Usage is at {:.1}%", usage);

        tracing::info!(
            "Sending {} notification for {}: {}",
            match level {
                NotificationLevel::Warning => "warning",
                NotificationLevel::Critical => "critical",
            },
            provider_id,
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
        assert_eq!(thresholds.warning_percent, 80.0);
        assert_eq!(thresholds.critical_percent, 95.0);
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
        let agent = NotificationAgent::new();
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // Update with a warning-level snapshot
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(85.0));
        agent.update_snapshot("test-provider", &snapshot).await;

        assert_eq!(notify_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_notification_agent_critical() {
        let agent = NotificationAgent::new();
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

        // Update with a critical-level snapshot
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(98.0));
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
        let thresholds = NotificationThresholds::new(80.0, 95.0).with_cooldown(1); // 1 minute
        let agent = NotificationAgent::with_thresholds(thresholds);
        let notify_count = Arc::new(AtomicU32::new(0));
        let notify_count_clone = notify_count.clone();

        agent
            .on_notify(move |_title, _message, _level| {
                notify_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // First notification should go through
        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(85.0));
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

        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(85.0));

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

        let snapshot = UsageSnapshot::new().with_primary(RateWindow::new(85.0));

        // Different providers should not affect each other's cooldown
        agent.update_snapshot("provider-1", &snapshot).await;
        agent.update_snapshot("provider-2", &snapshot).await;

        assert_eq!(notify_count.load(Ordering::SeqCst), 2);
    }
}
