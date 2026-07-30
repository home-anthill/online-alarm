use redis::AsyncCommands;

use super::db_utils::{clean_test_keys, redis_connection};
use alarm_notifier::db::alarm::{
    acknowledge_alarm_events, alarm_settings_key, apply_notification_preferences, find_pending_alarms,
    pending_alarms_key,
};
use alarm_notifier::models::online::Online;

#[tokio::test]
async fn alarm_preferences_pending_events_and_acknowledgement_use_test_database() {
    let mut con = redis_connection().await;
    clean_test_keys(&mut con).await;

    let event_id = "test-alarm:device:feature:feature:type:motion:nonce:nonce";
    let _: () = con
        .hset_multiple(
            event_id,
            &[
                ("id", event_id),
                ("apiToken", "test-api-token-alarm"),
                ("deviceUuid", "device"),
                ("featureUuid", "feature"),
                ("alarmType", "motion"),
                ("payload", r#"{"value":1}"#),
                ("createdAt", "1710000000001"),
                ("receivedAt", "1710000000002"),
            ],
        )
        .await
        .expect("seed alarm hash");
    let _: () = con.zadd(pending_alarms_key(true), event_id, 1710000000002u64).await.expect("seed pending index");
    let _: () = con.hset("fcm_by_api_token", "test-api-token-alarm", "test-fcm-token").await.expect("seed FCM lookup");

    let events = find_pending_alarms(&con, &con, true).await.expect("read pending alarms");
    assert_eq!(1, events.len());
    assert_eq!("motion", events[0].alarm_type);
    assert_eq!("test-fcm-token", events[0].fcm_token);

    acknowledge_alarm_events(&con, &events, true).await.expect("acknowledge alarm");
    let exists: bool = con.exists(event_id).await.expect("check event hash");
    let pending: Vec<String> = con.zrange(pending_alarms_key(true), 0, -1).await.expect("check pending index");
    assert!(!exists);
    assert!(pending.is_empty());

    let mut online = vec![Online {
        api_token: "test-api-token-alarm".to_string(),
        device_uuid: "device".to_string(),
        feature_uuid: "feature".to_string(),
        fcm_token: "test-fcm-token".to_string(),
        notification_silenced: false,
        created_at: 1,
        modified_at: 2,
    }];
    let _: () = con
        .hset(alarm_settings_key("device", "feature", true), "notificationSilenced", "true")
        .await
        .expect("seed alarm setting");
    apply_notification_preferences(&con, &mut online, true).await.expect("read alarm setting");
    assert!(online[0].notification_silenced);

    clean_test_keys(&mut con).await;
}
