use crate::hasher::*;
use crate::swap::SwapContract;
use moka::sync::Cache;

#[derive(Hash, PartialEq, Eq)]
pub struct RecentFiredMevKey {
    pub input_amount: u64,
    pub output_amount: u64,
    pub contracts: Vec<SwapContract>,
}

#[derive(Clone)]
pub struct RecentFiredMevCache {
    cache: Cache<RecentFiredMevKey, u64, RandomState>,
}

impl RecentFiredMevCache {
    pub fn new(time_to_live_secs: Option<u64>) -> Self {
        let time_to_live_secs = time_to_live_secs.unwrap_or(1200);
        let cache = Cache::builder()
            .time_to_live(std::time::Duration::from_secs(time_to_live_secs))
            .build_with_hasher(new_random_state());
        Self { cache }
    }

    pub fn insert(&mut self, key: RecentFiredMevKey, version: u64) {
        self.cache.insert(key, version)
    }
    pub fn get(&self, key: &RecentFiredMevKey) -> Option<u64> {
        self.cache.get(key)
    }
}
