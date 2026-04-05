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
            created_at,
            modified_at,
        });
    }
    Ok(elements)
}

pub fn filter_offline(all: &[Online], offline_timeout_seconds: u64) -> Vec<Online> {
    let curr_date: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before UNIX epoch")
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    all.iter()
        .filter(|el| el.modified_at < curr_date.saturating_sub(offline_timeout_seconds.saturating_mul(1000)))
        .cloned()
        .collect()
}

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
