use std::collections::HashMap;
use zeroize::Zeroizing;

/// Secure secret store using zeroizing strings
pub struct SecretStore {
    secrets: HashMap<String, Zeroizing<String>>,
}

impl SecretStore {
    /// Create a new secret store
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }

    /// Store a secret with automatic zeroization on drop
    pub fn store(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = Zeroizing::new(value.into());
        self.secrets.insert(key, value);
    }

    /// Retrieve a secret (returns a clone of the zeroized string)
    pub fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).map(|s| s.as_str().to_string())
    }

    /// Check if a secret exists
    pub fn has(&self, key: &str) -> bool {
        self.secrets.contains_key(key)
    }

    /// Remove a secret (will be zeroed)
    pub fn remove(&mut self, key: &str) -> Option<Zeroizing<String>> {
        self.secrets.remove(key)
    }

    /// Clear all secrets (will all be zeroed)
    pub fn clear_all(&mut self) {
        self.secrets.clear();
    }

    /// Get count of stored secrets
    pub fn count(&self) -> usize {
        self.secrets.len()
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SecretStore {
    fn drop(&mut self) {
        self.clear_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve() {
        let mut store = SecretStore::new();
        store.store("api_key", "secret123");

        assert_eq!(store.get("api_key"), Some("secret123".to_string()));
    }

    #[test]
    fn test_has_secret() {
        let mut store = SecretStore::new();
        store.store("password", "hunter2");

        assert!(store.has("password"));
        assert!(!store.has("missing"));
    }

    #[test]
    fn test_remove_secret() {
        let mut store = SecretStore::new();
        store.store("temp", "value");

        let removed = store.remove("temp");
        assert!(removed.is_some());
        assert!(!store.has("temp"));
    }

    #[test]
    fn test_clear_all() {
        let mut store = SecretStore::new();
        store.store("key1", "value1");
        store.store("key2", "value2");
        store.store("key3", "value3");

        assert_eq!(store.count(), 3);
        store.clear_all();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_multiple_secrets() {
        let mut store = SecretStore::new();
        store.store("db_pass", "postgres123");
        store.store("api_token", "token_xyz");
        store.store("aws_key", "AKIA...");

        assert_eq!(store.count(), 3);
        assert!(store.has("db_pass"));
        assert!(store.has("api_token"));
        assert!(store.has("aws_key"));
    }
}
