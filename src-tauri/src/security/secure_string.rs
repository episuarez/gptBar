//! Secure string handling with automatic memory zeroization
//!
//! Provides a string type that automatically zeroes its memory when dropped,
//! preventing sensitive data from lingering in memory.

use std::fmt;
use std::ops::Deref;
use zeroize::Zeroize;

/// A string that securely clears its memory when dropped
///
/// This is useful for storing sensitive data like tokens, passwords, or API keys
/// that should not linger in memory after use.
///
/// # Example
///
/// ```
/// use gptbar_lib::security::SecureString;
///
/// let secret = SecureString::new("my-secret-token".to_string());
/// // Use the secret...
/// assert_eq!(secret.as_str(), "my-secret-token");
/// // When dropped, memory is securely zeroed
/// ```
#[derive(Clone)]
pub struct SecureString {
    inner: String,
}

impl SecureString {
    /// Creates a new SecureString from a String
    ///
    /// The original String is consumed and its memory will be zeroed when
    /// this SecureString is dropped.
    pub fn new(s: String) -> Self {
        Self { inner: s }
    }

    /// Creates a new SecureString from a string slice
    ///
    /// Note: This creates a copy of the data. If you already have a String,
    /// prefer using `new()` to avoid an extra copy.
    pub fn from_str(s: &str) -> Self {
        Self {
            inner: s.to_string(),
        }
    }

    /// Returns the string as a slice
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Returns the length of the string in bytes
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the string is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Consumes the SecureString and returns the inner String
    ///
    /// # Warning
    ///
    /// This defeats the purpose of SecureString as the returned String
    /// will not be automatically zeroed. Only use this when absolutely necessary.
    pub fn into_inner(mut self) -> String {
        std::mem::take(&mut self.inner)
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        self.inner.zeroize();
    }
}

impl Zeroize for SecureString {
    fn zeroize(&mut self) {
        self.inner.zeroize();
    }
}

impl Deref for SecureString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AsRef<str> for SecureString {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl From<String> for SecureString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecureString {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

// Intentionally NOT implementing Display or Debug to prevent accidental logging
impl fmt::Debug for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecureString")
            .field("len", &self.inner.len())
            .field("content", &"[REDACTED]")
            .finish()
    }
}

impl PartialEq for SecureString {
    fn eq(&self, other: &Self) -> bool {
        // Use constant-time comparison to prevent timing attacks
        constant_time_eq(self.inner.as_bytes(), other.inner.as_bytes())
    }
}

impl Eq for SecureString {}

impl PartialEq<str> for SecureString {
    fn eq(&self, other: &str) -> bool {
        constant_time_eq(self.inner.as_bytes(), other.as_bytes())
    }
}

impl PartialEq<&str> for SecureString {
    fn eq(&self, other: &&str) -> bool {
        constant_time_eq(self.inner.as_bytes(), other.as_bytes())
    }
}

impl PartialEq<String> for SecureString {
    fn eq(&self, other: &String) -> bool {
        constant_time_eq(self.inner.as_bytes(), other.as_bytes())
    }
}

/// Constant-time byte comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// A vector that securely clears its memory when dropped
///
/// Useful for storing sensitive byte data like encryption keys.
#[derive(Clone)]
#[allow(dead_code)]
pub struct SecureBytes {
    inner: Vec<u8>,
}

#[allow(dead_code)]
impl SecureBytes {
    /// Creates a new SecureBytes from a Vec<u8>
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { inner: bytes }
    }

    /// Creates a new SecureBytes from a byte slice
    pub fn from_slice(bytes: &[u8]) -> Self {
        Self {
            inner: bytes.to_vec(),
        }
    }

    /// Returns the bytes as a slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Returns the length in bytes
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        self.inner.zeroize();
    }
}

impl Zeroize for SecureBytes {
    fn zeroize(&mut self) {
        self.inner.zeroize();
    }
}

impl Deref for SecureBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AsRef<[u8]> for SecureBytes {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl From<Vec<u8>> for SecureBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl From<&[u8]> for SecureBytes {
    fn from(bytes: &[u8]) -> Self {
        Self::from_slice(bytes)
    }
}

impl fmt::Debug for SecureBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecureBytes")
            .field("len", &self.inner.len())
            .field("content", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_string_new() {
        let secret = SecureString::new("password123".to_string());
        assert_eq!(secret.as_str(), "password123");
        assert_eq!(secret.len(), 11);
        assert!(!secret.is_empty());
    }

    #[test]
    fn test_secure_string_from_str() {
        let secret = SecureString::from_str("api-key");
        assert_eq!(secret.as_str(), "api-key");
    }

    #[test]
    fn test_secure_string_from_trait() {
        let secret: SecureString = "token".into();
        assert_eq!(secret.as_str(), "token");

        let secret: SecureString = String::from("token").into();
        assert_eq!(secret.as_str(), "token");
    }

    #[test]
    fn test_secure_string_deref() {
        let secret = SecureString::new("hello".to_string());
        // Can use string methods via Deref
        assert!(secret.starts_with("hel"));
        assert!(secret.contains("ell"));
    }

    #[test]
    fn test_secure_string_as_ref() {
        let secret = SecureString::new("test".to_string());
        let s: &str = secret.as_ref();
        assert_eq!(s, "test");
    }

    #[test]
    fn test_secure_string_debug_redacted() {
        let secret = SecureString::new("super-secret".to_string());
        let debug_output = format!("{:?}", secret);
        assert!(!debug_output.contains("super-secret"));
        assert!(debug_output.contains("REDACTED"));
        assert!(debug_output.contains("len"));
    }

    #[test]
    fn test_secure_string_equality() {
        let s1 = SecureString::new("same".to_string());
        let s2 = SecureString::new("same".to_string());
        let s3 = SecureString::new("different".to_string());

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_secure_string_eq_str() {
        let secret = SecureString::new("password".to_string());
        assert!(secret == "password");
        assert!(secret == String::from("password"));
        assert!(secret != "other");
    }

    #[test]
    fn test_secure_string_clone() {
        let original = SecureString::new("secret".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_secure_string_empty() {
        let empty = SecureString::new(String::new());
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_secure_bytes_new() {
        let bytes = SecureBytes::new(vec![1, 2, 3, 4]);
        assert_eq!(bytes.as_bytes(), &[1, 2, 3, 4]);
        assert_eq!(bytes.len(), 4);
    }

    #[test]
    fn test_secure_bytes_from_slice() {
        let bytes = SecureBytes::from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(bytes.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_secure_bytes_debug_redacted() {
        let bytes = SecureBytes::new(vec![1, 2, 3]);
        let debug_output = format!("{:?}", bytes);
        assert!(!debug_output.contains("1"));
        assert!(debug_output.contains("REDACTED"));
    }

    #[test]
    fn test_constant_time_eq_same() {
        let a = b"hello";
        let b = b"hello";
        assert!(constant_time_eq(a, b));
    }

    #[test]
    fn test_constant_time_eq_different() {
        let a = b"hello";
        let b = b"world";
        assert!(!constant_time_eq(a, b));
    }

    #[test]
    fn test_constant_time_eq_different_lengths() {
        let a = b"short";
        let b = b"longer string";
        assert!(!constant_time_eq(a, b));
    }

    #[test]
    fn test_secure_string_zeroize() {
        let mut secret = SecureString::new("secret".to_string());
        secret.zeroize();
        assert!(secret.is_empty());
    }

    #[test]
    fn test_secure_bytes_zeroize() {
        let mut bytes = SecureBytes::new(vec![1, 2, 3]);
        bytes.zeroize();
        assert!(bytes.is_empty());
    }
}
