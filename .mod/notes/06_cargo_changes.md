# Cargo.toml Modifications

## sui-core/Cargo.toml

**File:** `crates/sui-core/Cargo.toml`

Add the `interprocess` dependency for Unix socket support:

```toml
[dependencies]
# ... existing dependencies ...

# For Unix socket IPC (NEW - required for tx_handler and cache_update_handler)
interprocess = { version = "2", features = ["tokio"] }
```

**Note:** The following dependencies are already present in sui-core as workspace dependencies - no changes needed:

- `dashmap.workspace = true` (used for indexer_ids DashSet)
- `serde_json.workspace = true` (used for JSON event serialization)
- `bincode.workspace = true` (used for effects serialization)

## Workspace Cargo.toml

Since Sui uses workspace dependencies, add `interprocess` to the root `Cargo.toml`:

```toml
[workspace.dependencies]
interprocess = { version = "2", features = ["tokio"] }
```

Then in `crates/sui-core/Cargo.toml`:

```toml
[dependencies]
interprocess.workspace = true
```

## Notes

1. `interprocess` version 2.x is used for async Unix socket support
2. The `tokio` feature is required for async stream handling
3. Only `interprocess` needs to be added - other dependencies (`dashmap`, `serde_json`, `bincode`) are already present
