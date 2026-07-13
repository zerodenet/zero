use std::sync::Arc;

use super::{VmessMuxConnectionPool, VmessMuxIdentity, VmessMuxPoolKey};
use crate::VmessCipher;

#[tokio::test]
async fn eviction_through_a_bridge_clone_removes_cached_mux_connections() {
    let pool = VmessMuxConnectionPool::new();
    let bridge_pool = pool.clone();
    let identity = VmessMuxIdentity::from_parts([9; 16], "none".to_owned(), VmessCipher::None);
    let key = VmessMuxPoolKey::from_config_parts(
        "vmess.test".to_owned(),
        443,
        identity,
        None,
        None,
        None,
    )
    .expect("VMess mux pool key");
    let (stream, _peer) = tokio::io::duplex(64);
    let connection = Arc::new(key.clone().into_pool_conn(stream, 4));
    pool.pool
        .lock()
        .expect("VMess mux pool lock")
        .insert(key, connection);

    assert_eq!(pool.pool.lock().expect("VMess mux pool lock").len(), 1);
    bridge_pool.evict_all();
    assert!(pool.pool.lock().expect("VMess mux pool lock").is_empty());
}
