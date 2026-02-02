//! Security module - Secure storage, sanitization, and memory handling
//!
//! This module provides security primitives for:
//! - Sanitizing sensitive data for logs
//! - Secure string handling with zeroization
//! - DPAPI-based encryption on Windows
//! - Certificate pinning for HTTPS clients

mod sanitizer;
mod secure_string;

pub use sanitizer::Sanitizer;
pub use secure_string::SecureString;

#[cfg(windows)]
mod dpapi;
#[cfg(windows)]
pub use dpapi::DpapiStore;
