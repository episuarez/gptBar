//! Secure token storage using Windows Credential Manager
//!
//! Provides secure storage for OAuth tokens, API keys, and other credentials
//! using the Windows Credential Manager (accessed via the keyring crate).

use keyring::Entry;
use thiserror::Error;

/// Errors that can occur during secure storage operations
#[derive(Debug, Error)]
pub enum SecureStoreError {
    /// Keyring operation failed
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    /// Token not found
    #[error("Token not found for key: {0}")]
    NotFound(String),

    /// Invalid data format
    #[error("Invalid data format: {0}")]
    InvalidFormat(String),
}

/// Secure storage for tokens and credentials
///
/// Uses Windows Credential Manager to store sensitive data securely.
/// Data is tied to the current Windows user account.
///
/// # Example
///
/// ```no_run
/// use gptbar_lib::auth::SecureStore;
///
/// let store = SecureStore::new();
///
/// // Store a token
/// store.set_token("claude-oauth", "my-token").unwrap();
///
/// // Retrieve it later
/// let token = store.get_token("claude-oauth").unwrap();
/// assert_eq!(token, Some("my-token".to_string()));
///
/// // Delete when no longer needed
/// store.delete_token("claude-oauth").unwrap();
/// ```
pub struct SecureStore {
    service: &'static str,
}

impl SecureStore {
    /// Creates a new SecureStore with the default service name
    pub fn new() -> Self {
        Self {
            service: "GPTBar",
        }
    }

    /// Creates a new SecureStore with a custom service name
    ///
    /// Useful for testing or separating different credential sets.
    pub fn with_service(service: &'static str) -> Self {
        Self { service }
    }

    /// Returns the service name used for this store
    pub fn service(&self) -> &str {
        self.service
    }

    /// Stores a token securely
    ///
    /// # Arguments
    ///
    /// * `key` - Identifier for the token (e.g., "claude-oauth", "copilot-token")
    /// * `token` - The secret token value to store
    pub fn set_token(&self, key: &str, token: &str) -> Result<(), SecureStoreError> {
        let entry = Entry::new(self.service, key)?;
        entry.set_password(token)?;
        Ok(())
    }

    /// Retrieves a stored token
    ///
    /// # Arguments
    ///
    /// * `key` - Identifier for the token
    ///
    /// # Returns
    ///
    /// `Some(token)` if found, `None` if not stored
    pub fn get_token(&self, key: &str) -> Result<Option<String>, SecureStoreError> {
        let entry = Entry::new(self.service, key)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecureStoreError::Keyring(e)),
        }
    }

    /// Deletes a stored token
    ///
    /// # Arguments
    ///
    /// * `key` - Identifier for the token to delete
    ///
    /// # Returns
    ///
    /// `Ok(true)` if deleted, `Ok(false)` if not found
    pub fn delete_token(&self, key: &str) -> Result<bool, SecureStoreError> {
        let entry = Entry::new(self.service, key)?;
        match entry.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(SecureStoreError::Keyring(e)),
        }
    }

    /// Checks if a token exists
    ///
    /// # Arguments
    ///
    /// * `key` - Identifier for the token
    pub fn has_token(&self, key: &str) -> Result<bool, SecureStoreError> {
        Ok(self.get_token(key)?.is_some())
    }

    /// Stores a token only if it doesn't already exist
    ///
    /// # Returns
    ///
    /// `true` if the token was stored (didn't exist), `false` if it already existed
    pub fn set_token_if_absent(
        &self,
        key: &str,
        token: &str,
    ) -> Result<bool, SecureStoreError> {
        if self.has_token(key)? {
            Ok(false)
        } else {
            self.set_token(key, token)?;
            Ok(true)
        }
    }

    /// Updates a token only if it already exists
    ///
    /// # Returns
    ///
    /// `true` if the token was updated (existed), `false` if it didn't exist
    pub fn update_token(&self, key: &str, token: &str) -> Result<bool, SecureStoreError> {
        if self.has_token(key)? {
            self.set_token(key, token)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lists all known token keys for this service
    ///
    /// Note: This is a predefined list of known keys, not a dynamic enumeration
    /// (Windows Credential Manager doesn't support listing by service).
    pub fn known_keys() -> &'static [&'static str] {
        &[
            "claude-oauth",
            "claude-cookie",
            "copilot-token",
            "cursor-cookie",
            "gemini-token",
        ]
    }

    /// Clears all known tokens for this service
    pub fn clear_all(&self) -> Result<(), SecureStoreError> {
        for key in Self::known_keys() {
            let _ = self.delete_token(key);
        }
        Ok(())
    }
}

impl Default for SecureStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use a test-specific service to avoid conflicts
    fn test_store() -> SecureStore {
        SecureStore::with_service("GPTBar-Test")
    }

    #[test]
    fn test_store_creation() {
        let store = SecureStore::new();
        assert_eq!(store.service(), "GPTBar");

        let custom = SecureStore::with_service("Custom");
        assert_eq!(custom.service(), "Custom");
    }

    #[test]
    fn test_store_and_retrieve() {
        let store = test_store();
        let test_key = "test-token-1";
        let test_value = "super-secret-value";

        // Clean up any previous test data
        let _ = store.delete_token(test_key);

        // Store
        store.set_token(test_key, test_value).unwrap();

        // Retrieve
        let retrieved = store.get_token(test_key).unwrap();
        assert_eq!(retrieved, Some(test_value.to_string()));

        // Clean up
        store.delete_token(test_key).unwrap();
    }

    #[test]
    fn test_get_nonexistent() {
        let store = test_store();
        let result = store.get_token("nonexistent-key-12345").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_delete_nonexistent() {
        let store = test_store();
        let result = store.delete_token("nonexistent-key-67890").unwrap();
        assert!(!result);
    }

    #[test]
    fn test_has_token() {
        let store = test_store();
        let test_key = "test-has-token";

        // Clean up
        let _ = store.delete_token(test_key);

        // Should not exist
        assert!(!store.has_token(test_key).unwrap());

        // Store and check
        store.set_token(test_key, "value").unwrap();
        assert!(store.has_token(test_key).unwrap());

        // Clean up
        store.delete_token(test_key).unwrap();
    }

    #[test]
    fn test_set_if_absent() {
        let store = test_store();
        let test_key = "test-if-absent";

        // Clean up
        let _ = store.delete_token(test_key);

        // First set should succeed
        assert!(store.set_token_if_absent(test_key, "first").unwrap());
        assert_eq!(store.get_token(test_key).unwrap(), Some("first".to_string()));

        // Second set should fail (already exists)
        assert!(!store.set_token_if_absent(test_key, "second").unwrap());
        // Value should still be "first"
        assert_eq!(store.get_token(test_key).unwrap(), Some("first".to_string()));

        // Clean up
        store.delete_token(test_key).unwrap();
    }

    #[test]
    fn test_update_token() {
        let store = test_store();
        let test_key = "test-update";

        // Clean up
        let _ = store.delete_token(test_key);

        // Update non-existent should return false
        assert!(!store.update_token(test_key, "value").unwrap());

        // Create it
        store.set_token(test_key, "original").unwrap();

        // Update should succeed
        assert!(store.update_token(test_key, "updated").unwrap());
        assert_eq!(store.get_token(test_key).unwrap(), Some("updated".to_string()));

        // Clean up
        store.delete_token(test_key).unwrap();
    }

    #[test]
    fn test_overwrite() {
        let store = test_store();
        let test_key = "test-overwrite";

        // Clean up
        let _ = store.delete_token(test_key);

        // Store initial value
        store.set_token(test_key, "first").unwrap();
        assert_eq!(store.get_token(test_key).unwrap(), Some("first".to_string()));

        // Overwrite
        store.set_token(test_key, "second").unwrap();
        assert_eq!(store.get_token(test_key).unwrap(), Some("second".to_string()));

        // Clean up
        store.delete_token(test_key).unwrap();
    }

    #[test]
    fn test_known_keys() {
        let keys = SecureStore::known_keys();
        assert!(keys.contains(&"claude-oauth"));
        assert!(keys.contains(&"copilot-token"));
    }
}
