use std::collections::HashMap;

use pretty_assertions::assert_eq;
use redis::AsyncCommands;
use serde_json::json;

use super::db_utils::{clean_test_keys, redis_connection};
use online::db::notification::{
    NOTIFICATION_RETENTION_MILLIS, SentNotification, notification_key, notifications_by_api_token_key,
    save_sent_notification,
};
use online::models::online::Online;

fn online(api_token: &str, device_uuid: &str, feature_uuid: &str) -> Online {
    Online {
        api_token: api_token.to_string(),
        device_uuid: device_uuid.to_string(),
        feature_uuid: feature_uuid.to_string(),
        fcm_token: "fcm-token".to_string(),
        created_at: 1710000000001,
        modified_at: 1710000000002,
    }
}

#[tokio::test]
async fn save_sent_notification_writes_hash_and_api_token_index() {
    let mut con = redis_connection().await;
    clean_test_keys(&mut con).await;

    let api_tokens = vec!["test-api-token-a".to_string()];
    let devices = vec![online("test-api-token-a", "device-a", "feature-a")];
    let notification = SentNotification {
        id: "test-notification-a",
        api_tokens: &api_tokens,
        sent_at: 1717000000000,
        title: "home anthill",
        body: "Device is offline",
        devices: &devices,
        provider_message_id: Some("projects/home-anthill/messages/message-a"),
    };

    save_sent_notification(&con, &notification).await.expect("save sent notification");

    let hash: HashMap<String, String> =
        con.hgetall(notification_key("test-notification-a")).await.expect("read notification hash");
    let indexed_ids: Vec<String> = con
        .zrevrange(notifications_by_api_token_key("test-api-token-a"), 0, -1)
        .await
        .expect("read notification index");
    let score: f64 = con
        .zscore(notifications_by_api_token_key("test-api-token-a"), "test-notification-a")
        .await
        .expect("read notification score");

    clean_test_keys(&mut con).await;

    assert_eq!("test-notification-a", hash["id"]);
    assert_eq!("test-api-token-a", hash["apiToken"]);
    assert_eq!(r#"["test-api-token-a"]"#, hash["apiTokens"]);
    assert_eq!("1717000000000", hash["sentAt"]);
    assert_eq!("home anthill", hash["title"]);
    assert_eq!("Device is offline", hash["body"]);
    assert_eq!("1", hash["deviceCount"]);
    assert_eq!("fcm", hash["provider"]);
    assert_eq!("projects/home-anthill/messages/message-a", hash["providerMessageId"]);
    assert_eq!(vec!["test-notification-a"], indexed_ids);
    assert_eq!(1717000000000.0, score);

    let devices_json: serde_json::Value = serde_json::from_str(&hash["devices"]).expect("parse devices json");
    assert_eq!(
        json!([{
            "deviceUuid": "device-a",
            "featureUuid": "feature-a",
            "createdAt": 1710000000001u64,
            "modifiedAt": 1710000000002u64,
        }]),
        devices_json
    );
}

#[tokio::test]
async fn save_sent_notification_indexes_all_api_tokens_in_grouped_notification() {
    let mut con = redis_connection().await;
    clean_test_keys(&mut con).await;

    let api_tokens = vec!["test-api-token-a".to_string(), "test-api-token-b".to_string()];
    let devices =
        vec![online("test-api-token-a", "device-a", "feature-a"), online("test-api-token-b", "device-b", "feature-b")];
    let notification = SentNotification {
        id: "test-notification-grouped",
        api_tokens: &api_tokens,
        sent_at: 1717000005000,
        title: "home anthill",
        body: "2 devices are offline",
        devices: &devices,
        provider_message_id: None,
    };

    save_sent_notification(&con, &notification).await.expect("save sent notification");

    let token_a_ids: Vec<String> = con
        .zrevrange(notifications_by_api_token_key("test-api-token-a"), 0, -1)
        .await
        .expect("read token a notification index");
    let token_b_ids: Vec<String> = con
        .zrevrange(notifications_by_api_token_key("test-api-token-b"), 0, -1)
        .await
        .expect("read token b notification index");
    let hash: HashMap<String, String> =
        con.hgetall(notification_key("test-notification-grouped")).await.expect("read notification hash");

    clean_test_keys(&mut con).await;

    assert_eq!(vec!["test-notification-grouped"], token_a_ids);
    assert_eq!(vec!["test-notification-grouped"], token_b_ids);
    assert_eq!(r#"["test-api-token-a","test-api-token-b"]"#, hash["apiTokens"]);
    assert_eq!("test-api-token-a", hash["apiToken"]);
    assert_eq!("2", hash["deviceCount"]);
    assert_eq!("", hash["providerMessageId"]);
}

#[tokio::test]
async fn save_sent_notification_removes_notifications_older_than_ninety_days() {
    let mut con = redis_connection().await;
    clean_test_keys(&mut con).await;

    let api_token = "test-api-token-retention";
    let current_sent_at = NOTIFICATION_RETENTION_MILLIS + 10_000;
    let expired_sent_at = 9_999;
    let retained_sent_at = 10_000;
    let index_key = notifications_by_api_token_key(api_token);

    let _: () = con
        .hset_multiple(
            notification_key("test-expired-notification"),
            &[("id", "test-expired-notification"), ("body", "expired")],
        )
        .await
        .expect("seed expired notification hash");
    let _: () = con
        .hset_multiple(
            notification_key("test-retained-notification"),
            &[("id", "test-retained-notification"), ("body", "retained")],
        )
        .await
        .expect("seed retained notification hash");
    let _: () = con
        .zadd(&index_key, "test-expired-notification", expired_sent_at)
        .await
        .expect("seed expired notification index");
    let _: () = con
        .zadd(&index_key, "test-retained-notification", retained_sent_at)
        .await
        .expect("seed retained notification index");

    let api_tokens = vec![api_token.to_string()];
    let devices = vec![online(api_token, "device-a", "feature-a")];
    let notification = SentNotification {
        id: "test-current-notification",
        api_tokens: &api_tokens,
        sent_at: current_sent_at,
        title: "home anthill",
        body: "Device is offline",
        devices: &devices,
        provider_message_id: None,
    };

    save_sent_notification(&con, &notification).await.expect("save sent notification");

    let ids: Vec<String> = con.zrange(&index_key, 0, -1).await.expect("read notification index");
    let expired_exists: bool =
        con.exists(notification_key("test-expired-notification")).await.expect("check expired notification hash");
    let retained_exists: bool =
        con.exists(notification_key("test-retained-notification")).await.expect("check retained notification hash");
    let current_exists: bool =
        con.exists(notification_key("test-current-notification")).await.expect("check current notification hash");

    clean_test_keys(&mut con).await;

    assert_eq!(vec!["test-retained-notification", "test-current-notification"], ids);
    assert!(!expired_exists);
    assert!(retained_exists);
    assert!(current_exists);
}
