//! Read-replica configuration and round-robin routing.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

// ── ReadStrategy ──────────────────────────────────────────────────────────────

/// How read queries are distributed across replicas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReadStrategy {
    /// Cycle through replicas in order (default).
    #[default]
    RoundRobin,
    /// Pick a replica at random.
    Random,
    /// Route to the replica with the fewest active connections
    /// (requires runtime support in the execution layer).
    LeastConnections,
}

// ── DatabaseConfig ────────────────────────────────────────────────────────────

/// Connection-URL configuration for a primary + optional read replicas.
///
/// ```rust
/// use rok_orm_core::replica::{DatabaseConfig, ReadStrategy};
///
/// let cfg = DatabaseConfig::new("postgres://primary/db")
///     .with_replica("postgres://replica1/db")
///     .with_replica("postgres://replica2/db")
///     .with_strategy(ReadStrategy::RoundRobin);
///
/// assert!(cfg.has_replicas());
/// ```
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Write (primary) database URL.
    pub write: String,
    /// Read replica URLs.  Empty means all queries go to `write`.
    pub read: Vec<String>,
    /// Strategy for distributing reads across `read` replicas.
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

// ── RoundRobinCounter ─────────────────────────────────────────────────────────

/// Thread-safe, `Clone`-able counter for distributing reads across replicas.
///
/// Wrap in `Arc` on your `AppState` to share across handlers.
#[derive(Debug, Default, Clone)]
pub struct RoundRobinCounter(Arc<AtomicUsize>);

impl RoundRobinCounter {
    pub fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }

    /// Returns the next replica index given `len` replicas.  Returns `0` for empty pools.
    pub fn next(&self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        self.0.fetch_add(1, Ordering::Relaxed) % len
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

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
        assert_eq!(cfg.read_strategy, ReadStrategy::RoundRobin);
    }

    #[test]
    fn no_replicas_has_replicas_false() {
        let cfg = DatabaseConfig::new("postgres://primary/db");
        assert!(!cfg.has_replicas());
    }

    #[test]
    fn round_robin_wraps() {
        let counter = RoundRobinCounter::new();
        assert_eq!(counter.next(3), 0);
        assert_eq!(counter.next(3), 1);
        assert_eq!(counter.next(3), 2);
        assert_eq!(counter.next(3), 0); // wraps
    }

    #[test]
    fn round_robin_zero_len() {
        let counter = RoundRobinCounter::new();
        assert_eq!(counter.next(0), 0);
    }

    #[test]
    fn round_robin_cloned_shares_state() {
        let a = RoundRobinCounter::new();
        let b = a.clone();
        a.next(4);
        assert_eq!(b.next(4), 1); // shared AtomicUsize
    }
}
