//! Read-replica configuration and round-robin routing.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// How read queries are distributed across replicas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReadStrategy {
    #[default]
    RoundRobin,
    Random,
    LeastConnections,
}

/// Connection-URL configuration for a primary + optional read replicas.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub write: String,
    pub read: Vec<String>,
    pub read_strategy: ReadStrategy,
}

impl DatabaseConfig {
    pub fn new(write: impl Into<String>) -> Self {
        Self {
            write: write.into(),
            read: Vec::new(),
            read_strategy: ReadStrategy::default(),
        }
    }

    pub fn with_replica(mut self, url: impl Into<String>) -> Self {
        self.read.push(url.into());
        self
    }

    pub fn with_strategy(mut self, strategy: ReadStrategy) -> Self {
        self.read_strategy = strategy;
        self
    }

    pub fn has_replicas(&self) -> bool {
        !self.read.is_empty()
    }
}

/// Thread-safe, `Clone`-able counter for distributing reads across replicas.
#[derive(Debug, Default, Clone)]
pub struct RoundRobinCounter(Arc<AtomicUsize>);

impl RoundRobinCounter {
    pub fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }

    pub fn next(&self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        self.0.fetch_add(1, Ordering::Relaxed) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_config_builder() {
        let cfg = DatabaseConfig::new("postgres://primary/db")
            .with_replica("postgres://r1/db")
            .with_replica("postgres://r2/db")
            .with_strategy(ReadStrategy::RoundRobin);
        assert_eq!(cfg.write, "postgres://primary/db");
        assert_eq!(cfg.read.len(), 2);
        assert!(cfg.has_replicas());
    }

    #[test]
    fn round_robin_wraps() {
        let counter = RoundRobinCounter::new();
        assert_eq!(counter.next(3), 0);
        assert_eq!(counter.next(3), 1);
        assert_eq!(counter.next(3), 2);
        assert_eq!(counter.next(3), 0);
    }

    #[test]
    fn round_robin_cloned_shares_state() {
        let a = RoundRobinCounter::new();
        let b = a.clone();
        a.next(4);
        assert_eq!(b.next(4), 1);
    }
}
