use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use dashmap::DashSet;
use once_cell::sync::Lazy;
use sui_types::base_types::ObjectID;
use sui_types::object::Object;
use tokio::io::AsyncWriteExt;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{error, info};

/// Socket path for cache update streaming.
/// Configurable via SUI_CACHE_SOCKET_PATH environment variable.
pub static SOCKET_PATH: Lazy<String> = Lazy::new(|| {
    std::env::var("SUI_CACHE_SOCKET_PATH")
        .unwrap_or_else(|_| "/tmp/sui_cache_updates.sock".to_string())
});

/// Path to file containing indexer-related object IDs to monitor.
/// Configurable via SUI_INDEXER_OBJECTS_PATH environment variable.
pub static INDEXER_OBJECTS_PATH: Lazy<String> = Lazy::new(|| {
    std::env::var("SUI_INDEXER_OBJECTS_PATH")
        .unwrap_or_else(|_| "/tmp/indexer_ids.txt".to_string())
});

/// Load indexer-related object IDs from file.
/// These are the objects we want to monitor for changes.
pub fn indexer_related_object_ids() -> DashSet<ObjectID> {
    let content = match std::fs::read_to_string(&*INDEXER_OBJECTS_PATH) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read indexer_ids file: {}", e);
            return DashSet::new();
        }
    };

    let set = DashSet::new();
    for line in content.trim().lines() {
        if let Ok(id) = line.parse() {
            set.insert(id);
        }
    }
    set
}

#[derive(Debug)]
pub struct CacheUpdateHandler {
    socket_path: Arc<PathBuf>,
    connections: Arc<Mutex<Vec<UnixStream>>>,
    running: Arc<AtomicBool>,
}

impl Clone for CacheUpdateHandler {
    fn clone(&self) -> Self {
        Self {
            socket_path: Arc::clone(&self.socket_path),
            connections: Arc::clone(&self.connections),
            running: Arc::clone(&self.running),
        }
    }
}

impl Default for CacheUpdateHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheUpdateHandler {
    pub fn new() -> Self {
        let socket_path = Arc::new(PathBuf::from(&*SOCKET_PATH));

        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(socket_path.as_ref());

        let listener = UnixListener::bind(socket_path.as_ref())
            .expect("Failed to bind cache update Unix socket");

        let connections = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(true));

        let connections_clone = Arc::clone(&connections);
        let running_clone = Arc::clone(&running);

        // Spawn connection acceptor task
        tokio::spawn(async move {
            while running_clone.load(Ordering::SeqCst) {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        info!("New client connected to cache update socket");
                        connections_clone.lock().await.push(stream);
                    }
                    Err(e) => {
                        error!("Error accepting cache update connection: {}", e);
                    }
                }
            }
        });

        Self {
            socket_path,
            connections,
            running,
        }
    }

    /// Notify all connected clients of written objects.
    ///
    /// Wire format:
    /// - len (4 bytes, little-endian u32)
    /// - payload (BCS serialized Vec<(ObjectID, Object)>)
    pub async fn notify_written(&self, objects: Vec<(ObjectID, Object)>) {
        let serialized = match bcs::to_bytes(&objects) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize cache update: {}", e);
                return;
            }
        };

        let len_bytes = (serialized.len() as u32).to_le_bytes();

        let mut connections = self.connections.lock().await;
        let mut i = 0;

        while i < connections.len() {
            let stream = &mut connections[i];

            let result = async {
                stream.write_all(&len_bytes).await?;
                stream.write_all(&serialized).await?;
                Ok::<_, std::io::Error>(())
            }
            .await;

            if result.is_err() {
                // Remove failed connection
                connections.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

impl Drop for CacheUpdateHandler {
    fn drop(&mut self) {
        // Only clean up when this is the last reference
        if Arc::strong_count(&self.socket_path) == 1 {
            self.running.store(false, Ordering::SeqCst);
            let _ = std::fs::remove_file(self.socket_path.as_ref());
        }
    }
}
