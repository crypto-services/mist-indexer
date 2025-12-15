# sui-core lib.rs Modifications

**File:** `crates/sui-core/src/lib.rs`

## Add Module Exports

Add these public module declarations to expose the new handlers:

```rust
// ... existing module declarations ...

pub mod cache_update_handler;
pub mod tx_handler;
```

This makes the handlers accessible from other crates (like `sui-node`) that need to reference them.
