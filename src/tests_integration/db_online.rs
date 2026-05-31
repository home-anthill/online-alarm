use pretty_assertions::assert_eq;
use redis::AsyncCommands;

use super::db_utils::{clean_test_keys, hset_multiple, redis_connection};
use online::db::online::find_all;

#[tokio::test]
async fn find_all_reads_valid_testing_hashes_and_skips_invalid_redis_entries() {
    let mut con = redis_connection().await;
    clean_test_keys(&mut con).await;

    hset_multiple(
        &mut con,
        "test_device-a_feature_feature-a",
        &[
            ("apiToken", "api-token-a"),
            ("fcmToken", "fcm-token-a"),
            ("createdAt", "1710000000001"),
            ("modifiedAt", "1710000000002"),
        ],
    )
    .await;
    hset_multiple(
        &mut con,
        "test_device-b_feature_feature-b",
        &[
            ("apiToken", "api-token-b"),
            ("fcmToken", "fcm-token-b"),
            ("createdAt", "1710000000003"),
            ("modifiedAt", "1710000000004"),
        ],
    )
    .await;
    let _: () = con.hset("test_malformed", "apiToken", "api-token").await.expect("seed malformed redis hash");
    hset_multiple(
        &mut con,
        "test_missing-fcm_feature_feature",
        &[("apiToken", "api-token"), ("createdAt", "1"), ("modifiedAt", "2")],
    )
    .await;
    hset_multiple(
        &mut con,
        "test_empty-fcm_feature_feature",
        &[("apiToken", "api-token"), ("fcmToken", ""), ("createdAt", "1"), ("modifiedAt", "2")],
    )
    .await;
    hset_multiple(
        &mut con,
        "test_invalid-created_feature_feature",
        &[("apiToken", "api-token"), ("fcmToken", "fcm-token"), ("createdAt", "bad"), ("modifiedAt", "2")],
    )
    .await;
    hset_multiple(
        &mut con,
        "online_prod-device_feature_prod-feature",
        &[("apiToken", "api-token-prod"), ("fcmToken", "fcm-token-prod"), ("createdAt", "1"), ("modifiedAt", "2")],
    )
    .await;

    let devices = find_all(&con, true).await.expect("find all devices");
    let mut devices = devices;
    devices.sort_by_key(|device| device.cache_key());
    clean_test_keys(&mut con).await;

    assert_eq!(2, devices.len());
    assert_eq!("api-token-a", devices[0].api_token);
    assert_eq!("device-a", devices[0].device_uuid);
    assert_eq!("feature-a", devices[0].feature_uuid);
    assert_eq!("fcm-token-a", devices[0].fcm_token);
    assert_eq!(1710000000001, devices[0].created_at);
    assert_eq!(1710000000002, devices[0].modified_at);
    assert_eq!("api-token-b", devices[1].api_token);
    assert_eq!("device-b", devices[1].device_uuid);
    assert_eq!("feature-b", devices[1].feature_uuid);
    assert_eq!("fcm-token-b", devices[1].fcm_token);
    assert_eq!(1710000000003, devices[1].created_at);
    assert_eq!(1710000000004, devices[1].modified_at);
}
