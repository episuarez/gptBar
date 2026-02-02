//! Data sanitization for secure logging and display
//!
//! Provides utilities to sanitize sensitive information before logging
//! or displaying to users, preventing accidental credential leakage.

use thiserror::Error;

/// Errors that can occur during input validation
#[derive(Debug, Error, PartialEq)]
pub enum SanitizerError {
    /// Input contains potentially dangerous characters
    #[error("Invalid input: contains dangerous characters")]
    InvalidInput,

    /// Input is empty when it shouldn't be
    #[error("Input cannot be empty")]
    EmptyInput,

    /// Input exceeds maximum allowed length
    #[error("Input exceeds maximum length of {0}")]
    TooLong(usize),
}

/// Sanitizer for sensitive data
///
/// Provides static methods to sanitize various types of sensitive data
/// before logging or display.
pub struct Sanitizer;

impl Sanitizer {
    /// Sanitizes an email address for safe logging
    ///
    /// Shows only the first 2 characters of the local part followed by "..."
    /// and the full domain.
    ///
    /// # Examples
    ///
    /// ```
    /// use gptbar_lib::security::Sanitizer;
    ///
    /// assert_eq!(Sanitizer::sanitize_email("john.doe@example.com"), "jo...@example.com");
    /// assert_eq!(Sanitizer::sanitize_email("a@b.com"), "***@b.com");
    /// assert_eq!(Sanitizer::sanitize_email("invalid"), "***");
    /// ```
    pub fn sanitize_email(email: &str) -> String {
        if let Some(at_pos) = email.find('@') {
            let (local, domain) = email.split_at(at_pos);
            if local.len() > 2 {
                format!("{}...{}", &local[..2], domain)
            } else {
                format!("***{}", domain)
            }
        } else {
            "***".to_string()
        }
    }

    /// Sanitizes a token/API key for safe logging
    ///
    /// Shows only the last 4 characters preceded by "***".
    ///
    /// # Examples
    ///
    /// ```
    /// use gptbar_lib::security::Sanitizer;
    ///
    /// assert_eq!(Sanitizer::sanitize_token("sk-ant-api03-abcdefghijklmnop"), "***mnop");
    /// assert_eq!(Sanitizer::sanitize_token("short"), "***hort");
    /// assert_eq!(Sanitizer::sanitize_token("abc"), "****");
    /// ```
    pub fn sanitize_token(token: &str) -> String {
        if token.len() > 4 {
            format!("***{}", &token[token.len() - 4..])
        } else {
            "****".to_string()
        }
    }

    /// Sanitizes a URL by removing query parameters and fragments
    ///
    /// Useful for logging URLs that might contain tokens in query strings.
    ///
    /// # Examples
    ///
    /// ```
    /// use gptbar_lib::security::Sanitizer;
    ///
    /// assert_eq!(
    ///     Sanitizer::sanitize_url("https://api.example.com/auth?token=secret"),
    ///     "https://api.example.com/auth"
    /// );
    /// ```
    pub fn sanitize_url(url: &str) -> String {
        url.split('?')
            .next()
            .unwrap_or(url)
            .split('#')
            .next()
            .unwrap_or(url)
            .to_string()
    }

    /// Validates that input doesn't contain dangerous characters
    ///
    /// Checks for characters that could be used in injection attacks:
    /// - HTML/XML: < > " ' &
    /// - Null bytes: \0
    /// - Control characters (except common whitespace)
    ///
    /// # Examples
    ///
    /// ```
    /// use gptbar_lib::security::Sanitizer;
    ///
    /// assert!(Sanitizer::validate_input("normal text").is_ok());
    /// assert!(Sanitizer::validate_input("<script>").is_err());
    /// assert!(Sanitizer::validate_input("hello\0world").is_err());
    /// ```
    pub fn validate_input(input: &str) -> Result<(), SanitizerError> {
        if input.is_empty() {
            return Err(SanitizerError::EmptyInput);
        }

        let dangerous_chars = ['<', '>', '"', '\'', '&', '\0'];
        if input.chars().any(|c| dangerous_chars.contains(&c)) {
            return Err(SanitizerError::InvalidInput);
        }

        // Check for control characters (except tab, newline, carriage return)
        if input
            .chars()
            .any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r')
        {
            return Err(SanitizerError::InvalidInput);
        }

        Ok(())
    }

    /// Validates input with a maximum length constraint
    pub fn validate_input_with_max_length(
        input: &str,
        max_length: usize,
    ) -> Result<(), SanitizerError> {
        if input.len() > max_length {
            return Err(SanitizerError::TooLong(max_length));
        }
        Self::validate_input(input)
    }

    /// Masks a string, showing only first and last n characters
    ///
    /// # Examples
    ///
    /// ```
    /// use gptbar_lib::security::Sanitizer;
    ///
    /// assert_eq!(Sanitizer::mask_string("abcdefghij", 2), "ab...ij");
    /// assert_eq!(Sanitizer::mask_string("short", 2), "sh...rt");
    /// assert_eq!(Sanitizer::mask_string("tiny", 3), "****");
    /// ```
    pub fn mask_string(s: &str, visible_chars: usize) -> String {
        if s.len() <= visible_chars * 2 {
            "****".to_string()
        } else {
            format!("{}...{}", &s[..visible_chars], &s[s.len() - visible_chars..])
        }
    }

    /// Escapes a string for safe display in logs (no HTML interpretation)
    pub fn escape_for_log(s: &str) -> String {
        s.replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
            .replace('&', "&amp;")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_email_normal() {
        assert_eq!(
            Sanitizer::sanitize_email("john.doe@example.com"),
            "jo...@example.com"
        );
    }

    #[test]
    fn test_sanitize_email_short_local() {
        assert_eq!(Sanitizer::sanitize_email("a@b.com"), "***@b.com");
        assert_eq!(Sanitizer::sanitize_email("ab@domain.org"), "***@domain.org");
    }

    #[test]
    fn test_sanitize_email_no_at_symbol() {
        assert_eq!(Sanitizer::sanitize_email("invalid"), "***");
        assert_eq!(Sanitizer::sanitize_email(""), "***");
    }

    #[test]
    fn test_sanitize_token_normal() {
        assert_eq!(
            Sanitizer::sanitize_token("sk-ant-api03-abcdefghijklmnop"),
            "***mnop"
        );
    }

    #[test]
    fn test_sanitize_token_short() {
        assert_eq!(Sanitizer::sanitize_token("abc"), "****");
        assert_eq!(Sanitizer::sanitize_token("abcd"), "****");
        assert_eq!(Sanitizer::sanitize_token("abcde"), "***bcde");
    }

    #[test]
    fn test_sanitize_url_with_query() {
        assert_eq!(
            Sanitizer::sanitize_url("https://api.example.com/auth?token=secret&user=admin"),
            "https://api.example.com/auth"
        );
    }

    #[test]
    fn test_sanitize_url_with_fragment() {
        assert_eq!(
            Sanitizer::sanitize_url("https://example.com/page#section"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_sanitize_url_clean() {
        assert_eq!(
            Sanitizer::sanitize_url("https://api.example.com/path"),
            "https://api.example.com/path"
        );
    }

    #[test]
    fn test_validate_input_normal() {
        assert!(Sanitizer::validate_input("normal text").is_ok());
        assert!(Sanitizer::validate_input("Hello, World!").is_ok());
        assert!(Sanitizer::validate_input("line1\nline2").is_ok());
        assert!(Sanitizer::validate_input("with\ttab").is_ok());
    }

    #[test]
    fn test_validate_input_empty() {
        assert_eq!(Sanitizer::validate_input(""), Err(SanitizerError::EmptyInput));
    }

    #[test]
    fn test_validate_input_html_chars() {
        assert_eq!(
            Sanitizer::validate_input("<script>"),
            Err(SanitizerError::InvalidInput)
        );
        assert_eq!(
            Sanitizer::validate_input("test>value"),
            Err(SanitizerError::InvalidInput)
        );
        assert_eq!(
            Sanitizer::validate_input("say \"hello\""),
            Err(SanitizerError::InvalidInput)
        );
        assert_eq!(
            Sanitizer::validate_input("it's"),
            Err(SanitizerError::InvalidInput)
        );
        assert_eq!(
            Sanitizer::validate_input("a&b"),
            Err(SanitizerError::InvalidInput)
        );
    }

    #[test]
    fn test_validate_input_null_byte() {
        assert_eq!(
            Sanitizer::validate_input("hello\0world"),
            Err(SanitizerError::InvalidInput)
        );
    }

    #[test]
    fn test_validate_input_control_chars() {
        // Bell character
        assert_eq!(
            Sanitizer::validate_input("hello\x07world"),
            Err(SanitizerError::InvalidInput)
        );
    }

    #[test]
    fn test_validate_input_with_max_length() {
        assert!(Sanitizer::validate_input_with_max_length("short", 10).is_ok());
        assert_eq!(
            Sanitizer::validate_input_with_max_length("this is too long", 5),
            Err(SanitizerError::TooLong(5))
        );
    }

    #[test]
    fn test_mask_string() {
        assert_eq!(Sanitizer::mask_string("abcdefghij", 2), "ab...ij");
        assert_eq!(Sanitizer::mask_string("secret_token_here", 3), "sec...ere");
        assert_eq!(Sanitizer::mask_string("tiny", 3), "****");
        assert_eq!(Sanitizer::mask_string("ab", 2), "****");
    }

    #[test]
    fn test_escape_for_log() {
        assert_eq!(Sanitizer::escape_for_log("<script>"), "&lt;script&gt;");
        assert_eq!(
            Sanitizer::escape_for_log("a & b"),
            "a &amp; b"
        );
        assert_eq!(
            Sanitizer::escape_for_log("\"quoted\""),
            "&quot;quoted&quot;"
        );
    }
}
