use log::info;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use redis::{aio::MultiplexedConnection, AsyncCommands};

use crate::models::online::Online;

pub async fn find_all_online(con: &MultiplexedConnection) -> Vec<Online> {
    info!(target: "app", "find_all_online - To get all online elements from db");
    let mut con = con.clone();

    let mut not_online_devices: Vec<Online> = vec![];

    // get all keys with format 'online-<uuid>'
    let values = con.scan_match::<&str, String>("online-*").await.unwrap();
    let keys: Vec<String> = values.collect().await;

    for key in keys {
        // hgetall returns the entire redis hash table (with all "key: value")
        let value: HashMap<String, u64> = con.hgetall(&key).await.unwrap();
        let online: u64 = match value.get("online") {
            Some(val) => *val,
            None => 0u64,
        };
        let created_at: u64 = match value.get("createdAt") {
            Some(val) => *val,
            None => 0u64,
        };
        let modified_at: u64 = match value.get("modifiedAt") {
            Some(val) => *val,
            None => 0u64,
        };

        let curr_date = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        if modified_at < (curr_date - 60) {
            let online: Online = Online {
                uuid: key,
                createdAt: created_at,
                modifiedAt: modified_at,
                online: online == 1,
            };
            not_online_devices.push(online);
        }
    }
    not_online_devices
}
