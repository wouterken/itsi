use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// HTTP-01 challenge handler for ACME certificate validation
///
/// This module provides functionality to handle HTTP-01 challenges by serving
/// challenge responses at the `.well-known/acme-challenge/` endpoint.
#[derive(Debug, Clone)]
pub struct Http01Handler {
    /// Map of challenge tokens to their key authorizations
    challenges: Arc<RwLock<HashMap<String, String>>>,
}

impl Http01Handler {
    /// Create a new HTTP-01 challenge handler
    pub fn new() -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a challenge token and its corresponding key authorization
    pub fn add_challenge(&self, token: String, key_auth: String) {
        let mut challenges = self.challenges.write();
        challenges.insert(token, key_auth);
    }

    /// Remove a challenge token
    pub fn remove_challenge(&self, token: &str) {
        let mut challenges = self.challenges.write();
        challenges.remove(token);
    }

    /// Get the key authorization for a given token
    pub fn get_key_auth(&self, token: &str) -> Option<String> {
        let challenges = self.challenges.read();
        challenges.get(token).cloned()
    }

    /// Clear all challenges
    pub fn clear(&self) {
        let mut challenges = self.challenges.write();
        challenges.clear();
    }

    /// Handle an HTTP request for ACME challenge verification
    ///
    /// This should be called for requests to `/.well-known/acme-challenge/{token}`
    /// Returns Some(key_auth) if the token exists, None otherwise
    pub fn handle_challenge_request(&self, path: &str) -> Option<String> {
        // Extract token from path like "/.well-known/acme-challenge/TOKEN"
        const ACME_CHALLENGE_PREFIX: &str = "/.well-known/acme-challenge/";

        if !path.starts_with(ACME_CHALLENGE_PREFIX) {
            return None;
        }

        let token = &path[ACME_CHALLENGE_PREFIX.len()..];

        // Validate token format (should be base64url safe characters)
        if token.is_empty()
            || !token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }

        self.get_key_auth(token)
    }
}

impl Default for Http01Handler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_challenge() {
        let handler = Http01Handler::new();
        let token = "test_token".to_string();
        let key_auth = "test_key_auth".to_string();

        handler.add_challenge(token.clone(), key_auth.clone());
        assert_eq!(handler.get_key_auth(&token), Some(key_auth));
    }

    #[test]
    fn test_remove_challenge() {
        let handler = Http01Handler::new();
        let token = "test_token".to_string();
        let key_auth = "test_key_auth".to_string();

        handler.add_challenge(token.clone(), key_auth);
        assert!(handler.get_key_auth(&token).is_some());

        handler.remove_challenge(&token);
        assert!(handler.get_key_auth(&token).is_none());
    }

    #[test]
    fn test_handle_challenge_request() {
        let handler = Http01Handler::new();
        let token = "abcd1234-_EFGH5678";
        let key_auth = "test_key_auth".to_string();

        handler.add_challenge(token.to_string(), key_auth.clone());

        // Valid challenge request
        let path = format!("/.well-known/acme-challenge/{}", token);
        assert_eq!(handler.handle_challenge_request(&path), Some(key_auth));

        // Invalid paths
        assert_eq!(handler.handle_challenge_request("/invalid/path"), None);
        assert_eq!(
            handler.handle_challenge_request("/.well-known/acme-challenge/"),
            None
        );
        assert_eq!(
            handler.handle_challenge_request("/.well-known/acme-challenge/invalid@token"),
            None
        );
    }

    #[test]
    fn test_clear_challenges() {
        let handler = Http01Handler::new();
        handler.add_challenge("token1".to_string(), "auth1".to_string());
        handler.add_challenge("token2".to_string(), "auth2".to_string());

        assert!(handler.get_key_auth("token1").is_some());
        assert!(handler.get_key_auth("token2").is_some());

        handler.clear();

        assert!(handler.get_key_auth("token1").is_none());
        assert!(handler.get_key_auth("token2").is_none());
    }
}
