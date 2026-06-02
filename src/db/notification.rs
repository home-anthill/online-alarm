use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use redis::aio::ConnectionManager;
use serde_json::json;

use crate::models::online::Online;

pub const NOTIFICATION_RETENTION_MILLIS: u64 = 90 * 24 * 60 * 60 * 1000;

static NOTIFICATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn next_notification_id(sent_at: u64) -> String {
    let sequence = NOTIFICATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{sent_at}-{sequence}")
}

pub fn notification_key(id: &str) -> String {
    format!("notification:{id}")
}

pub fn notifications_by_api_token_key(api_token: &str) -> String {
    format!("notifications:by_api_token:{api_token}")
}

pub fn api_tokens_for_devices(devices: &[Online]) -> Vec<String> {
    devices.iter().map(|device| device.api_token.clone()).collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn retention_threshold(sent_at: u64) -> u64 {
    sent_at.saturating_sub(NOTIFICATION_RETENTION_MILLIS)
}

pub struct SentNotification<'a> {
    pub id: &'a str,
    pub api_tokens: &'a [String],
    pub sent_at: u64,
    pub title: &'a str,
    pub body: &'a str,
    pub devices: &'a [Online],
    pub provider_message_id: Option<&'a str>,
}

pub async fn save_sent_notification(
    db: &ConnectionManager,
    notification: &SentNotification<'_>,
) -> redis::RedisResult<()> {
    let mut con = db.clone();
    let key = notification_key(notification.id);
    let device_count = notification.devices.len();
    let api_tokens_json =
        serde_json::to_string(notification.api_tokens).expect("serializing api tokens should not fail");
    let devices_json = devices_json(notification.devices);

    let expired_ids_by_api_token = expired_notification_ids_by_api_token(&mut con, notification).await?;

    let mut pipe = redis::pipe();
    pipe.atomic();

    for (api_token, expired_ids) in &expired_ids_by_api_token {
        if expired_ids.is_empty() {
            continue;
        }

        pipe.cmd("ZREM").arg(notifications_by_api_token_key(api_token)).arg(expired_ids);
        for id in expired_ids {
            pipe.cmd("DEL").arg(notification_key(id));
        }
    }

    pipe.cmd("HSET")
        .arg(&key)
        .arg("id")
        .arg(notification.id)
        .arg("apiToken")
        .arg(notification.api_tokens.first().map(String::as_str).unwrap_or(""))
        .arg("apiTokens")
        .arg(api_tokens_json)
        .arg("sentAt")
        .arg(notification.sent_at)
        .arg("title")
        .arg(notification.title)
        .arg("body")
        .arg(notification.body)
        .arg("deviceCount")
        .arg(device_count)
        .arg("devices")
        .arg(devices_json)
        .arg("provider")
        .arg("fcm")
        .arg("providerMessageId")
        .arg(notification.provider_message_id.unwrap_or(""));

    for api_token in notification.api_tokens {
        pipe.cmd("ZADD").arg(notifications_by_api_token_key(api_token)).arg(notification.sent_at).arg(notification.id);
    }

    pipe.query_async(&mut con).await
}

async fn expired_notification_ids_by_api_token(
    con: &mut ConnectionManager,
    notification: &SentNotification<'_>,
) -> redis::RedisResult<Vec<(String, Vec<String>)>> {
    let retention_threshold = retention_threshold(notification.sent_at);
    let mut expired_ids_by_api_token = Vec::with_capacity(notification.api_tokens.len());

    for api_token in notification.api_tokens {
        let expired_ids = redis::cmd("ZRANGEBYSCORE")
            .arg(notifications_by_api_token_key(api_token))
            .arg("-inf")
            .arg(format!("({retention_threshold}"))
            .query_async(con)
            .await?;
        expired_ids_by_api_token.push((api_token.clone(), expired_ids));
    }

    Ok(expired_ids_by_api_token)
}

fn devices_json(devices: &[Online]) -> String {
    let devices = devices
        .iter()
        .map(|device| {
            json!({
                "deviceUuid": device.device_uuid,
                "featureUuid": device.feature_uuid,
                "createdAt": device.created_at,
                "modifiedAt": device.modified_at,
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&devices).expect("serializing notification devices should not fail")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        NOTIFICATION_RETENTION_MILLIS, api_tokens_for_devices, devices_json, next_notification_id, notification_key,
        notifications_by_api_token_key, retention_threshold,
    };
    use crate::models::online::Online;

    fn online(api_token: &str, device_uuid: &str, feature_uuid: &str) -> Online {
        Online {
            api_token: api_token.to_string(),
            device_uuid: device_uuid.to_string(),
            feature_uuid: feature_uuid.to_string(),
            fcm_token: "fcm-token".to_string(),
            notification_silenced: false,
            created_at: 1,
            modified_at: 2,
        }
    }

    #[test]
    fn notification_keys_match_redis_schema() {
        assert_eq!("notification:notification-id", notification_key("notification-id"));
        assert_eq!("notifications:by_api_token:api-token", notifications_by_api_token_key("api-token"));
    }

    #[test]
    fn api_tokens_for_devices_returns_unique_sorted_tokens() {
        let tokens = api_tokens_for_devices(&[
            online("api-token-b", "device-b", "feature-b"),
            online("api-token-a", "device-a", "feature-a"),
            online("api-token-b", "device-c", "feature-c"),
        ]);

        assert_eq!(vec!["api-token-a", "api-token-b"], tokens);
    }

    #[test]
    fn devices_json_stores_device_feature_and_timestamps() {
        let devices = devices_json(&[online("api-token", "device-a", "feature-a")]);

        assert_eq!(r#"[{"createdAt":1,"deviceUuid":"device-a","featureUuid":"feature-a","modifiedAt":2}]"#, devices);
    }

    #[test]
    fn next_notification_id_includes_timestamp() {
        let id = next_notification_id(123);

        assert!(id.starts_with("123-"));
    }

    #[test]
    fn retention_threshold_keeps_only_last_ninety_days() {
        let sent_at = NOTIFICATION_RETENTION_MILLIS + 1;

        assert_eq!(1, retention_threshold(sent_at));
        assert_eq!(0, retention_threshold(1));
    }
}
