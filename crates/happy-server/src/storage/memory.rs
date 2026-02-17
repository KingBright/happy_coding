//! In-memory cache using DashMap (replaces Redis for simplicity)

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Simple in-memory cache with TTL support
pub struct MemoryCache {
    data: Arc<DashMap<String, CacheEntry>>,
}

struct CacheEntry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

impl MemoryCache {
    pub fn new() -> Self {
        let cache = Self {
            data: Arc::new(DashMap::new()),
        };

        // Start cleanup task
        cache.start_cleanup_task();

        cache
    }

    /// Get a value from cache
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.data.get(key).and_then(|entry| {
            if let Some(expires) = entry.expires_at {
                if Instant::now() > expires {
                    drop(entry);
                    self.data.remove(key);
                    return None;
                }
            }
            Some(entry.value.clone())
        })
    }

    /// Set a value in cache (no TTL)
    pub fn set(&self, key: String, value: Vec<u8>) {
        self.data.insert(
            key,
            CacheEntry {
                value,
                expires_at: None,
            },
        );
    }

    /// Set a value with TTL
    pub fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) {
        self.data.insert(
            key,
            CacheEntry {
                value,
                expires_at: Some(Instant::now() + ttl),
            },
        );
    }

    /// Delete a key from cache
    pub fn delete(&self, key: &str) {
        self.data.remove(key);
    }

    /// Check if key exists
    pub fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Get and delete (atomic operation for session tokens)
    pub fn take(&self, key: &str) -> Option<Vec<u8>> {
        self.data.remove(key).map(|(_, entry)| entry.value)
    }

    fn start_cleanup_task(&self) {
        let data = self.data.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;

                let now = Instant::now();
                let keys_to_remove: Vec<String> = data
                    .iter()
                    .filter(|entry| {
                        entry
                            .expires_at
                            .map(|expires| now > expires)
                            .unwrap_or(false)
                    })
                    .map(|entry| entry.key().clone())
                    .collect();

                for key in keys_to_remove {
                    data.remove(&key);
                }
            }
        });
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_operations() {
        let cache = MemoryCache::new();

        // Test set and get
        cache.set("key1".to_string(), vec![1, 2, 3]);
        assert_eq!(cache.get("key1"), Some(vec![1, 2, 3]));

        // Test non-existent key
        assert_eq!(cache.get("nonexistent"), None);

        // Test delete
        cache.delete("key1");
        assert_eq!(cache.get("key1"), None);
    }

    #[tokio::test]
    async fn test_ttl() {
        let cache = MemoryCache::new();

        // Set with very short TTL
        cache.set_with_ttl("key1".to_string(), vec![1, 2, 3], Duration::from_millis(10));
        assert_eq!(cache.get("key1"), Some(vec![1, 2, 3]));

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(cache.get("key1"), None);
    }

    #[tokio::test]
    async fn test_take() {
        let cache = MemoryCache::new();

        cache.set("key1".to_string(), vec![1, 2, 3]);
        let value = cache.take("key1");
        assert_eq!(value, Some(vec![1, 2, 3]));
        assert_eq!(cache.get("key1"), None);
    }
}
