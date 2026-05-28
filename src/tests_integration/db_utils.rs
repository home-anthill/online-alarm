use futures::StreamExt;
use redis::AsyncCommands;

pub async fn clean_test_keys(con: &mut redis::aio::ConnectionManager) {
    let iter = con.scan_match::<&str, String>("test_*").await.expect("scan test keys");
    let keys = iter.map(|result| result.expect("read test key")).collect::<Vec<_>>().await;
    if !keys.is_empty() {
        let _: () = con.del(keys).await.expect("delete test keys");
    }
    let _: () = con.del("online_prod-device_feature_prod-feature").await.expect("delete production fixture key");
}

pub async fn hset_multiple(con: &mut redis::aio::ConnectionManager, key: &str, items: &[(&str, &str)]) {
    let _: () = con.hset_multiple(key, items).await.expect("seed redis hash");
}
