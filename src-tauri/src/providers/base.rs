//! Base types and traits for AI providers
//!
//! Defines the core abstractions used by all providers following SOLID principles.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Represents a rate limit window with usage information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateWindow {
    /// Usage percentage (0.0 - 100.0)
    pub used_percent: f64,
    /// Duration of the window in minutes (e.g., 300 for 5-hour session)
    pub window_minutes: Option<i64>,
    /// When the window resets
    pub resets_at: Option<DateTime<Utc>>,
    /// Human-readable reset description
    pub reset_description: Option<String>,
}

impl RateWindow {
    /// Creates a new RateWindow with the given usage percentage
    pub fn new(used_percent: f64) -> Self {
        Self {
            used_percent,
            window_minutes: None,
            resets_at: None,
            reset_description: None,
        }
    }

    /// Sets the window duration in minutes
    pub fn with_window_minutes(mut self, minutes: i64) -> Self {
        self.window_minutes = Some(minutes);
        self
    }

    /// Sets the reset time
    pub fn with_resets_at(mut self, resets_at: DateTime<Utc>) -> Self {
        self.resets_at = Some(resets_at);
        self
    }

    /// Sets the reset description
    pub fn with_reset_description(mut self, description: impl Into<String>) -> Self {
        self.reset_description = Some(description.into());
        self
    }

    /// Returns true if usage is at warning level (>= 80%)
    pub fn is_warning(&self) -> bool {
        self.used_percent >= 80.0
    }

    /// Returns true if usage is at critical level (>= 95%)
    pub fn is_critical(&self) -> bool {
        self.used_percent >= 95.0
    }
}

impl Default for RateWindow {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// Identity information for a provider account
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct IdentitySnapshot {
    /// User email (may be sanitized for display)
    pub email: Option<String>,
    /// Subscription plan (e.g., "pro", "free", "team")
    pub plan: Option<String>,
    /// Organization name if applicable
    pub organization: Option<String>,
}

impl IdentitySnapshot {
    /// Creates a new IdentitySnapshot
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the email
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Sets the plan
    pub fn with_plan(mut self, plan: impl Into<String>) -> Self {
        self.plan = Some(plan.into());
        self
    }

    /// Sets the organization
    pub fn with_organization(mut self, org: impl Into<String>) -> Self {
        self.organization = Some(org.into());
        self
    }
}

/// A snapshot of usage data from a provider
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageSnapshot {
    /// Primary usage window (typically 5-hour session limit)
    pub primary: Option<RateWindow>,
    /// Secondary usage window (typically weekly limit)
    pub secondary: Option<RateWindow>,
    /// Tertiary usage window (model-specific limits like Opus)
    pub tertiary: Option<RateWindow>,
    /// When this snapshot was captured
    pub updated_at: DateTime<Utc>,
    /// Account identity information
    pub identity: Option<IdentitySnapshot>,
}

impl UsageSnapshot {
    /// Creates a new UsageSnapshot with the current timestamp
    pub fn new() -> Self {
        Self {
            primary: None,
            secondary: None,
            tertiary: None,
            updated_at: Utc::now(),
            identity: None,
        }
    }

    /// Sets the primary rate window
    pub fn with_primary(mut self, window: RateWindow) -> Self {
        self.primary = Some(window);
        self
    }

    /// Sets the secondary rate window
    pub fn with_secondary(mut self, window: RateWindow) -> Self {
        self.secondary = Some(window);
        self
    }

    /// Sets the tertiary rate window
    pub fn with_tertiary(mut self, window: RateWindow) -> Self {
        self.tertiary = Some(window);
        self
    }

    /// Sets the identity information
    pub fn with_identity(mut self, identity: IdentitySnapshot) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Returns the highest usage percentage across all windows
    pub fn max_usage(&self) -> f64 {
        [
            self.primary.as_ref().map(|w| w.used_percent),
            self.secondary.as_ref().map(|w| w.used_percent),
            self.tertiary.as_ref().map(|w| w.used_percent),
        ]
        .into_iter()
        .flatten()
        .fold(0.0, f64::max)
    }

    /// Returns true if any window is at warning level
    pub fn has_warning(&self) -> bool {
        self.primary.as_ref().map_or(false, |w| w.is_warning())
            || self.secondary.as_ref().map_or(false, |w| w.is_warning())
            || self.tertiary.as_ref().map_or(false, |w| w.is_warning())
    }

    /// Returns true if any window is at critical level
    pub fn has_critical(&self) -> bool {
        self.primary.as_ref().map_or(false, |w| w.is_critical())
            || self.secondary.as_ref().map_or(false, |w| w.is_critical())
            || self.tertiary.as_ref().map_or(false, |w| w.is_critical())
    }
}

impl Default for UsageSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when fetching provider data
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Authentication is required but not available
    #[error("Authentication required")]
    AuthRequired,

    /// Authentication credentials are invalid or expired
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Network error during fetch
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Error parsing response data
    #[error("Parse error: {0}")]
    Parse(String),

    /// Error extracting cookies from browser
    #[error("Cookie extraction failed: {0}")]
    CookieExtraction(String),

    /// Error with secure storage
    #[error("Storage error: {0}")]
    Storage(String),

    /// Provider is not available or disabled
    #[error("Provider not available: {0}")]
    NotAvailable(String),

    /// Generic internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Authentication method for a provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    /// OAuth 2.0 token
    OAuth,
    /// Browser cookies
    Cookie,
    /// CLI-based authentication
    Cli,
    /// API token
    ApiToken,
    /// No authentication required
    None,
}

/// Result of a fetch operation
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// The usage snapshot if successful
    pub snapshot: UsageSnapshot,
    /// The authentication method that was used
    pub auth_method: AuthMethod,
    /// Whether this is cached data
    pub is_cached: bool,
}

/// Trait that all AI providers must implement
///
/// This follows the Interface Segregation Principle - providers only need
/// to implement what they support.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Returns the unique identifier for this provider (e.g., "claude")
    fn id(&self) -> &'static str;

    /// Returns the display name for this provider (e.g., "Claude")
    fn name(&self) -> &'static str;

    /// Returns whether this provider is currently enabled
    fn is_enabled(&self) -> bool;

    /// Returns whether this provider supports login flow
    fn supports_login(&self) -> bool {
        true
    }

    /// Fetches the current usage data from the provider
    ///
    /// Implementations should try multiple auth methods in order of preference:
    /// 1. OAuth (if available)
    /// 2. Cookies (if available)
    /// 3. CLI (if available)
    async fn fetch(&self) -> Result<UsageSnapshot, ProviderError>;

    /// Initiates the login flow for this provider
    ///
    /// Returns true if login was successful, false if cancelled
    async fn login(&self) -> Result<bool, ProviderError>;

    /// Logs out from this provider, clearing stored credentials
    async fn logout(&self) -> Result<(), ProviderError>;

    /// Checks if authentication is available for this provider
    async fn is_available(&self) -> bool;

    /// Returns the preferred authentication methods in order of preference
    fn auth_methods(&self) -> Vec<AuthMethod> {
        vec![AuthMethod::OAuth, AuthMethod::Cookie, AuthMethod::Cli]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_window_new() {
        let window = RateWindow::new(45.5);
        assert_eq!(window.used_percent, 45.5);
        assert!(window.window_minutes.is_none());
        assert!(window.resets_at.is_none());
    }

    #[test]
    fn test_rate_window_builder() {
        let resets = Utc::now();
        let window = RateWindow::new(75.0)
            .with_window_minutes(300)
            .with_resets_at(resets)
            .with_reset_description("Resets in 2 hours");

        assert_eq!(window.used_percent, 75.0);
        assert_eq!(window.window_minutes, Some(300));
        assert_eq!(window.resets_at, Some(resets));
        assert_eq!(window.reset_description, Some("Resets in 2 hours".into()));
    }

    #[test]
    fn test_rate_window_warning_levels() {
        assert!(!RateWindow::new(79.9).is_warning());
        assert!(RateWindow::new(80.0).is_warning());
        assert!(RateWindow::new(85.0).is_warning());

        assert!(!RateWindow::new(94.9).is_critical());
        assert!(RateWindow::new(95.0).is_critical());
        assert!(RateWindow::new(100.0).is_critical());
    }

    #[test]
    fn test_usage_snapshot_new() {
        let snapshot = UsageSnapshot::new();
        assert!(snapshot.primary.is_none());
        assert!(snapshot.secondary.is_none());
        assert!(snapshot.tertiary.is_none());
        assert!(snapshot.identity.is_none());
    }

    #[test]
    fn test_usage_snapshot_builder() {
        let primary = RateWindow::new(45.5);
        let secondary = RateWindow::new(12.0);
        let identity = IdentitySnapshot::new()
            .with_email("test@example.com")
            .with_plan("pro");

        let snapshot = UsageSnapshot::new()
            .with_primary(primary.clone())
            .with_secondary(secondary.clone())
            .with_identity(identity.clone());

        assert_eq!(snapshot.primary, Some(primary));
        assert_eq!(snapshot.secondary, Some(secondary));
        assert_eq!(snapshot.identity, Some(identity));
    }

    #[test]
    fn test_usage_snapshot_max_usage() {
        let snapshot = UsageSnapshot::new()
            .with_primary(RateWindow::new(45.0))
            .with_secondary(RateWindow::new(80.0))
            .with_tertiary(RateWindow::new(30.0));

        assert_eq!(snapshot.max_usage(), 80.0);
    }

    #[test]
    fn test_usage_snapshot_warning_detection() {
        let normal = UsageSnapshot::new()
            .with_primary(RateWindow::new(50.0))
            .with_secondary(RateWindow::new(60.0));

        assert!(!normal.has_warning());
        assert!(!normal.has_critical());

        let warning = UsageSnapshot::new()
            .with_primary(RateWindow::new(85.0))
            .with_secondary(RateWindow::new(60.0));

        assert!(warning.has_warning());
        assert!(!warning.has_critical());

        let critical = UsageSnapshot::new()
            .with_primary(RateWindow::new(50.0))
            .with_secondary(RateWindow::new(98.0));

        assert!(critical.has_warning()); // 98% is also warning
        assert!(critical.has_critical());
    }

    #[test]
    fn test_identity_snapshot_builder() {
        let identity = IdentitySnapshot::new()
            .with_email("user@example.com")
            .with_plan("team")
            .with_organization("Acme Corp");

        assert_eq!(identity.email, Some("user@example.com".into()));
        assert_eq!(identity.plan, Some("team".into()));
        assert_eq!(identity.organization, Some("Acme Corp".into()));
    }

    #[test]
    fn test_rate_window_serialization() {
        let window = RateWindow::new(55.5).with_window_minutes(300);
        let json = serde_json::to_string(&window).unwrap();
        let deserialized: RateWindow = serde_json::from_str(&json).unwrap();

        assert_eq!(window, deserialized);
    }

    #[test]
    fn test_usage_snapshot_serialization() {
        let snapshot = UsageSnapshot::new()
            .with_primary(RateWindow::new(45.5))
            .with_identity(IdentitySnapshot::new().with_email("test@test.com"));

        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: UsageSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot.primary, deserialized.primary);
        assert_eq!(snapshot.identity, deserialized.identity);
    }
}
