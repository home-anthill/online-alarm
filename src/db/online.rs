use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use redis::{AsyncCommands, aio::ConnectionManager};
use tracing::{error, warn};

use crate::errors::db_error::DbError;
use crate::errors::redis_error::RedisError;
use crate::models::online::Online;

pub async fn find_all(db: &ConnectionManager, is_testing: bool) -> Result<Vec<Online>, anyhow::Error> {
    let mut con = db.clone();
    let mut elements: Vec<Online> = vec![];

    // get all keys with format 'online_<deviceUuid>_feature_<featureUuid>'
    let db_keys_iter =
        con.scan_match::<&str, String>(get_all_keys_pattern(is_testing)).await.map_err(RedisError::GetKeysError)?;
    let db_keys: Vec<String> = db_keys_iter
        .filter_map(|r| async {
            match r {
                Ok(key) => Some(key),
                Err(err) => {
                    warn!(target: "app", "find_all - Redis SCAN iteration error, skipping entry: {:?}", err);
                    None
                }
            }
        })
        .collect::<Vec<String>>()
        .await;
    for db_key in db_keys {
        // hgetall returns the entire redis hash table (with all "key: value")
        let value: HashMap<String, String> = match con.hgetall(&db_key).await {
            Ok(val) => val,
            Err(err) => {
                error!(target: "app", "find_all - HGETALL failed for key {}: {:?}", db_key, err);
                continue;
            }
        };

        let items: Vec<&str> = db_key.split('_').collect();
        let Some(device_uuid) = items.get(1) else {
            warn!(target: "app", "find_all - malformed Redis key (missing device_uuid): {}", db_key);
            continue;
        };
        let Some(feature_uuid) = items.last().filter(|_| items.len() >= 4) else {
            warn!(target: "app", "find_all - malformed Redis key (missing feature_uuid): {}", db_key);
            continue;
        };

        let api_token = match value.get("apiToken") {
            Some(val) => val.as_str(),
            None => {
                error!(target: "app", "find_all - apiToken is missing for key: {}", db_key);
                continue;
            }
        };
        let fcm_token = match value.get("fcmToken") {
            Some(val) if !val.is_empty() => val.as_str(),
            Some(_) => {
                warn!(target: "app", "find_all - fcmToken is empty for key: {}", db_key);
                continue;
            }
            None => {
                warn!(target: "app", "find_all - fcmToken is missing for key: {}", db_key);
                continue;
            }
        };
        let created_at = match get_date_field_by_name(&value, "createdAt") {
            Ok(val) => val,
            Err(_) => {
                error!(target: "app", "find_all - cannot parse createdAt for key: {}", db_key);
                continue;
            }
        };
        let modified_at = match get_date_field_by_name(&value, "modifiedAt") {
            Ok(val) => val,
            Err(_) => {
                error!(target: "app", "find_all - cannot parse modifiedAt for key: {}", db_key);
                continue;
            }
        };

        elements.push(Online {
            api_token: api_token.to_string(),
            device_uuid: device_uuid.to_string(),
            feature_uuid: feature_uuid.to_string(),
            fcm_token: fcm_token.to_string(),
            // Notification preferences are stored in Redis DB 3.
            // Set the default to false here; apply_notification_preferences()
            // loads and applies the actual value after the online records are read.
            notification_silenced: false,
            created_at,
            modified_at,
        });
    }
    Ok(elements)
}

// filter offline devices features, ignoring a device if it has notification_silenced = true
pub fn filter_offline(all: &[Online], offline_timeout_seconds: u64) -> Vec<Online> {
    let curr_date: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before UNIX epoch")
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    all.iter()
        .filter(|el| {
            el.modified_at < curr_date.saturating_sub(offline_timeout_seconds.saturating_mul(1000))
                && !el.notification_silenced
        })
        .cloned()
        .collect()
}

// filter online devices features (all that are not in offline list)
pub fn filter_online(all: &[Online], offline: &[Online]) -> Vec<Online> {
    let offline_keys: HashSet<(&str, &str)> =
        offline.iter().map(|el| (el.device_uuid.as_str(), el.feature_uuid.as_str())).collect();
    all.iter()
        .filter(|el| !offline_keys.contains(&(el.device_uuid.as_str(), el.feature_uuid.as_str())))
        .cloned()
        .collect()
}

pub fn get_all_keys_pattern(is_testing: bool) -> &'static str {
    if is_testing { "test_*" } else { "online_*" }
}

pub fn get_date_field_by_name(value: &HashMap<String, String>, field_name: &str) -> Result<u64, DbError> {
    match value.get(field_name) {
        Some(val) => val.parse::<u64>().map_err(DbError::DbStrToNumError),
        None => Err(DbError::DbMissingFieldError(field_name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{filter_offline, filter_online, get_all_keys_pattern, get_date_field_by_name};
    use crate::errors::db_error::DbError;
    use crate::models::online::Online;
    use pretty_assertions::assert_eq;

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX epoch")
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn online(device_uuid: &str, feature_uuid: &str, modified_at: u64) -> Online {
        Online {
            api_token: format!("api-token-{device_uuid}"),
            device_uuid: device_uuid.to_string(),
            feature_uuid: feature_uuid.to_string(),
            fcm_token: format!("fcm-token-{feature_uuid}"),
            notification_silenced: false,
            created_at: 1,
            modified_at,
        }
    }

    #[test_log::test]
    fn get_all_keys_pattern_uses_test_prefix_only_in_testing() {
        assert_eq!("test_*", get_all_keys_pattern(true));
        assert_eq!("online_*", get_all_keys_pattern(false));
    }

    #[test_log::test]
    fn get_date_field_by_name_parses_existing_numeric_field() {
        let value = HashMap::from([("createdAt".to_string(), "1710000000123".to_string())]);

        assert_eq!(1710000000123, get_date_field_by_name(&value, "createdAt").unwrap());
    }

    #[test_log::test]
    fn get_date_field_by_name_reports_missing_field_name() {
        let value = HashMap::new();

        let err = get_date_field_by_name(&value, "modifiedAt").unwrap_err();

        match err {
            DbError::DbMissingFieldError(field_name) => assert_eq!("modifiedAt", field_name),
            other => panic!("expected missing field error, got {other:?}"),
        }
    }

    #[test_log::test]
    fn get_date_field_by_name_reports_parse_error() {
        let value = HashMap::from([("modifiedAt".to_string(), "not-a-number".to_string())]);

        let err = get_date_field_by_name(&value, "modifiedAt").unwrap_err();

        match err {
            DbError::DbStrToNumError(_) => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test_log::test]
    fn filter_offline_returns_devices_older_than_timeout() {
        let now = now_millis();
        let devices =
            vec![online("device-a", "feature-a", now.saturating_sub(120_000)), online("device-b", "feature-b", now)];

        let offline = filter_offline(&devices, 60);

        assert_eq!(vec!["device-a"], offline.iter().map(|device| device.device_uuid.as_str()).collect::<Vec<_>>());
    }

    #[test_log::test]
    fn filter_offline_keeps_recent_devices_online() {
        let now = now_millis();
        let devices = vec![online("old", "feature-a", now.saturating_sub(120_000)), online("recent", "feature-b", now)];

        let offline = filter_offline(&devices, 60);

        assert_eq!(vec!["old"], offline.iter().map(|device| device.device_uuid.as_str()).collect::<Vec<_>>());
    }

    #[test_log::test]
    fn filter_online_removes_only_matching_device_feature_pairs() {
        let all = vec![
            online("device-a", "feature-a", 1),
            online("device-a", "feature-b", 1),
            online("device-b", "feature-a", 1),
        ];
        let offline = vec![online("device-a", "feature-a", 1)];

        let result = filter_online(&all, &offline);

        assert_eq!(
            vec![("device-a", "feature-b"), ("device-b", "feature-a")],
            result.iter().map(|device| (device.device_uuid.as_str(), device.feature_uuid.as_str())).collect::<Vec<_>>()
        );
    }
}
