use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use redis::{AsyncCommands, AsyncIter, RedisResult, aio::ConnectionManager};
use tracing::error;

use crate::errors::db_error::DbError;
use crate::errors::redis_error::RedisError;
use crate::models::online::Online;

pub async fn find_all(db: &ConnectionManager) -> Result<Vec<Online>, anyhow::Error> {
    let mut con = db.clone();
    let mut elements: Vec<Online> = vec![];

    // get all keys with format 'online_<deviceUuid>_feature_<featureUuid>'
    let db_keys_iter_res: RedisResult<AsyncIter<String>> =
        con.scan_match::<&str, String>(get_all_keys_pattern().as_str()).await;
    if db_keys_iter_res.is_err() {
        return Err(anyhow::Error::from(RedisError::GetKeysError));
    }
    let db_keys: Vec<String> = db_keys_iter_res?.map(Result::unwrap).collect::<Vec<String>>().await;
    for db_key in db_keys {
        // hgetall returns the entire redis hash table (with all "key: value")
        let value_res: RedisResult<HashMap<String, String>> = con.hgetall(&db_key).await;
        if value_res.is_err() {
            return Err(anyhow::Error::from(RedisError::HGetAllError));
        }
        let value: HashMap<String, String> = value_res?;

        let items: Vec<&str> = db_key.split('_').collect();
        let device_uuid = items.get(1).unwrap().to_string();
        let feature_uuid = items.last().unwrap().to_string();

        let api_token: Result<&str, DbError> = match &value.get("apiToken") {
            Some(val) => Ok(val),
            None => Err(DbError::DbNotFound),
        };
        let fcm_token: Result<&str, DbError> = match &value.get("fcmToken") {
            Some(val) => Ok(val),
            None => Ok(""),
        };

        let created_at: Result<u128, DbError> = get_date_field_by_name(&value, "createdAt");
        let modified_at: Result<u128, DbError> = get_date_field_by_name(&value, "modifiedAt");

        if api_token.is_err() {
            error!(target: "app", "find_all - apiToken is missing");
            continue;
        }
        if created_at.is_err() || modified_at.is_err() {
            error!(target: "app", "find_all - cannot parse dates");
            continue;
        }
        let online: Online = Online {
            apiToken: api_token?.to_string(),
            deviceUuid: device_uuid.to_string(),
            featureUuid: feature_uuid.to_string(),
            fcmToken: fcm_token?.to_string(),
            createdAt: created_at?.to_string(),
            modifiedAt: modified_at?.to_string(),
        };
        elements.push(online);
    }
    Ok(elements)
}

pub fn filter_offline(all: Vec<Online>, offline_timeout_seconds: u128) -> Vec<Online> {
    let curr_date: u128 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    all.into_iter()
        .filter(|el: &Online| {
            let mod_date: u128 = el.modifiedAt.parse().unwrap();
            mod_date < (curr_date - (offline_timeout_seconds * 1000))
        })
        .collect()
}

pub fn filter_online(all: Vec<Online>, offline: Vec<Online>) -> Vec<Online> {
    let offline_uuids: Vec<String> = offline
        .into_iter()
        .map(|el| format!("{}-{}", el.deviceUuid, el.featureUuid))
        .collect();
    all.into_iter()
        .filter(|el: &Online| !offline_uuids.contains(&format!("{}-{}", el.deviceUuid, el.featureUuid)))
        .collect()
}

pub fn get_all_keys_pattern() -> String {
    let env = env::var("ENV").ok().unwrap_or("".to_string());
    (if env == "testing" { "test_*" } else { "online_*" }).to_owned()
}

pub fn get_date_field_by_name(value: &HashMap<String, String>, field_name: &str) -> Result<u128, DbError> {
    if field_name != "createdAt" && field_name != "modifiedAt" {
        return Err(DbError::UnknownFieldNameError);
    }
    let date: Result<u128, DbError> = match value.get(field_name) {
        Some(val) => match val.parse::<u128>() {
            Ok(val) => Ok(val),
            Err(_) => Err(DbError::DbStrToNumError),
        },
        None => Ok(0u128),
    };
    date
}
