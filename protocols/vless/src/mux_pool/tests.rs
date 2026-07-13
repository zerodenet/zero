use std::sync::Arc;

use super::{MuxConnectionPool, MuxIdentity, PoolKey};

#[tokio::test]
async fn eviction_through_a_bridge_clone_removes_cached_mux_connections() {
    let pool = MuxConnectionPool::new();
    let bridge_pool = pool.clone();
    let identity = MuxIdentity::from_uuid([7; 16]);
    let key = PoolKey::from_config_parts("vless.test".to_owned(), 443, identity, None, None, None);
    let (stream, _peer) = tokio::io::duplex(64);
    let connection = Arc::new(key.clone().into_pool_conn(stream, 4));
    pool.pool
        .lock()
        .expect("mux pool lock")
        .insert(key, connection);

    assert_eq!(pool.pool.lock().expect("mux pool lock").len(), 1);
    bridge_pool.evict_all();
    assert!(pool.pool.lock().expect("mux pool lock").is_empty());
}
