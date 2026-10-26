//! A map that holds at most a given weight of values. The entries held
//! longest are dropped first.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Keys to values, bounded by the weight of the values. The entries held
/// longest go first.
pub(crate) struct Bounded<K, V> {
    map: HashMap<K, (V, usize)>,
    order: VecDeque<K>,
    weight: usize,
    limit: usize,
}

impl<K: Hash + Eq + Clone, V: Clone> Bounded<K, V> {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            weight: 0,
            limit,
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        self.map.get(key).map(|(value, _)| value.clone())
    }

    pub(crate) fn insert(&mut self, key: K, value: V, weight: usize) {
        if self.limit == 0 || self.map.contains_key(&key) {
            return;
        }
        self.weight += weight;
        self.order.push_back(key.clone());
        self.map.insert(key, (value, weight));
        while self.weight > self.limit && self.order.len() > 1 {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some((_, dropped)) = self.map.remove(&oldest) {
                self.weight -= dropped;
            }
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    pub(crate) fn weight(&self) -> usize {
        self.weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entries_held_longest_go_first() {
        let mut bounded: Bounded<u8, u8> = Bounded::new(5);
        bounded.insert(1, 1, 2);
        bounded.insert(2, 2, 2);
        bounded.insert(3, 3, 2);
        assert_eq!(bounded.get(&1), None);
        assert_eq!(bounded.get(&2), Some(2));
        assert_eq!(bounded.get(&3), Some(3));
        assert_eq!(bounded.weight(), 4);
    }

    #[test]
    fn one_entry_heavier_than_the_limit_is_still_held() {
        let mut bounded: Bounded<u8, u8> = Bounded::new(1);
        bounded.insert(1, 1, 10);
        assert_eq!(bounded.get(&1), Some(1));
        bounded.insert(2, 2, 10);
        assert_eq!(bounded.get(&1), None);
        assert_eq!(bounded.get(&2), Some(2));
    }

    #[test]
    fn a_limit_of_zero_holds_nothing() {
        let mut bounded: Bounded<u8, u8> = Bounded::new(0);
        bounded.insert(1, 1, 1);
        assert_eq!(bounded.get(&1), None);
    }
}
