# Sui Node Modifications for MEV Bot

This directory contains all modifications needed to make MystenLabs/sui work with this MEV bot.

## Required Components

| File                                                           | Type   | Purpose                                       |
| -------------------------------------------------------------- | ------ | --------------------------------------------- |
| [01_tx_handler.rs](01_tx_handler.rs)                           | New    | Streams tx effects/events over Unix socket    |
| [02_cache_update_handler.rs](02_cache_update_handler.rs)       | New    | Streams object updates over Unix socket       |
| [03_authority_changes.md](03_authority_changes.md)             | Modify | Integrates handlers into AuthorityState       |
| [04_writeback_cache_changes.md](04_writeback_cache_changes.md) | Modify | Adds reload_objects/update_underlying methods |
| [05_sui_core_lib.md](05_sui_core_lib.md)                       | Modify | Exports new modules                           |
| [06_cargo_changes.md](06_cargo_changes.md)                     | Modify | Adds interprocess dependency                  |

## Socket Paths (Defaults)

| Socket            | Default Path                  | Env Variable               | Purpose                               |
| ----------------- | ----------------------------- | -------------------------- | ------------------------------------- |
| Transaction Relay | `/tmp/sui_tx.sock`            | `SUI_TX_SOCKET_PATH`       | Effects + events to PublicTxCollector |
| Cache Updates     | `/tmp/sui_cache_updates.sock` | `SUI_CACHE_SOCKET_PATH`    | Object updates to DBSimulator         |
| Pool IDs File     | `/tmp/indexer_ids.txt`        | `SUI_INDEXER_OBJECTS_PATH` | Objects to monitor for cache updates  |

## Wire Protocols

### Transaction Relay (tx_handler)

```
[effects_len: u32 BE][effects: bincode][events_len: u32 BE][events: JSON]
```

### Cache Updates (cache_update_handler)

```
[len: u32 LE][payload: BCS Vec<(ObjectID, Object)>]
```

## Environment Variables

| Variable                   | Default                       | Purpose                                                 |
| -------------------------- | ----------------------------- | ------------------------------------------------------- |
| `OUR_ADDRESS`              | (none)                        | Your address - triggers cache updates for owned objects |
| `SUI_TX_SOCKET_PATH`       | `/tmp/sui_tx.sock`            | Socket path for tx effects/events streaming             |
| `SUI_CACHE_SOCKET_PATH`    | `/tmp/sui_cache_updates.sock` | Socket path for cache update streaming                  |
| `SUI_INDEXER_OBJECTS_PATH` | `/tmp/indexer_ids.txt`        | Path to file with pool object IDs to monitor            |

## Build & Run

```bash
cargo build -r --bin sui-node
./target/release/sui-node --config-path /path/to/fullnode.yaml
```

## Update

Ensure the original repo is set as `upstream`

```bash
git remote add upstream git@github.com:MystenLabs/sui.git
git remote set-url --push upstream DISABLE
```

Checkout the patched branch

```bash
git checkout indexer-patch
```

Apply changes from upstream

```bash
git fetch upstream
git rebase upstream/testnet-v.1.62.0
```

Review any merge issues and test build:

```bash
cargo build -r --bin sui-node
```

Once you're happy tag the release and push to GH (requires force as rebase overwrites history)

```bash
git tag testnet_v1.62.0-patched
git push origin indexer-patch --force-with-lease
git push origin testnet_v1.62.0-patched
```

Additionally there is a patch file in `.mod/patches/*` to automate application to fresh versions if required.