//! Refresh agent - Periodically fetches usage data from providers
//!
//! Runs in the background and updates usage snapshots at configurable intervals.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::base::{Agent, AgentError, AgentStatus};
use crate::providers::{Provider, UsageSnapshot};

/// Callback type for when usage data is updated
pub type UsageCallback = Box<dyn Fn(&str, &UsageSnapshot) + Send + Sync>;

/// Configuration for the refresh agent
#[derive(Debug, Clone)]
pub struct RefreshConfig {
    /// Interval between refreshes
    pub interval: Duration,
    /// Whether to fetch immediately on start
    pub fetch_on_start: bool,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5 * 60), // 5 minutes
            fetch_on_start: true,
        }
    }
}

impl RefreshConfig {
    /// Creates a config with a custom interval in minutes
    pub fn with_interval_minutes(minutes: u64) -> Self {
        Self {
            interval: Duration::from_secs(minutes * 60),
            fetch_on_start: true,
        }
    }

    /// Creates a config with a custom interval in seconds (for testing)
    pub fn with_interval_seconds(seconds: u64) -> Self {
        Self {
            interval: Duration::from_secs(seconds),
            fetch_on_start: true,
        }
    }
}

/// Agent that periodically refreshes usage data from providers
pub struct RefreshAgent {
    config: RefreshConfig,
    providers: RwLock<Vec<Arc<dyn Provider>>>,
    status: RwLock<AgentStatus>,
    cancel_token: CancellationToken,
    snapshots: RwLock<std::collections::HashMap<String, UsageSnapshot>>,
    on_update: RwLock<Option<UsageCallback>>,
}

impl RefreshAgent {
    /// Creates a new RefreshAgent with default configuration
    pub fn new() -> Self {
        Self::with_config(RefreshConfig::default())
    }

    /// Creates a new RefreshAgent with custom configuration
    pub fn with_config(config: RefreshConfig) -> Self {
        Self {
            config,
            providers: RwLock::new(Vec::new()),
            status: RwLock::new(AgentStatus::Idle),
            cancel_token: CancellationToken::new(),
            snapshots: RwLock::new(std::collections::HashMap::new()),
            on_update: RwLock::new(None),
        }
    }

    /// Creates a new RefreshAgent with interval in minutes
    pub fn with_interval(minutes: u64) -> Self {
        Self::with_config(RefreshConfig::with_interval_minutes(minutes))
    }

    /// Adds a provider to monitor
    pub async fn add_provider(&self, provider: Arc<dyn Provider>) {
        self.providers.write().await.push(provider);
    }

    /// Removes all providers
    pub async fn clear_providers(&self) {
        self.providers.write().await.clear();
    }

    /// Sets a callback to be called when usage data is updated
    pub async fn on_update<F>(&self, callback: F)
    where
        F: Fn(&str, &UsageSnapshot) + Send + Sync + 'static,
    {
        *self.on_update.write().await = Some(Box::new(callback));
    }

    /// Gets the current snapshot for a provider
    pub async fn get_snapshot(&self, provider_id: &str) -> Option<UsageSnapshot> {
        self.snapshots.read().await.get(provider_id).cloned()
    }

    /// Gets all current snapshots
    pub async fn get_all_snapshots(&self) -> std::collections::HashMap<String, UsageSnapshot> {
        self.snapshots.read().await.clone()
    }

    /// Fetches data from all providers once
    async fn fetch_all(&self) {
        let providers = self.providers.read().await.clone();

        for provider in providers {
            if !provider.is_enabled() {
                continue;
            }

            let provider_id = provider.id().to_string();

            match provider.fetch().await {
                Ok(snapshot) => {
                    tracing::debug!("Fetched usage for {}: {:?}", provider_id, snapshot);

                    // Store the snapshot
                    self.snapshots
                        .write()
                        .await
                        .insert(provider_id.clone(), snapshot.clone());

                    // Call the callback if set
                    if let Some(ref callback) = *self.on_update.read().await {
                        callback(&provider_id, &snapshot);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch usage for {}: {}", provider_id, e);
                }
            }
        }
    }
}

impl Default for RefreshAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for RefreshAgent {
    fn id(&self) -> &'static str {
        "refresh"
    }

    fn name(&self) -> &'static str {
        "Refresh Agent"
    }

    fn status(&self) -> AgentStatus {
        // Use try_read to avoid blocking
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

        // Set status to running
        *self.status.write().await = AgentStatus::Running;

        // Reset cancellation token
        // Note: In a real implementation, we'd need to handle this differently
        // since CancellationToken doesn't have a reset method

        // Fetch immediately if configured
        if self.config.fetch_on_start {
            self.fetch_all().await;
        }

        // Main loop
        loop {
            tokio::select! {
                _ = tokio::time::sleep(self.config.interval) => {
                    self.fetch_all().await;
                }
                _ = self.cancel_token.cancelled() => {
                    tracing::info!("Refresh agent cancelled");
                    break;
                }
            }
        }

        *self.status.write().await = AgentStatus::Stopped;
        Ok(())
    }

    async fn stop(&self) -> Result<(), AgentError> {
        // Check if running
        {
            let status = self.status.read().await;
            if !status.is_running() {
                return Ok(()); // Already stopped
            }
        }

        // Cancel the token
        self.cancel_token.cancel();

        // Wait a bit for the agent to stop
        tokio::time::sleep(Duration::from_millis(100)).await;

        *self.status.write().await = AgentStatus::Stopped;
        Ok(())
    }

    async fn trigger(&self) -> Result<(), AgentError> {
        self.fetch_all().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ProviderError, RateWindow};
    use std::sync::atomic::{AtomicU32, Ordering};

    // Mock provider for testing
    struct MockProvider {
        fetch_count: Arc<AtomicU32>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                fetch_count: Arc::new(AtomicU32::new(0)),
            }
        }

        fn with_counter(counter: Arc<AtomicU32>) -> Self {
            Self {
                fetch_count: counter,
            }
        }

        fn fetch_count(&self) -> u32 {
            self.fetch_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &'static str {
            "mock"
        }

        fn name(&self) -> &'static str {
            "Mock Provider"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn fetch(&self) -> Result<UsageSnapshot, ProviderError> {
            self.fetch_count.fetch_add(1, Ordering::SeqCst);
            Ok(UsageSnapshot::new().with_primary(RateWindow::new(50.0)))
        }

        async fn login(&self) -> Result<bool, ProviderError> {
            Ok(true)
        }

        async fn logout(&self) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn is_available(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_refresh_config_default() {
        let config = RefreshConfig::default();
        assert_eq!(config.interval, Duration::from_secs(300));
        assert!(config.fetch_on_start);
    }

    #[test]
    fn test_refresh_config_with_minutes() {
        let config = RefreshConfig::with_interval_minutes(10);
        assert_eq!(config.interval, Duration::from_secs(600));
    }

    #[test]
    fn test_refresh_agent_new() {
        let agent = RefreshAgent::new();
        assert_eq!(agent.id(), "refresh");
        assert_eq!(agent.name(), "Refresh Agent");
        assert_eq!(agent.status(), AgentStatus::Idle);
    }

    #[tokio::test]
    async fn test_refresh_agent_add_provider() {
        let agent = RefreshAgent::new();
        let provider = Arc::new(MockProvider::new());

        agent.add_provider(provider).await;

        let providers = agent.providers.read().await;
        assert_eq!(providers.len(), 1);
    }

    #[tokio::test]
    async fn test_refresh_agent_trigger() {
        let agent = RefreshAgent::new();
        let counter = Arc::new(AtomicU32::new(0));
        let provider = Arc::new(MockProvider::with_counter(counter.clone()));

        agent.add_provider(provider).await;

        // Trigger a fetch
        agent.trigger().await.unwrap();

        // Should have fetched once
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Check snapshot is stored
        let snapshot = agent.get_snapshot("mock").await;
        assert!(snapshot.is_some());
        assert_eq!(snapshot.unwrap().primary.unwrap().used_percent, 50.0);
    }

    #[tokio::test]
    async fn test_refresh_agent_callback() {
        let agent = RefreshAgent::new();
        let provider = Arc::new(MockProvider::new());
        let callback_count = Arc::new(AtomicU32::new(0));
        let callback_count_clone = callback_count.clone();

        agent.add_provider(provider).await;
        agent
            .on_update(move |_id, _snapshot| {
                callback_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        agent.trigger().await.unwrap();

        assert_eq!(callback_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_refresh_agent_get_all_snapshots() {
        let agent = RefreshAgent::new();
        let provider = Arc::new(MockProvider::new());

        agent.add_provider(provider).await;
        agent.trigger().await.unwrap();

        let snapshots = agent.get_all_snapshots().await;
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots.contains_key("mock"));
    }

    #[tokio::test]
    async fn test_refresh_agent_clear_providers() {
        let agent = RefreshAgent::new();
        let provider = Arc::new(MockProvider::new());

        agent.add_provider(provider).await;
        assert_eq!(agent.providers.read().await.len(), 1);

        agent.clear_providers().await;
        assert_eq!(agent.providers.read().await.len(), 0);
    }
}
