use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use ttl_cache::TtlCache;

pub trait CacheManagement: Send + Sync {
    type Value;

    fn get(&self, key: &str) -> Option<Self::Value>;
    fn insert(&self, key: &str, value: Self::Value, ttl: Duration) -> Option<Self::Value>;
    fn invalidate(&self, key: &str) -> Option<Self::Value>;
}

pub struct CacheManager<T> {
    pub cache: Arc<RwLock<TtlCache<String, T>>>,
}

impl<T> CacheManager<T> {
    pub fn new(capacity: usize) -> Self {
        let cache = Arc::new(RwLock::new(TtlCache::new(capacity)));

        Self { cache }
    }
}

impl<T: Send + Sync + Clone> CacheManagement for CacheManager<T> {
    type Value = T;

    fn get(&self, key: &str) -> Option<Self::Value> {
        self.cache
            .read()
            .expect("cache lock should not be poisoned")
            .get(key)
            .cloned()
    }

    fn insert(&self, key: &str, value: T, ttl: Duration) -> Option<Self::Value> {
        self.cache
            .write()
            .expect("cache lock should not be poisoned")
            .insert(key.to_string(), value, ttl)
    }

    fn invalidate(&self, key: &str) -> Option<Self::Value> {
        self.cache
            .write()
            .expect("cache lock should not be poisoned")
            .remove(key)
    }
}
