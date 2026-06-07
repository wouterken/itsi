use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

const ACME_CHALLENGE_PREFIX: &str = "/.well-known/acme-challenge/";

#[derive(Debug, Clone, Default)]
pub struct Http01Handler {
    challenges: Arc<RwLock<HashMap<String, String>>>,
}

impl Http01Handler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_challenge(&self, token: String, key_authorization: String) {
        self.challenges.write().insert(token, key_authorization);
    }

    pub fn remove_challenge(&self, token: &str) {
        self.challenges.write().remove(token);
    }

    pub fn handle_challenge_request(&self, path: &str) -> Option<String> {
        let token = path.strip_prefix(ACME_CHALLENGE_PREFIX)?;
        if token.is_empty()
            || !token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }

        self.challenges.read().get(token).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::Http01Handler;

    #[test]
    fn serves_registered_key_authorization() {
        let handler = Http01Handler::new();
        handler.add_challenge("token_123".to_string(), "token_123.thumbprint".to_string());

        assert_eq!(
            handler.handle_challenge_request("/.well-known/acme-challenge/token_123"),
            Some("token_123.thumbprint".to_string())
        );
    }

    #[test]
    fn rejects_invalid_paths_and_tokens() {
        let handler = Http01Handler::new();
        handler.add_challenge("token_123".to_string(), "token_123.thumbprint".to_string());

        assert_eq!(handler.handle_challenge_request("/not-acme"), None);
        assert_eq!(
            handler.handle_challenge_request("/.well-known/acme-challenge/"),
            None
        );
        assert_eq!(
            handler.handle_challenge_request("/.well-known/acme-challenge/invalid token"),
            None
        );
    }

    #[test]
    fn removes_tokens() {
        let handler = Http01Handler::new();
        handler.add_challenge("token_123".to_string(), "token_123.thumbprint".to_string());
        handler.remove_challenge("token_123");

        assert_eq!(
            handler.handle_challenge_request("/.well-known/acme-challenge/token_123"),
            None
        );
    }
}
