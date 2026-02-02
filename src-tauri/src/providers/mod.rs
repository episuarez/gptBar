//! Provider module - Defines the Provider trait and usage types
//!
//! This module follows SOLID principles:
//! - Single Responsibility: Each provider handles only one AI service
//! - Open/Closed: New providers can be added without modifying existing code
//! - Liskov Substitution: Any Provider implementation is interchangeable
//! - Interface Segregation: Small, focused traits
//! - Dependency Inversion: Depends on abstractions (Provider trait)

mod base;
mod claude;
mod codex;
mod gemini;
mod openai;

pub use base::*;
pub use claude::ClaudeProvider;
pub use codex::CodexProvider;
pub use gemini::GeminiProvider;
pub use openai::OpenAIProvider;

use std::collections::HashMap;
use std::sync::Arc;

/// Registry of all available providers
pub struct ProviderRegistry {
    providers: HashMap<&'static str, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    /// Creates a new registry with all providers
    pub fn new() -> Self {
        let mut providers: HashMap<&'static str, Arc<dyn Provider>> = HashMap::new();

        providers.insert("claude", Arc::new(ClaudeProvider::new()));
        providers.insert("openai", Arc::new(OpenAIProvider::new()));
        providers.insert("gemini", Arc::new(GeminiProvider::new()));
        providers.insert("codex", Arc::new(CodexProvider::new()));

        Self { providers }
    }

    /// Gets a provider by ID
    pub fn get(&self, id: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(id).cloned()
    }

    /// Gets all provider IDs
    pub fn provider_ids(&self) -> Vec<&'static str> {
        self.providers.keys().copied().collect()
    }

    /// Gets all providers
    pub fn all(&self) -> Vec<Arc<dyn Provider>> {
        self.providers.values().cloned().collect()
    }

    /// Gets provider metadata for all providers
    pub fn metadata(&self) -> Vec<ProviderMetadata> {
        self.providers
            .values()
            .map(|p| ProviderMetadata {
                id: p.id().to_string(),
                name: p.name().to_string(),
                supports_login: p.supports_login(),
                auth_methods: p.auth_methods(),
            })
            .collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata about a provider (serializable)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderMetadata {
    pub id: String,
    pub name: String,
    pub supports_login: bool,
    pub auth_methods: Vec<AuthMethod>,
}
