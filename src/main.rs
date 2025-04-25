#[macro_use]
extern crate rocket;

use log::{debug, error, info, warn};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use fcm_rs::{
    client::FcmClient,
    models::{Message, Notification},
};
use redis::aio::ConnectionManager;

use online::catchers;
use online::config::{Env, init};
use online::db::online::{filter_offline, filter_online, find_all};
use online::routes;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    // 1. Init logger and env
    let env: Env = init();
    let cache_timeout_seconds: u128 = env.cache_timeout_seconds.clone().parse().unwrap();
    let offline_timeout_seconds: u128 = env.offline_timeout_seconds.clone().parse().unwrap();

    // 2. Init and connect to Redis
    let client = redis::Client::open(env.redis_uri.clone()).unwrap();
    let con: ConnectionManager = client.get_connection_manager().await.unwrap();

    // 3. Init Firebase client
    // To download the service account file, follow this procedure:
    // a. Go to https://console.firebase.google.com/
    // b. Open your project
    // c. Click on the settings button and choose the Project settings option
    // d. Click on the Service Account tab. You should be in a page like this:
    //    https://console.firebase.google.com/project/<YOUR_PROJECT_ID>/settings/serviceaccounts/adminsdk
    // e. Click on the "Generate new private key" button to download the service account .json file
    // f. Place `serviceAccountKey.json` at the root of this project
    let client = FcmClient::new("./serviceAccountKey.json").await.unwrap();

    // 4. Init cache
    // It's used to store UUIDs as keys and insertion date as value to prevent too many notifications
    let cache = DashMap::new();

    // 5. send notifications for all offline devices
    tokio::task::spawn(async move {
        // TODO improve logic to group notifications by `fcmToken` to send only one
        //      message for all devices in a single time.
        loop {
            // read all elements
            // TODO this is bad, because I have to improve logic to clean old devices from redis and so on
            let all_res = find_all(&con).await;
            match &all_res {
                Ok(_) => (),
                Err(err) => {
                    error!(target: "app", "cannot find all elements in db, err = {:?}", err);
                    continue;
                }
            }
            let all_devices = all_res.unwrap();
            let offline_devices = filter_offline(all_devices.clone(), offline_timeout_seconds);
            let online_devices = filter_online(all_devices, offline_devices.clone());

            // clean from cache all devices that become online
            for online in online_devices.into_iter() {
                if cache.get(&online.uuid).is_some() {
                    cache.remove(&online.uuid);
                    info!(target: "app", "cleaned online device uuid={} from cache", &online.uuid);
                }
            }

            // process all offline devices
            for offline in offline_devices.into_iter() {
                debug!(target: "app", "offline device uuid={} (createdAt={}, modifiedAt={})", &offline.uuid, &offline.createdAt, &offline.modifiedAt);
                let curr_date: u128 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                let uuid = offline.uuid.clone();

                if cache.get(&uuid).is_none() {
                    warn!(target: "app", "adding offline device uuid={} to cache", &uuid);
                    cache.insert(uuid, curr_date);
                } else {
                    debug!(target: "app", "offline device uuid={} is already in cache", &uuid);
                    let el = cache.get(&uuid).unwrap().value().to_owned();
                    if el < (curr_date - (cache_timeout_seconds * 1000)) {
                        warn!(target: "app", "sending message to FCM for uuid={}", &uuid);
                        // build the notification and send it
                        let message = Message {
                            token: Some(offline.fcmToken.clone()),
                            notification: Some(Notification {
                                title: Some("home anthill".to_string()),
                                body: Some("Device is offline".to_string()),
                            }),
                            data: None,
                        };

                        match client.send(message).await {
                            Ok(response) => {
                                debug!(target: "app", "FCM response = {:?}", &response);
                            }
                            Err(err) => {
                                error!(target: "app", "cannot send message to FCM, err = {:?}", err);
                            }
                        }
                        // renew cache re-adding the element with a new date
                        cache.remove(&uuid);
                        warn!(target: "app", "re-adding offline device uuid={} to cache", &uuid);
                        cache.insert(uuid, curr_date);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // 4. Init Rocket
    // a) define APIs
    // b) define error handlers
    info!(target: "app", "Starting Rocket...");
    let _rocket = rocket::build()
        .mount("/", routes![routes::api::keep_alive])
        .register(
            "/",
            catchers![
                catchers::bad_request,
                catchers::not_found,
                catchers::internal_server_error,
            ],
        )
        .launch()
        .await?;

    Ok(())
}
