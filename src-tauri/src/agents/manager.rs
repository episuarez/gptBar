//! Agent manager - Orchestrates multiple agents
//!
//! Provides lifecycle management for all agents, including starting,
//! stopping, and monitoring their status.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::base::{Agent, AgentError, AgentStatus};

/// Manages the lifecycle of multiple agents
pub struct AgentManager {
    agents: RwLock<HashMap<&'static str, Arc<dyn Agent>>>,
    handles: RwLock<HashMap<&'static str, JoinHandle<()>>>,
}

impl AgentManager {
    /// Creates a new AgentManager
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            handles: RwLock::new(HashMap::new()),
        }
    }

    /// Registers an agent with the manager
    pub async fn register(&self, agent: Arc<dyn Agent>) {
        let id = agent.id();
        self.agents.write().await.insert(id, agent);
    }

    /// Unregisters an agent from the manager
    ///
    /// This will stop the agent if it's running.
    pub async fn unregister(&self, id: &str) -> Option<Arc<dyn Agent>> {
        // Stop the agent first
        self.stop_agent(id).await.ok();

        // Remove from agents
        self.agents.write().await.remove(id)
    }

    /// Gets a reference to an agent by ID
    pub async fn get(&self, id: &str) -> Option<Arc<dyn Agent>> {
        self.agents.read().await.get(id).cloned()
    }

    /// Starts all registered agents
    pub async fn start_all(&self) -> Result<(), AgentError> {
        let agents = self.agents.read().await.clone();

        for (id, agent) in agents {
            if agent.status().is_stopped() {
                self.start_agent_internal(id, agent).await?;
            }
        }

        Ok(())
    }

    /// Starts a specific agent by ID
    pub async fn start_agent(&self, id: &str) -> Result<(), AgentError> {
        let agents = self.agents.read().await;
        let (static_id, agent) = agents
            .iter()
            .find(|(k, _)| *k == &id)
            .map(|(k, v)| (*k, Arc::clone(v)))
            .ok_or_else(|| AgentError::OperationFailed(format!("Agent '{}' not found", id)))?;
        drop(agents);

        self.start_agent_internal(static_id, agent).await
    }

    /// Internal method to start an agent
    async fn start_agent_internal(
        &self,
        id: &'static str,
        agent: Arc<dyn Agent>,
    ) -> Result<(), AgentError> {
        let agent_clone = Arc::clone(&agent);

        let handle = tokio::spawn(async move {
            if let Err(e) = agent_clone.start().await {
                tracing::error!("Agent '{}' error: {}", agent_clone.id(), e);
            }
        });

        self.handles.write().await.insert(id, handle);

        tracing::info!("Started agent: {}", id);
        Ok(())
    }

    /// Stops all agents
    pub async fn stop_all(&self) -> Result<(), AgentError> {
        let agents = self.agents.read().await.clone();

        for (id, agent) in agents {
            if agent.status().is_running() {
                self.stop_agent_internal(id, agent).await?;
            }
        }

        Ok(())
    }

    /// Stops a specific agent by ID
    pub async fn stop_agent(&self, id: &str) -> Result<(), AgentError> {
        let agents = self.agents.read().await;
        let found = agents
            .iter()
            .find(|(k, _)| *k == &id)
            .map(|(k, v)| (*k, Arc::clone(v)));
        drop(agents);

        if let Some((static_id, agent)) = found {
            self.stop_agent_internal(static_id, agent).await
        } else {
            Ok(()) // Agent not found, nothing to stop
        }
    }

    /// Internal method to stop an agent
    async fn stop_agent_internal(
        &self,
        id: &'static str,
        agent: Arc<dyn Agent>,
    ) -> Result<(), AgentError> {
        // Signal the agent to stop
        agent.stop().await?;

        // Abort the task handle if it exists
        if let Some(handle) = self.handles.write().await.remove(id) {
            handle.abort();
        }

        tracing::info!("Stopped agent: {}", id);
        Ok(())
    }

    /// Gets the status of all agents
    pub async fn status(&self) -> HashMap<&'static str, AgentStatus> {
        let agents = self.agents.read().await;
        agents
            .iter()
            .map(|(id, agent)| (*id, agent.status()))
            .collect()
    }

    /// Gets the status of a specific agent
    pub async fn agent_status(&self, id: &str) -> Option<AgentStatus> {
        self.agents.read().await.get(id).map(|a| a.status())
    }

    /// Returns the number of registered agents
    pub async fn agent_count(&self) -> usize {
        self.agents.read().await.len()
    }

    /// Returns the number of running agents
    pub async fn running_count(&self) -> usize {
        self.agents
            .read()
            .await
            .values()
            .filter(|a| a.status().is_running())
            .count()
    }

    /// Triggers all agents that support it
    pub async fn trigger_all(&self) -> Result<(), AgentError> {
        let agents = self.agents.read().await.clone();

        for (_, agent) in agents {
            if let Err(e) = agent.trigger().await {
                tracing::warn!("Failed to trigger agent '{}': {}", agent.id(), e);
            }
        }

        Ok(())
    }

    /// Triggers a specific agent
    pub async fn trigger_agent(&self, id: &str) -> Result<(), AgentError> {
        let agent = self.agents.read().await.get(id).cloned();

        if let Some(agent) = agent {
            agent.trigger().await
        } else {
            Err(AgentError::OperationFailed(format!(
                "Agent '{}' not found",
                id
            )))
        }
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test agent
    struct TestAgent {
        id: &'static str,
        status: RwLock<AgentStatus>,
    }

    impl TestAgent {
        fn new(id: &'static str) -> Self {
            Self {
                id,
                status: RwLock::new(AgentStatus::Idle),
            }
        }
    }

    #[async_trait::async_trait]
    impl Agent for TestAgent {
        fn id(&self) -> &'static str {
            self.id
        }

        fn name(&self) -> &'static str {
            "Test Agent"
        }

        fn status(&self) -> AgentStatus {
            self.status
                .try_read()
                .map(|s| s.clone())
                .unwrap_or(AgentStatus::Idle)
        }

        async fn start(&self) -> Result<(), AgentError> {
            *self.status.write().await = AgentStatus::Running;
            // Simulate running
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if !self.status.read().await.is_running() {
                    break;
                }
            }
            Ok(())
        }

        async fn stop(&self) -> Result<(), AgentError> {
            *self.status.write().await = AgentStatus::Stopped;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_agent_manager_new() {
        let manager = AgentManager::new();
        assert_eq!(manager.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_agent_manager_register() {
        let manager = AgentManager::new();
        let agent = Arc::new(TestAgent::new("test-1"));

        manager.register(agent).await;

        assert_eq!(manager.agent_count().await, 1);
        assert!(manager.get("test-1").await.is_some());
    }

    #[tokio::test]
    async fn test_agent_manager_unregister() {
        let manager = AgentManager::new();
        let agent = Arc::new(TestAgent::new("test-1"));

        manager.register(agent).await;
        assert_eq!(manager.agent_count().await, 1);

        manager.unregister("test-1").await;
        assert_eq!(manager.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_agent_manager_status() {
        let manager = AgentManager::new();
        let agent = Arc::new(TestAgent::new("test-1"));

        manager.register(agent).await;

        let status = manager.status().await;
        assert_eq!(status.len(), 1);
        assert_eq!(status.get("test-1"), Some(&AgentStatus::Idle));
    }

    #[tokio::test]
    async fn test_agent_manager_agent_status() {
        let manager = AgentManager::new();
        let agent = Arc::new(TestAgent::new("test-1"));

        manager.register(agent).await;

        assert_eq!(
            manager.agent_status("test-1").await,
            Some(AgentStatus::Idle)
        );
        assert_eq!(manager.agent_status("nonexistent").await, None);
    }

    #[tokio::test]
    async fn test_agent_manager_running_count() {
        let manager = AgentManager::new();
        let agent1 = Arc::new(TestAgent::new("test-1"));
        let agent2 = Arc::new(TestAgent::new("test-2"));

        manager.register(agent1).await;
        manager.register(agent2).await;

        assert_eq!(manager.running_count().await, 0);
    }

    #[tokio::test]
    async fn test_agent_manager_get_nonexistent() {
        let manager = AgentManager::new();
        assert!(manager.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_agent_manager_stop_nonexistent() {
        let manager = AgentManager::new();
        // Should not error
        assert!(manager.stop_agent("nonexistent").await.is_ok());
    }
}
