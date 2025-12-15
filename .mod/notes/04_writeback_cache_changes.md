# WritebackCache & ExecutionCacheWrite Modifications

## Overview

The indexer modifications require adding methods to the `ExecutionCacheWrite` trait and implementing them in `WritebackCache`.

---

## File 1: execution_cache.rs (Trait Definition)

**File:** `crates/sui-core/src/execution_cache.rs`

Add these methods to the `ExecutionCacheWrite` trait:

```rust
pub trait ExecutionCacheWrite: Send + Sync {
    // ... existing methods ...

    /// Reload objects into cache. Called when receiving updates from socket.
    fn reload_objects(&self, objects: Vec<(ObjectID, Object)>);

    /// Sync with underlying RocksDB and optionally clear cache.
    fn update_underlying(&self, clear_cache: bool);
}
```

---

## File 2: writeback_cache.rs (Implementation)

**File:** `crates/sui-core/src/execution_cache/writeback_cache.rs`

### Add required trait from Map

```rust
use typed_store::traits::{Map};
```

### Add reload_cached method (helper)

```rust
impl WritebackCache {
    /// Insert objects directly into the cache.
    pub fn reload_cached(&self, objects: Vec<(ObjectID, Object)>) {
        for (object_id, object) in objects {
            let _ = self.object_by_id_cache.insert(
                &object_id,
                LatestObjectCacheEntry::Object(object.version(), object.into()),
                Ticket::Write,
            );
        }
    }

    /// Clear all cached data.
    pub fn clear(&self) {
        self.cached.clear();
    }
}
```

### Implement trait methods

```rust
impl ExecutionCacheWrite for WritebackCache {
    // ... existing methods ...

    fn reload_objects(&self, objects: Vec<(ObjectID, Object)>) {
        self.reload_cached(objects);
    }

    fn update_underlying(&self, clear_cache: bool) {
        // Sync RocksDB secondary with primary
        self.store
            .perpetual_tables
            .objects
            .try_catch_up_with_primary()
            .unwrap();

        if clear_cache {
            self.clear();
        }
    }
}
```

And implement clear method

```rust
impl CachedCommittedData {
    // ... existing methods ...

    fn clear(&self) {
        self.object_cache.invalidate_all();
        self.marker_cache.invalidate_all();
        self.transactions.invalidate_all();
        self.transaction_effects.invalidate_all();
        self.transaction_events.invalidate_all();
        self.executed_effects_digests.invalidate_all();
        self._transaction_objects.invalidate_all();
    }
}
```

---

## Key Implementation Details

1. **`object_by_id_cache`** - Uses `LatestObjectCacheEntry::Object(version, object)` wrapper
2. **`Ticket::Write`** - Required for cache insertion
3. **`try_catch_up_with_primary()`** - RocksDB method to sync secondary replica with primary
4. **`self.cached.clear()`** - Clears the entire cache structure

---

## Usage in Bot

From `crates/simulator/src/db_simulator/mod.rs`:

```rust
// Line 594 - receives object updates from socket
cache_writer.reload_objects(objects);

// Line 603 - periodic full refresh (every 24 hours)
cache_writer.update_underlying(true);
```

The bot's `CacheWriter` trait is just a local marker combining `ExecutionCacheWrite + ObjectStore`:

```rust
pub trait CacheWriter: ExecutionCacheWrite + ObjectStore {}
impl CacheWriter for WritebackCache {}
```

No changes needed to this bot-side trait - it automatically gains access to the new methods through `ExecutionCacheWrite`.
