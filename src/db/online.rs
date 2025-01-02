use log::{debug, error, info};
use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use redis::{aio::ConnectionManager, AsyncCommands, AsyncIter, RedisResult};

use crate::errors::db_error::DbError;
use crate::errors::redis_error::RedisError;
use crate::models::online::Online;

pub async fn find_all_offline(db: &ConnectionManager) -> Result<Vec<Online>, anyhow::Error> {
    debug!(target: "app", "find_all_offline - to get all online elements from db");
    let mut con = db.clone();

    let mut not_online_devices: Vec<Online> = vec![];

    // get all keys with format 'online-<uuid>'
    let db_keys_iter_res: RedisResult<AsyncIter<String>> =
        con.scan_match::<&str, String>(get_all_keys_pattern().as_str()).await;
    if db_keys_iter_res.is_err() {
        return Err(anyhow::Error::from(RedisError::GetKeysError));
    }
    let db_keys: Vec<String> = db_keys_iter_res?.collect().await;

    for db_key in db_keys {
        // hgetall returns the entire redis hash table (with all "key: value")
        let value_res: RedisResult<HashMap<String, String>> = con.hgetall(&db_key).await;
        if value_res.is_err() {
            return Err(anyhow::Error::from(RedisError::HGetAllError));
        }
        let value: HashMap<String, String> = value_res?;

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
            error!(target: "app", "REST - GET - find_all_offline - apiToken is missing");
            continue;
        }
        if created_at.is_err() || modified_at.is_err() {
            error!(target: "app", "REST - GET - find_all_offline - cannot parse dates");
            continue;
        }

        let curr_date: u128 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let mod_date: u128 = modified_at?;
        if mod_date < (curr_date - (60 * 1000)) {
            let online: Online = Online {
                uuid: db_key,
                apiToken: api_token?.to_string(),
                fcmToken: fcm_token?.to_string(),
                createdAt: created_at?.to_string(),
                modifiedAt: mod_date.to_string(),
            };
            not_online_devices.push(online);
        }
    }
    Ok(not_online_devices)
}

pub fn get_all_keys_pattern() -> String {
    let env = env::var("ENV").ok().unwrap_or("".to_string());
    (if env == "testing" { "test-*" } else { "online-*" }).to_owned()
}

pub fn get_date_field_by_name(value: &HashMap<String, String>, field_name: &str) -> Result<u128, DbError> {
    if field_name != "createdAt" || field_name != "modifiedAt" {
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
