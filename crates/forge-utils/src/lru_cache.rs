use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::hash::Hash;

#[derive(Debug)]
pub struct LruCache<K, V> {
    capacity: usize,
    order: VecDeque<K>,
    map: HashMap<K, V>,
}

impl<K, V> LruCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
            order: VecDeque::with_capacity(capacity),
        }
    }

    pub async fn get_or_fetch<T, F, E>(&mut self, key: K, fetcher: T) -> Result<V, E>
    where
        T: FnOnce(&K) -> F,
        F: Future<Output = Result<V, E>>,
    {
        if let Some(val) = self.map.get(&key).cloned() {
            self.touch(&key);
            return Ok(val);
        }

        let val: V = fetcher(&key).await?;
        self.insert(key, val.clone());
        Ok(val)
    }

    fn touch(&mut self, key: &K) {
        if self.order.back().is_some_and(|last: &K| last == key) {
            return;
        }

        if let Some(pos) = self.order.iter().position(|x| x == key) {
            self.order.remove(pos);
        }

        self.order.push_back(key.clone());
    }

    fn insert(&mut self, key: K, val: V) {
        if self.capacity == 0 {
            return;
        }

        if self.map.contains_key(&key) {
            self.map.insert(key.clone(), val);
            self.touch(&key);
            return;
        }

        if self.map.len() >= self.capacity
            && let Some(old_key) = self.order.pop_front()
        {
            self.map.remove(&old_key);
        }

        self.map.insert(key.clone(), val);
        self.order.push_back(key);
    }
}
