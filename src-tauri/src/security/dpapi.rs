//! Windows DPAPI (Data Protection API) integration
//!
//! Provides secure encryption/decryption using Windows DPAPI, which ties
//! the encryption to the current user account.

use thiserror::Error;

/// Errors that can occur during DPAPI operations
#[derive(Debug, Error)]
pub enum DpapiError {
    /// DPAPI encryption failed
    #[error("DPAPI encryption failed: {0}")]
    EncryptionFailed(String),

    /// DPAPI decryption failed
    #[error("DPAPI decryption failed: {0}")]
    DecryptionFailed(String),

    /// Windows API error
    #[error("Windows API error: {0}")]
    WindowsError(String),

    /// Memory allocation error
    #[error("Memory allocation error")]
    MemoryError,
}

/// DPAPI-based secure storage
///
/// Uses Windows Data Protection API to encrypt/decrypt data tied to
/// the current user account. Only the same Windows user can decrypt
/// the data.
pub struct DpapiStore;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    // Import LocalFree from kernel32
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    impl DpapiStore {
        /// Encrypts data using DPAPI
        pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, DpapiError> {
            if data.is_empty() {
                return Ok(Vec::new());
            }

            unsafe {
                let mut blob_in = CRYPT_INTEGER_BLOB {
                    cbData: data.len() as u32,
                    pbData: data.as_ptr() as *mut u8,
                };
                let mut blob_out = CRYPT_INTEGER_BLOB::default();

                let result = CryptProtectData(
                    &mut blob_in,
                    None,
                    None,
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut blob_out,
                );

                if result.is_err() {
                    return Err(DpapiError::EncryptionFailed(
                        "CryptProtectData failed".into(),
                    ));
                }

                if blob_out.pbData.is_null() || blob_out.cbData == 0 {
                    return Err(DpapiError::MemoryError);
                }

                let encrypted =
                    std::slice::from_raw_parts(blob_out.pbData, blob_out.cbData as usize).to_vec();

                // Free the memory allocated by Windows using direct FFI
                LocalFree(blob_out.pbData as *mut std::ffi::c_void);

                Ok(encrypted)
            }
        }

        /// Decrypts data using DPAPI
        pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, DpapiError> {
            if encrypted.is_empty() {
                return Ok(Vec::new());
            }

            unsafe {
                let mut blob_in = CRYPT_INTEGER_BLOB {
                    cbData: encrypted.len() as u32,
                    pbData: encrypted.as_ptr() as *mut u8,
                };
                let mut blob_out = CRYPT_INTEGER_BLOB::default();

                let result = CryptUnprotectData(
                    &mut blob_in,
                    None,
                    None,
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut blob_out,
                );

                if result.is_err() {
                    return Err(DpapiError::DecryptionFailed(
                        "CryptUnprotectData failed".into(),
                    ));
                }

                if blob_out.pbData.is_null() || blob_out.cbData == 0 {
                    return Err(DpapiError::MemoryError);
                }

                let decrypted =
                    std::slice::from_raw_parts(blob_out.pbData, blob_out.cbData as usize).to_vec();

                // Free the memory allocated by Windows using direct FFI
                LocalFree(blob_out.pbData as *mut std::ffi::c_void);

                Ok(decrypted)
            }
        }

        /// Encrypts a string using DPAPI and returns base64-encoded result
        pub fn encrypt_string(&self, plaintext: &str) -> Result<String, DpapiError> {
            use base64::Engine;
            let encrypted = self.encrypt(plaintext.as_bytes())?;
            Ok(base64::engine::general_purpose::STANDARD.encode(&encrypted))
        }

        /// Decrypts a base64-encoded DPAPI-encrypted string
        pub fn decrypt_string(&self, encoded: &str) -> Result<String, DpapiError> {
            use base64::Engine;
            let encrypted = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| DpapiError::DecryptionFailed(format!("Base64 decode error: {}", e)))?;

            let decrypted = self.decrypt(&encrypted)?;

            String::from_utf8(decrypted)
                .map_err(|e| DpapiError::DecryptionFailed(format!("UTF-8 decode error: {}", e)))
        }
    }
}

impl DpapiStore {
    /// Creates a new DpapiStore
    pub fn new() -> Self {
        Self
    }

    // Non-Windows stubs for cross-platform compilation
    #[cfg(not(windows))]
    pub fn encrypt(&self, _data: &[u8]) -> Result<Vec<u8>, DpapiError> {
        Err(DpapiError::WindowsError(
            "DPAPI is only available on Windows".into(),
        ))
    }

    #[cfg(not(windows))]
    pub fn decrypt(&self, _encrypted: &[u8]) -> Result<Vec<u8>, DpapiError> {
        Err(DpapiError::WindowsError(
            "DPAPI is only available on Windows".into(),
        ))
    }

    #[cfg(not(windows))]
    pub fn encrypt_string(&self, _plaintext: &str) -> Result<String, DpapiError> {
        Err(DpapiError::WindowsError(
            "DPAPI is only available on Windows".into(),
        ))
    }

    #[cfg(not(windows))]
    pub fn decrypt_string(&self, _encoded: &str) -> Result<String, DpapiError> {
        Err(DpapiError::WindowsError(
            "DPAPI is only available on Windows".into(),
        ))
    }
}

impl Default for DpapiStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_store_creation() {
        let store = DpapiStore::new();
        let _ = store; // Just verify it compiles
    }

    #[test]
    fn test_dpapi_empty_data() {
        let store = DpapiStore::new();
        // On non-Windows, these will return errors, which is expected
        let result = store.encrypt(&[]);
        #[cfg(windows)]
        assert!(result.is_ok());
        #[cfg(not(windows))]
        assert!(result.is_err());
    }

    #[cfg(windows)]
    #[test]
    fn test_dpapi_roundtrip() {
        let store = DpapiStore::new();
        let original = b"secret data to encrypt";

        let encrypted = store.encrypt(original).expect("encryption should work");
        assert_ne!(encrypted, original);

        let decrypted = store.decrypt(&encrypted).expect("decryption should work");
        assert_eq!(decrypted, original);
    }

    #[cfg(windows)]
    #[test]
    fn test_dpapi_string_roundtrip() {
        let store = DpapiStore::new();
        let original = "my-secret-token";

        let encrypted = store
            .encrypt_string(original)
            .expect("encryption should work");
        assert_ne!(encrypted, original);

        let decrypted = store
            .decrypt_string(&encrypted)
            .expect("decryption should work");
        assert_eq!(decrypted, original);
    }

    #[cfg(windows)]
    #[test]
    fn test_dpapi_unicode() {
        let store = DpapiStore::new();
        let original = "日本語テスト 🔐";

        let encrypted = store
            .encrypt_string(original)
            .expect("encryption should work");
        let decrypted = store
            .decrypt_string(&encrypted)
            .expect("decryption should work");

        assert_eq!(decrypted, original);
    }
}
