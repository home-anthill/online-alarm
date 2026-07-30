use std::collections::HashMap;

use redis::{AsyncCommands, aio::ConnectionManager};
use tracing::{error, warn};

use crate::models::alarm::AlarmEvent;
use crate::models::online::Online;

pub fn alarm_settings_key(device_uuid: &str, feature_uuid: &str, is_testing: bool) -> String {
    let prefix = if is_testing { "test-alarm-settings" } else { "alarm-settings" };
    format!("{prefix}:{device_uuid}:{feature_uuid}")
}

pub fn pending_alarms_key(is_testing: bool) -> &'static str {
    if is_testing { "test-alarms:pending" } else { "alarms:pending" }
}

pub async fn apply_notification_preferences(
    alarms_db: &ConnectionManager,
    online: &mut [Online],
    is_testing: bool,
) -> redis::RedisResult<()> {
    let mut con = alarms_db.clone();
    for device_feature in online {
        let setting: Option<String> = con
            .hget(
                alarm_settings_key(&device_feature.device_uuid, &device_feature.feature_uuid, is_testing),
                "notificationSilenced",
            )
            .await?;
        if let Some(setting) = setting {
            device_feature.notification_silenced = setting == "true";
        }
    }
    Ok(())
}

pub async fn find_pending_alarms(
    alarms_db: &ConnectionManager,
    online_db: &ConnectionManager,
    is_testing: bool,
) -> redis::RedisResult<Vec<AlarmEvent>> {
    let mut alarms_con = alarms_db.clone();
    let mut online_con = online_db.clone();
    let pending_key = pending_alarms_key(is_testing);
    let event_ids: Vec<String> = alarms_con.zrange(pending_key, 0, -1).await?;
    let mut events = Vec::with_capacity(event_ids.len());

    for event_id in event_ids {
        let value: HashMap<String, String> = alarms_con.hgetall(&event_id).await?;
        if value.is_empty() {
            let _: usize = alarms_con.zrem(pending_key, &event_id).await?;
            continue;
        }

        let Some(mut event) = alarm_from_hash(&event_id, &value) else {
            warn!(target: "app", "discarding malformed pending alarm id={}", event_id);
            acknowledge_alarm_ids(&mut alarms_con, pending_key, &[event_id]).await?;
            continue;
        };
        let silenced: Option<String> = alarms_con
            .hget(alarm_settings_key(&event.device_uuid, &event.feature_uuid, is_testing), "notificationSilenced")
            .await?;
        if silenced.as_deref() == Some("true") {
            acknowledge_alarm_ids(&mut alarms_con, pending_key, &[event.id]).await?;
            continue;
        }

        let fcm_token: Option<String> = online_con.hget("fcm_by_api_token", &event.api_token).await?;
        let Some(fcm_token) = fcm_token.filter(|token| !token.is_empty()) else {
            warn!(target: "app", "pending alarm has no FCM token id={}", event.id);
            continue;
        };
        event.fcm_token = fcm_token;
        events.push(event);
    }
    Ok(events)
}

pub async fn acknowledge_alarm_events(
    alarms_db: &ConnectionManager,
    events: &[AlarmEvent],
    is_testing: bool,
) -> redis::RedisResult<()> {
    let mut con = alarms_db.clone();
    let ids = events.iter().map(|event| event.id.clone()).collect::<Vec<_>>();
    acknowledge_alarm_ids(&mut con, pending_alarms_key(is_testing), &ids).await
}

async fn acknowledge_alarm_ids(
    con: &mut ConnectionManager,
    pending_key: &str,
    ids: &[String],
) -> redis::RedisResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut pipe = redis::pipe();
    pipe.atomic().cmd("ZREM").arg(pending_key).arg(ids);
    for id in ids {
        pipe.cmd("DEL").arg(id);
    }
    pipe.query_async(con).await
}

fn alarm_from_hash(id: &str, value: &HashMap<String, String>) -> Option<AlarmEvent> {
    let field = |name: &str| {
        value.get(name).cloned().or_else(|| {
            error!(target: "app", "pending alarm {} is missing {}", id, name);
            None
        })
    };
    let parse_date = |name: &str| {
        field(name)?.parse::<u64>().ok().or_else(|| {
            error!(target: "app", "pending alarm {} has invalid {}", id, name);
            None
        })
    };

    let stored_id = field("id")?;
    if stored_id != id {
        error!(target: "app", "pending alarm {} has mismatched stored id", id);
        return None;
    }

    Some(AlarmEvent {
        id: stored_id,
        api_token: field("apiToken")?,
        device_uuid: field("deviceUuid")?,
        feature_uuid: field("featureUuid")?,
        alarm_type: field("alarmType")?,
        payload: field("payload")?,
        created_at: parse_date("createdAt")?,
        received_at: parse_date("receivedAt")?,
        fcm_token: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::{alarm_from_hash, alarm_settings_key, pending_alarms_key};

    #[test]
    fn database_keys_use_alarm_namespace() {
        assert_eq!("alarm-settings:device:feature", alarm_settings_key("device", "feature", false));
        assert_eq!("test-alarm-settings:device:feature", alarm_settings_key("device", "feature", true));
        assert_eq!("alarms:pending", pending_alarms_key(false));
    }

    #[test]
    fn parses_complete_alarm_hash() {
        let value = HashMap::from([
            ("id".to_string(), "alarm-id".to_string()),
            ("apiToken".to_string(), "api-token".to_string()),
            ("deviceUuid".to_string(), "device".to_string()),
            ("featureUuid".to_string(), "feature".to_string()),
            ("alarmType".to_string(), "motion".to_string()),
            ("payload".to_string(), r#"{"value":1}"#.to_string()),
            ("createdAt".to_string(), "10".to_string()),
            ("receivedAt".to_string(), "11".to_string()),
        ]);

        let event = alarm_from_hash("alarm-id", &value).expect("valid alarm");

        assert_eq!(event.alarm_type, "motion");
        assert_eq!(event.created_at, 10);
        assert_eq!(event.received_at, 11);
    }
}
