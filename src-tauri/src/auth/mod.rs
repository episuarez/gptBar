//! Authentication module - Secure storage and credential management
//!
//! Provides secure storage for tokens and credentials using:
//! - Windows Credential Manager (via keyring crate)
//! - DPAPI for additional encryption layer
//! - Cookie extraction from browsers

mod secure_store;
mod cookie_extractor;

pub use secure_store::SecureStore;
pub use cookie_extractor::{CookieExtractor, BrowserType};
