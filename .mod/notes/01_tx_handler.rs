// Location: crates/sui-core/src/tx_handler.rs
// Type: NEW FILE
//
// Purpose: Streams transaction effects and events over Unix socket

use std::sync::Arc;

use anyhow::Result;
use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericNamespaced, ListenerOptions,
};
use once_cell::sync::Lazy;
use sui_json_rpc_types::SuiEvent;
use sui_types::effects::TransactionEffects;
use tokio::{io::AsyncWriteExt, sync::Mutex};

/// Socket path for transaction effects/events streaming.
/// Configurable via SUI_TX_SOCKET_PATH environment variable.
pub static TX_SOCKET_PATH: Lazy<String> = Lazy::new(|| {
    std::env::var("SUI_TX_SOCKET_PATH")
        .unwrap_or_else(|_| "/tmp/sui_tx.sock".to_string())
});

#[derive(Clone)]
pub struct TxHandler {
    path: String,
    conns: Arc<Mutex<Vec<Stream>>>,
}

impl Default for TxHandler {
    fn default() -> Self {
        Self::new(&TX_SOCKET_PATH)
    }
}

impl Drop for TxHandler {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl TxHandler {
    pub fn new(path: &str) -> Self {
        let _ = std::fs::remove_file(path);

        let name = path
            .to_ns_name::<GenericNamespaced>()
            .expect("Invalid tx socket path");
        let opts = ListenerOptions::new().name(name);
        let listener = opts.create_tokio().expect("Failed to bind tx socket");
        let conns = Arc::new(Mutex::new(vec![]));
        let conns_clone = conns.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok(conn) => {
                        conns_clone.lock().await.push(conn);
                    }
                    Err(_) => continue,
                }
            }
        });

        Self {
            path: path.to_string(),
            conns,
        }
    }

    /// Send transaction effects and events to all connected clients.
    ///
    /// Wire format:
    /// - effects_len (4 bytes, big-endian u32)
    /// - effects (bincode serialized TransactionEffects)
    /// - events_len (4 bytes, big-endian u32)
    /// - events (JSON serialized Vec<SuiEvent>)
    pub async fn send_tx_effects_and_events(
        &self,
        effects: &TransactionEffects,
        events: Vec<SuiEvent>,
    ) -> Result<()> {
        let effects_bytes = bincode::serialize(effects)?;
        let events_bytes = serde_json::to_vec(&events)?;

        let effects_len_bytes = (effects_bytes.len() as u32).to_be_bytes();
        let events_len_bytes = (events_bytes.len() as u32).to_be_bytes();

        let mut conns = self.conns.lock().await;
        let mut active_conns = Vec::new();

        while let Some(mut conn) = conns.pop() {
            let result: Result<()> = async {
                conn.write_all(&effects_len_bytes).await?;
                conn.write_all(&effects_bytes).await?;
                conn.write_all(&events_len_bytes).await?;
                conn.write_all(&events_bytes).await?;
                Ok(())
            }
            .await;

            if result.is_ok() {
                active_conns.push(conn);
            }
            // Failed connections are dropped (not re-added)
        }

        *conns = active_conns;
        Ok(())
    }
}
