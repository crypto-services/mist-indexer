# Authority.rs Modifications

**File:** `crates/sui-core/src/authority.rs`

## 1. Add Imports

At the top of the file, add:

```rust
use crate::cache_update_handler::{indexer_related_object_ids, CacheUpdateHandler};
use crate::tx_handler::TxHandler;
use dashmap::DashSet;
```

## 2. Add Fields to AuthorityState

Find `pub struct AuthorityState` and add these fields:

```rust
pub struct AuthorityState {
    // ... existing fields ...

    /// Handler for streaming cache updates to custom indexer
    pub cache_update_handler: CacheUpdateHandler,

    /// Handler for streaming transaction effects/events to custom indexer
    pub tx_handler: TxHandler,

    /// Set of indexer-related object IDs to monitor
    pub indexer_ids: DashSet<ObjectID>,
}
```

## 3. Initialize Handlers

In the `AuthorityState::new()` or `AuthorityState::new_for_testing()` function, initialize the new fields:

```rust
impl AuthorityState {
    pub async fn new(/* ... existing params ... */) -> Arc<Self> {
        // ... existing initialization ...

        let cache_update_handler = CacheUpdateHandler::new();
        let tx_handler = TxHandler::default();
        let indexer_ids = indexer_related_object_ids();

        Arc::new(Self {
            // ... existing fields ...
            cache_update_handler,
            tx_handler,
            indexer_ids,
        })
    }
}
```

## 4. Modify commit_certificate()

Find the `commit_certificate()` method. After the line that calls `write_transaction_outputs`, add the notification logic:

```rust
async fn commit_certificate(
    &self,
    // ... params ...
) -> SuiResult<TransactionOutputs> {
    // ... existing code ...

    // Find this existing line:
    self.get_cache_writer()
        .write_transaction_outputs(epoch_store.epoch(), Arc::clone(&transaction_outputs));

    // ADD THIS BLOCK AFTER:
    // Notify of changes via socket
    if !certificate.transaction_data().is_system_tx() {
        // 1. Notify cache updates for indexer-related objects
        let changed_objects: Vec<_> = transaction_outputs
            .written
            .iter()
            .filter_map(|(id, obj)| {
                // Check if object is indexer-related or owned by us
                let owned_by_us = std::env::var("OUR_ADDRESS")
                    .ok()
                    .and_then(|addr| ObjectID::from_str(&addr).ok())
                    .map(|target| obj.owner() == &Owner::AddressOwner(target.into()))
                    .unwrap_or(false);

                if self.indexer_ids.contains(id) || owned_by_us {
                    Some((*id, obj.clone()))
                } else {
                    None
                }
            })
            .collect();

        if !changed_objects.is_empty() {
            let handler = self.cache_update_handler.clone();
            tokio::spawn(async move {
                handler.notify_written(changed_objects).await;
            });
        }

        // 2. Stream transaction effects and events
        let raw_events = &transaction_outputs.events;
        if !raw_events.data.is_empty() {
            let tx_digest = certificate.digest();
            let backing_store = self.get_backing_package_store().clone();
            let executor = epoch_store.executor();

            // Convert events to SuiEvent format
            let sui_events: Vec<SuiEvent> = raw_events
                .data
                .iter()
                .enumerate()
                .filter_map(|(seq, event)| {
                    let mut layout_resolver =
                        executor.type_layout_resolver(Box::new(backing_store.as_ref()));
                    match layout_resolver.get_annotated_layout(&event.type_) {
                        Ok(layout) => SuiEvent::try_from(
                            event.clone(),
                            *tx_digest,
                            seq as u64,
                            None,
                            layout,
                        )
                        .ok(),
                        Err(_) => None,
                    }
                })
                .collect();

            if !sui_events.is_empty() {
                let tx_handler = self.tx_handler.clone();
                let effects = transaction_outputs.effects.clone();
                tokio::spawn(async move {
                    let _ = tx_handler.send_tx_effects_and_events(&effects, sui_events).await;
                });
            }
        }
    }

    // ... rest of existing code ...
}
```

## Notes

1. The `OUR_ADDRESS` environment variable should be set to your wallet address to receive cache updates for your owned objects.

2. The `indexer_ids` are loaded from the file specified in `cache_update_handler.rs`. Update the path to match your setup.

3. System transactions are skipped to avoid noise.

4. Events are converted to `SuiEvent` format using the type layout resolver for proper JSON serialization.
