//! Browser cookie extraction for authentication
//!
//! Extracts cookies from Chrome, Edge, and Firefox browsers to enable
//! authentication with web-based AI services.

use rusqlite::Connection;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during cookie extraction
#[derive(Debug, Error)]
pub enum CookieError {
    /// Browser database not found
    #[error("Cookie database not found for {browser}: {path}")]
    DatabaseNotFound { browser: String, path: String },

    /// Database access error
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Cookie decryption failed
    #[error("Cookie decryption failed: {0}")]
    Decryption(String),

    /// No cookies found for domain
    #[error("No cookies found for domain: {0}")]
    NoCookiesFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Environment variable not set
    #[error("Environment variable not set: {0}")]
    EnvVar(String),
}

/// Supported browser types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserType {
    /// Google Chrome
    Chrome,
    /// Microsoft Edge
    Edge,
    /// Mozilla Firefox
    Firefox,
}

impl BrowserType {
    /// Returns the display name of the browser
    pub fn name(&self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Edge => "Edge",
            Self::Firefox => "Firefox",
        }
    }

    /// Returns all supported browser types in preference order
    pub fn all() -> &'static [BrowserType] {
        &[Self::Chrome, Self::Edge, Self::Firefox]
    }
}

/// A single cookie extracted from a browser
#[derive(Debug, Clone)]
pub struct Cookie {
    /// Cookie name
    pub name: String,
    /// Cookie value (decrypted)
    pub value: String,
    /// Domain the cookie belongs to
    pub domain: String,
    /// Path the cookie applies to
    pub path: String,
    /// Expiration timestamp (Unix epoch)
    pub expires: Option<i64>,
    /// Whether the cookie is secure-only
    pub secure: bool,
    /// Whether the cookie is HTTP-only
    pub http_only: bool,
}

impl Cookie {
    /// Formats the cookie for use in an HTTP Cookie header
    pub fn to_header_value(&self) -> String {
        format!("{}={}", self.name, self.value)
    }
}

/// Cookie extractor for Windows browsers
///
/// Extracts cookies from Chrome, Edge, and Firefox browsers.
/// On Windows, Chrome and Edge cookies are encrypted using DPAPI.
pub struct CookieExtractor;

impl CookieExtractor {
    /// Creates a new CookieExtractor
    pub fn new() -> Self {
        Self
    }

    /// Returns the cookie database path for a browser
    pub fn cookie_path(browser: BrowserType) -> Result<PathBuf, CookieError> {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| CookieError::EnvVar("LOCALAPPDATA".into()))?;
        let app_data = std::env::var("APPDATA")
            .map_err(|_| CookieError::EnvVar("APPDATA".into()))?;

        let path = match browser {
            BrowserType::Chrome => PathBuf::from(&local_app_data)
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join("Default")
                .join("Network")
                .join("Cookies"),
            BrowserType::Edge => PathBuf::from(&local_app_data)
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join("Default")
                .join("Network")
                .join("Cookies"),
            BrowserType::Firefox => {
                // Firefox uses a profile directory
                let profiles_dir = PathBuf::from(&app_data)
                    .join("Mozilla")
                    .join("Firefox")
                    .join("Profiles");

                // Find the default profile (ends with .default or .default-release)
                if profiles_dir.exists() {
                    for entry in std::fs::read_dir(&profiles_dir)? {
                        let entry = entry?;
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if name_str.ends_with(".default") || name_str.ends_with(".default-release")
                        {
                            return Ok(entry.path().join("cookies.sqlite"));
                        }
                    }
                }
                return Err(CookieError::DatabaseNotFound {
                    browser: "Firefox".into(),
                    path: profiles_dir.to_string_lossy().into(),
                });
            }
        };

        Ok(path)
    }

    /// Checks if a browser has cookies available
    pub fn is_browser_available(browser: BrowserType) -> bool {
        Self::cookie_path(browser)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    /// Returns the first available browser
    pub fn first_available_browser() -> Option<BrowserType> {
        BrowserType::all()
            .iter()
            .copied()
            .find(|&b| Self::is_browser_available(b))
    }

    /// Extracts cookies for a domain from a specific browser
    ///
    /// # Arguments
    ///
    /// * `browser` - The browser to extract from
    /// * `domain` - The domain to get cookies for (e.g., "claude.ai")
    ///
    /// # Returns
    ///
    /// A list of cookies for the domain
    pub fn extract_cookies(
        &self,
        browser: BrowserType,
        domain: &str,
    ) -> Result<Vec<Cookie>, CookieError> {
        let db_path = Self::cookie_path(browser)?;

        if !db_path.exists() {
            return Err(CookieError::DatabaseNotFound {
                browser: browser.name().into(),
                path: db_path.to_string_lossy().into(),
            });
        }

        // Chrome/Edge lock the database, so we need to copy it first
        let temp_path = self.copy_database_if_locked(&db_path)?;
        let db_path_to_use = temp_path.as_ref().unwrap_or(&db_path);

        let cookies = match browser {
            BrowserType::Chrome | BrowserType::Edge => {
                self.extract_chromium_cookies(db_path_to_use, domain)?
            }
            BrowserType::Firefox => self.extract_firefox_cookies(db_path_to_use, domain)?,
        };

        // Clean up temp file
        if let Some(temp) = temp_path {
            let _ = std::fs::remove_file(temp);
        }

        if cookies.is_empty() {
            return Err(CookieError::NoCookiesFound(domain.into()));
        }

        Ok(cookies)
    }

    /// Extracts cookies from any available browser
    ///
    /// Tries browsers in order of preference: Chrome, Edge, Firefox
    pub fn extract_cookies_any_browser(&self, domain: &str) -> Result<Vec<Cookie>, CookieError> {
        for browser in BrowserType::all() {
            match self.extract_cookies(*browser, domain) {
                Ok(cookies) => return Ok(cookies),
                Err(_) => continue,
            }
        }
        Err(CookieError::NoCookiesFound(domain.into()))
    }

    /// Formats cookies as a Cookie header value
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gptbar_lib::auth::{CookieExtractor, BrowserType};
    ///
    /// let extractor = CookieExtractor::new();
    /// let cookies = extractor.extract_cookies(BrowserType::Chrome, "claude.ai").unwrap();
    /// let header = CookieExtractor::format_cookie_header(&cookies);
    /// // header = "sessionKey=abc123; userId=xyz789"
    /// ```
    pub fn format_cookie_header(cookies: &[Cookie]) -> String {
        cookies
            .iter()
            .map(|c| c.to_header_value())
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Copies the database if it's locked by the browser
    fn copy_database_if_locked(&self, path: &PathBuf) -> Result<Option<PathBuf>, CookieError> {
        // Try to open directly first
        if Connection::open(path).is_ok() {
            return Ok(None);
        }

        // Copy to temp directory
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("gptbar_cookies_{}.db", std::process::id()));

        std::fs::copy(path, &temp_path)?;

        Ok(Some(temp_path))
    }

    /// Extracts cookies from Chrome/Edge database
    fn extract_chromium_cookies(
        &self,
        db_path: &PathBuf,
        domain: &str,
    ) -> Result<Vec<Cookie>, CookieError> {
        let conn = Connection::open(db_path)?;

        // Chrome uses host_key column
        let mut stmt = conn.prepare(
            "SELECT name, encrypted_value, host_key, path, expires_utc, is_secure, is_httponly
             FROM cookies
             WHERE host_key LIKE ?1 OR host_key LIKE ?2",
        )?;

        let domain_pattern = format!("%{}", domain);
        let subdomain_pattern = format!(".{}", domain);

        let rows = stmt.query_map([&domain_pattern, &subdomain_pattern], |row| {
            Ok((
                row.get::<_, String>(0)?,           // name
                row.get::<_, Vec<u8>>(1)?,          // encrypted_value
                row.get::<_, String>(2)?,           // host_key (domain)
                row.get::<_, String>(3)?,           // path
                row.get::<_, Option<i64>>(4)?,      // expires_utc
                row.get::<_, bool>(5)?,             // is_secure
                row.get::<_, bool>(6)?,             // is_httponly
            ))
        })?;

        let mut cookies = Vec::new();
        for row_result in rows {
            let (name, encrypted_value, host_key, path, expires, secure, http_only) = row_result?;

            // Decrypt the cookie value using DPAPI
            let value = self.decrypt_chromium_cookie(&encrypted_value)?;

            cookies.push(Cookie {
                name,
                value,
                domain: host_key,
                path,
                expires,
                secure,
                http_only,
            });
        }

        Ok(cookies)
    }

    /// Extracts cookies from Firefox database
    fn extract_firefox_cookies(
        &self,
        db_path: &PathBuf,
        domain: &str,
    ) -> Result<Vec<Cookie>, CookieError> {
        let conn = Connection::open(db_path)?;

        // Firefox uses baseDomain column and stores values in plaintext
        let mut stmt = conn.prepare(
            "SELECT name, value, host, path, expiry, isSecure, isHttpOnly
             FROM moz_cookies
             WHERE host LIKE ?1 OR host LIKE ?2",
        )?;

        let domain_pattern = format!("%{}", domain);
        let subdomain_pattern = format!(".{}", domain);

        let rows = stmt.query_map([&domain_pattern, &subdomain_pattern], |row| {
            Ok(Cookie {
                name: row.get(0)?,
                value: row.get(1)?,
                domain: row.get(2)?,
                path: row.get(3)?,
                expires: row.get(4)?,
                secure: row.get(5)?,
                http_only: row.get(6)?,
            })
        })?;

        let mut cookies = Vec::new();
        for cookie_result in rows {
            cookies.push(cookie_result?);
        }

        Ok(cookies)
    }

    /// Decrypts a Chrome/Edge cookie value using DPAPI or AES-GCM
    #[cfg(windows)]
    fn decrypt_chromium_cookie(&self, encrypted: &[u8]) -> Result<String, CookieError> {
        // Chrome cookies start with "v10" or "v11" prefix for newer encryption
        // Older cookies use plain DPAPI

        if encrypted.is_empty() {
            return Ok(String::new());
        }

        // Check for v10/v11 prefix (AES-GCM encryption)
        if encrypted.len() > 3 && (&encrypted[..3] == b"v10" || &encrypted[..3] == b"v11") {
            return self.decrypt_chromium_v10(encrypted);
        }

        // Use DPAPI for decryption (older Chrome versions)
        use crate::security::DpapiStore;
        let dpapi = DpapiStore::new();
        let decrypted = dpapi
            .decrypt(encrypted)
            .map_err(|e| CookieError::Decryption(e.to_string()))?;

        String::from_utf8(decrypted)
            .map_err(|e| CookieError::Decryption(format!("UTF-8 error: {}", e)))
    }

    /// Decrypts Chrome v10/v11 encrypted cookies using AES-GCM
    #[cfg(windows)]
    fn decrypt_chromium_v10(&self, encrypted: &[u8]) -> Result<String, CookieError> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };

        // Get the encryption key from Chrome's Local State file
        let key = self.get_chromium_encryption_key()?;

        // Structure: "v10" (3 bytes) + nonce (12 bytes) + ciphertext + tag (16 bytes)
        if encrypted.len() < 3 + 12 + 16 {
            return Err(CookieError::Decryption("Encrypted data too short".into()));
        }

        let nonce_bytes = &encrypted[3..15]; // 12 bytes after "v10"
        let ciphertext = &encrypted[15..]; // Rest is ciphertext + auth tag

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| CookieError::Decryption(format!("Invalid key: {}", e)))?;

        let nonce = Nonce::from_slice(nonce_bytes);

        let decrypted = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CookieError::Decryption(format!("AES-GCM decryption failed: {}", e)))?;

        String::from_utf8(decrypted)
            .map_err(|e| CookieError::Decryption(format!("UTF-8 error: {}", e)))
    }

    /// Gets the encryption key from Chrome's Local State file
    #[cfg(windows)]
    fn get_chromium_encryption_key(&self) -> Result<Vec<u8>, CookieError> {
        use base64::Engine;
        use crate::security::DpapiStore;

        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| CookieError::EnvVar("LOCALAPPDATA".into()))?;

        // Try Chrome first, then Edge
        let local_state_paths = [
            PathBuf::from(&local_app_data)
                .join("Google")
                .join("Chrome")
                .join("User Data")
                .join("Local State"),
            PathBuf::from(&local_app_data)
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
                .join("Local State"),
        ];

        for path in &local_state_paths {
            if path.exists() {
                let content = std::fs::read_to_string(path)
                    .map_err(|e| CookieError::Io(e))?;

                // Parse JSON to get the encrypted key
                let json: serde_json::Value = serde_json::from_str(&content)
                    .map_err(|e| CookieError::Decryption(format!("JSON parse error: {}", e)))?;

                if let Some(encrypted_key_b64) = json
                    .get("os_crypt")
                    .and_then(|v| v.get("encrypted_key"))
                    .and_then(|v| v.as_str())
                {
                    // Decode base64
                    let encrypted_key = base64::engine::general_purpose::STANDARD
                        .decode(encrypted_key_b64)
                        .map_err(|e| CookieError::Decryption(format!("Base64 error: {}", e)))?;

                    // Remove "DPAPI" prefix (5 bytes)
                    if encrypted_key.len() < 5 || &encrypted_key[..5] != b"DPAPI" {
                        return Err(CookieError::Decryption("Invalid key format".into()));
                    }

                    // Decrypt with DPAPI
                    let dpapi = DpapiStore::new();
                    let key = dpapi
                        .decrypt(&encrypted_key[5..])
                        .map_err(|e| CookieError::Decryption(format!("DPAPI error: {}", e)))?;

                    return Ok(key);
                }
            }
        }

        Err(CookieError::Decryption("Could not find encryption key".into()))
    }

    #[cfg(not(windows))]
    fn decrypt_chromium_cookie(&self, _encrypted: &[u8]) -> Result<String, CookieError> {
        Err(CookieError::Decryption(
            "Cookie decryption only available on Windows".into(),
        ))
    }
}

impl Default for CookieExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_type_name() {
        assert_eq!(BrowserType::Chrome.name(), "Chrome");
        assert_eq!(BrowserType::Edge.name(), "Edge");
        assert_eq!(BrowserType::Firefox.name(), "Firefox");
    }

    #[test]
    fn test_browser_type_all() {
        let all = BrowserType::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&BrowserType::Chrome));
        assert!(all.contains(&BrowserType::Edge));
        assert!(all.contains(&BrowserType::Firefox));
    }

    #[test]
    fn test_cookie_to_header() {
        let cookie = Cookie {
            name: "session".to_string(),
            value: "abc123".to_string(),
            domain: "example.com".to_string(),
            path: "/".to_string(),
            expires: None,
            secure: true,
            http_only: true,
        };

        assert_eq!(cookie.to_header_value(), "session=abc123");
    }

    #[test]
    fn test_format_cookie_header() {
        let cookies = vec![
            Cookie {
                name: "a".to_string(),
                value: "1".to_string(),
                domain: "example.com".to_string(),
                path: "/".to_string(),
                expires: None,
                secure: false,
                http_only: false,
            },
            Cookie {
                name: "b".to_string(),
                value: "2".to_string(),
                domain: "example.com".to_string(),
                path: "/".to_string(),
                expires: None,
                secure: false,
                http_only: false,
            },
        ];

        let header = CookieExtractor::format_cookie_header(&cookies);
        assert_eq!(header, "a=1; b=2");
    }

    #[test]
    fn test_cookie_path_chrome() {
        // This test will only pass on Windows with Chrome installed
        if let Ok(path) = CookieExtractor::cookie_path(BrowserType::Chrome) {
            assert!(path.to_string_lossy().contains("Chrome"));
            assert!(path.to_string_lossy().contains("Cookies"));
        }
    }

    #[test]
    fn test_is_browser_available() {
        // Just check that it doesn't panic
        let _ = CookieExtractor::is_browser_available(BrowserType::Chrome);
        let _ = CookieExtractor::is_browser_available(BrowserType::Edge);
        let _ = CookieExtractor::is_browser_available(BrowserType::Firefox);
    }

    #[test]
    fn test_extractor_creation() {
        let extractor = CookieExtractor::new();
        let _ = extractor; // Just verify it compiles
    }
}
