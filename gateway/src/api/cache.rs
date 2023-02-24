use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use ttl_cache::TtlCache;

pub trait CacheManagement: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn insert(&self, key: &str, value: String, ttl: Duration) -> Option<String>;
    fn invalidate(&self, key: &str) -> Option<String>;
}

pub struct CacheManager {
    pub cache: Arc<RwLock<TtlCache<String, String>>>,
}

impl CacheManager {
    pub fn new() -> Self {
        let cache = Arc::new(RwLock::new(TtlCache::new(1000)));

        Self { cache }
    }
}

impl CacheManagement for CacheManager {
    fn get(&self, key: &str) -> Option<String> {
        self.cache
            .read()
            .expect("cache lock should not be poisoned")
            .get(key)
            .cloned()
    }

    fn insert(&self, key: &str, value: String, ttl: Duration) -> Option<String> {
        self.cache
            .write()
            .expect("cache lock should not be poisoned")
            .insert(key.to_string(), value, ttl)
    }

    fn invalidate(&self, key: &str) -> Option<String> {
        self.cache
            .write()
            .expect("cache lock should not be poisoned")
            .remove(key)
    }
}
