//! Agent module - Background tasks and periodic operations
//!
//! Provides agents for:
//! - Periodic refresh of usage data
//! - Usage threshold notifications
//! - Cookie change monitoring

mod base;
mod manager;
mod refresh_agent;
mod notification_agent;

pub use base::{Agent, AgentError, AgentStatus};
pub use manager::AgentManager;
pub use refresh_agent::RefreshAgent;
pub use notification_agent::{NotificationAgent, NotificationThresholds};
