use std::time::Duration;

use futures::StreamExt;
use redis::AsyncCommands;

const TEST_REDIS_URI: &str = "redis://localhost:6379/15";

pub async fn redis_connection() -> redis::aio::ConnectionManager {
    let client = redis::Client::open(TEST_REDIS_URI).expect("valid default redis test url");
    tokio::time::timeout(Duration::from_secs(2), client.get_connection_manager())
        .await
        .unwrap_or_else(|_| panic!("timed out connecting to Redis at {TEST_REDIS_URI}"))
        .unwrap_or_else(|_| panic!("connect to Redis at {TEST_REDIS_URI}"))
}

pub async fn clean_test_keys(con: &mut redis::aio::ConnectionManager) {
    let mut keys = vec![];

    for pattern in [
        "test_*",
        "test-alarm:*",
        "test-alarm-settings:*",
        "test-alarms:*",
        "notification:test-*",
        "notifications:by_api_token:test-*",
    ] {
        let iter = con.scan_match::<&str, String>(pattern).await.expect("scan test keys");
        keys.extend(iter.map(|result| result.expect("read test key")).collect::<Vec<_>>().await);
    }

    if !keys.is_empty() {
        let _: () = con.del(keys).await.expect("delete test keys");
    }
    let _: () = con.del("online_prod-device_feature_prod-feature").await.expect("delete production fixture key");
    let _: () = con.hdel("fcm_by_api_token", "test-api-token-alarm").await.expect("delete alarm FCM fixture");
}

pub async fn hset_multiple(con: &mut redis::aio::ConnectionManager, key: &str, items: &[(&str, &str)]) {
    let _: () = con.hset_multiple(key, items).await.expect("seed redis hash");
}
