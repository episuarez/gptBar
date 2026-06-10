//! Authentication module - Secure storage and credential management
//!
//! Provides secure storage for tokens and credentials using:
//! - Windows Credential Manager (via keyring crate)
//! - DPAPI for additional encryption layer

mod secure_store;

pub use secure_store::SecureStore;
