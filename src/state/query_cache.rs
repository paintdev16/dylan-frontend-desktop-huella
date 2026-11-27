use std::{collections::HashMap, hash::Hash};

use super::store::DataLoad;

pub(super) struct QueryCache<K, V> {
    entries: HashMap<K, CachedQuery<V>>,
    sequence: u64,
}

impl<K, V> Default for QueryCache<K, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            sequence: 0,
        }
    }
}

pub struct CachedQuery<V> {
    pub value: Option<V>,
    pub error: Option<String>,
    valid: bool,
    pending: Option<u64>,
}

impl<V> Default for CachedQuery<V> {
    fn default() -> Self {
        Self {
            value: None,
            error: None,
            valid: false,
            pending: None,
        }
    }
}

impl<V> CachedQuery<V> {
    pub fn status(&self) -> DataLoad {
        if self.value.is_some() {
            DataLoad::Ready
        } else if self.pending.is_some() {
            DataLoad::Loading
        } else if self.error.is_some() {
            DataLoad::Failed
        } else {
            DataLoad::Idle
        }
    }

    pub(super) fn is_valid(&self) -> bool {
        self.valid
    }
}

impl<K: Eq + Hash, V> QueryCache<K, V> {
    pub fn get(&self, key: K) -> Option<&CachedQuery<V>> {
        self.entries.get(&key)
    }

    pub fn begin(&mut self, key: K, force: bool) -> Option<u64> {
        let entry = self.entries.entry(key).or_default();
        if entry.pending.is_some() || (entry.is_valid() && !force) {
            return None;
        }
        self.sequence += 1;
        entry.pending = Some(self.sequence);
        entry.error = None;
        Some(self.sequence)
    }

    pub fn is_pending(&self, key: K, request: u64) -> bool {
        self.get(key)
            .is_some_and(|entry| entry.pending == Some(request))
    }

    pub fn complete(&mut self, key: K, result: Result<V, String>) {
        let entry = self
            .entries
            .get_mut(&key)
            .expect("pending query has a cache entry");
        entry.pending = None;
        match result {
            Ok(value) => {
                entry.value = Some(value);
                entry.valid = true;
            }
            Err(error) => {
                entry.error = Some(error);
            }
        }
    }

    pub fn invalidate(&mut self, matches: impl Fn(&K) -> bool) {
        for (_, entry) in self.entries.iter_mut().filter(|(key, _)| matches(key)) {
            entry.valid = false;
            entry.pending = None;
            entry.error = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_loaded_results_for_the_session_and_coalesces_pending_requests() {
        let mut cache = QueryCache::default();
        let first = cache.begin(1, false).unwrap();
        assert!(cache.is_pending(1, first));
        assert_eq!(cache.begin(1, true), None);
        cache.complete(1, Ok(Vec::<u8>::new()));
        assert_eq!(cache.get(1).unwrap().status(), DataLoad::Ready);
        for key in 2..=12 {
            assert!(cache.begin(key, false).is_some());
            cache.complete(key, Ok(vec![key]));
        }
        assert_eq!(cache.begin(1, false), None);
        assert!(cache.begin(1, true).is_some());
        assert_eq!(cache.get(1).unwrap().status(), DataLoad::Ready);
        cache.complete(1, Err("Sin conexión".into()));
        assert_eq!(cache.get(1).unwrap().status(), DataLoad::Ready);
        assert!(cache.get(1).unwrap().error.is_some());
        assert_eq!(cache.begin(1, false), None);
    }

    #[test]
    fn invalidation_rejects_older_responses_and_failed_reads_can_retry() {
        let mut cache = QueryCache::<u8, ()>::default();
        cache.begin(1, false).unwrap();
        cache.complete(1, Ok(()));
        assert_eq!(cache.begin(1, false), None);
        let earlier = cache.begin(1, true).unwrap();
        let other = cache.begin(2, false).unwrap();
        cache.invalidate(|key| *key == 1);
        let later = cache.begin(1, false).unwrap();
        assert!(!cache.is_pending(1, earlier));
        assert!(cache.is_pending(1, later));
        assert!(cache.is_pending(2, other));
        cache.complete(1, Err("Sin conexión".into()));
        assert_eq!(cache.get(1).unwrap().status(), DataLoad::Ready);
        assert!(cache.begin(1, false).is_some());
        cache.complete(2, Err("Sin conexión".into()));
        assert_eq!(cache.get(2).unwrap().status(), DataLoad::Failed);
        assert!(cache.begin(2, false).is_some());
    }
}
