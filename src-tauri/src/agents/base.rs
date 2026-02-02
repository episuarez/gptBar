//! Base agent trait and types
//!
//! Defines the Agent trait that all background agents must implement.

use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur in agents
#[derive(Debug, Error)]
pub enum AgentError {
    /// Agent is already running
    #[error("Agent is already running")]
    AlreadyRunning,

    /// Agent is not running
    #[error("Agent is not running")]
    NotRunning,

    /// Agent operation failed
    #[error("Agent operation failed: {0}")]
    OperationFailed(String),

    /// Agent was cancelled
    #[error("Agent was cancelled")]
    Cancelled,

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Status of an agent
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent is idle and not running
    Idle,
    /// Agent is currently running
    Running,
    /// Agent encountered an error
    Error(String),
    /// Agent has been stopped
    Stopped,
}

impl AgentStatus {
    /// Returns true if the agent is running
    pub fn is_running(&self) -> bool {
        matches!(self, AgentStatus::Running)
    }

    /// Returns true if the agent is idle or stopped
    pub fn is_stopped(&self) -> bool {
        matches!(self, AgentStatus::Idle | AgentStatus::Stopped)
    }

    /// Returns true if the agent has an error
    pub fn has_error(&self) -> bool {
        matches!(self, AgentStatus::Error(_))
    }
}

/// Trait for background agents
///
/// Agents are long-running background tasks that can be started and stopped.
/// They typically perform periodic operations like refreshing data or
/// monitoring for changes.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Returns the unique identifier for this agent
    fn id(&self) -> &'static str;

    /// Returns the display name for this agent
    fn name(&self) -> &'static str;

    /// Returns the current status of the agent
    fn status(&self) -> AgentStatus;

    /// Starts the agent
    ///
    /// This should run the agent's main loop until cancelled.
    /// Implementations should use tokio::select! with a cancellation token.
    async fn start(&self) -> Result<(), AgentError>;

    /// Stops the agent
    ///
    /// Signals the agent to stop and waits for it to finish.
    async fn stop(&self) -> Result<(), AgentError>;

    /// Triggers an immediate action (if supported)
    ///
    /// For example, a refresh agent might immediately fetch new data.
    async fn trigger(&self) -> Result<(), AgentError> {
        Ok(()) // Default: no-op
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_is_running() {
        assert!(AgentStatus::Running.is_running());
        assert!(!AgentStatus::Idle.is_running());
        assert!(!AgentStatus::Stopped.is_running());
        assert!(!AgentStatus::Error("test".into()).is_running());
    }

    #[test]
    fn test_agent_status_is_stopped() {
        assert!(AgentStatus::Idle.is_stopped());
        assert!(AgentStatus::Stopped.is_stopped());
        assert!(!AgentStatus::Running.is_stopped());
        assert!(!AgentStatus::Error("test".into()).is_stopped());
    }

    #[test]
    fn test_agent_status_has_error() {
        assert!(AgentStatus::Error("test".into()).has_error());
        assert!(!AgentStatus::Idle.has_error());
        assert!(!AgentStatus::Running.has_error());
        assert!(!AgentStatus::Stopped.has_error());
    }

    #[test]
    fn test_agent_status_clone() {
        let status = AgentStatus::Error("error message".into());
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }
}
