#[macro_use]
extern crate rocket;

use log::{debug, error, info};
use std::sync::Arc;
use std::time::Duration;

use fcm::message::{Message, Notification, Target};
use fcm::response::FcmResponse;
use redis::aio::ConnectionManager;
use retainer::*;
use serde_json::json;

use online::catchers;
use online::config::{init, Env};
use online::db::online::find_all_offline;
use online::routes;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    // 1. Init logger and env
    let env: Env = init();

    // 2. Init and connect to Redis
    let client = redis::Client::open(env.redis_uri.clone()).unwrap();
    let con: ConnectionManager = client.get_connection_manager().await.unwrap();

    // 3. Init Firebase client
    // To download the service account file follow this procedure:
    // a. Go to https://console.firebase.google.com/
    // b. Open your project
    // c. Click on the settings button and choose the Project settings option
    // d. Click on the Service Account tab. You should be in a page like this:
    //    https://console.firebase.google.com/project/<YOUR_PROJECT_ID>/settings/serviceaccounts/adminsdk
    // e. Click on the "Generate new private key" button to download the service account .json file
    // f. Place `serviceAccountKey.json` at the root of this project
    let client = fcm::FcmClient::builder()
        .service_account_key_json_path("./serviceAccountKey.json")
        .build()
        .await
        .unwrap();

    // 4. Init notification cache
    // Cache to store UUIDs as keys that have been already notified to prevent too many notifications
    let cache = Arc::new(Cache::new());
    let cache_clone = cache.clone();
    // monitor the cache to evict entries (using the same timing taken from Redis source code)
    tokio::spawn(async move { cache_clone.monitor(4, 0.25, Duration::from_secs(3)).await });

    // 5. find offline devices
    tokio::task::spawn(async move {
        // TODO improve logic to group notifications by `fcmToken` to send only one
        //      message for all devices in a single time.
        loop {
            // process offline devices
            let offline_devices_res = find_all_offline(&con).await;
            match &offline_devices_res {
                Ok(_) => (),
                Err(err) => {
                    error!(target: "app", "cannot find all offline in db, err = {:?}", err);
                    continue;
                }
            }
            for offline in offline_devices_res.unwrap().into_iter() {
                debug!(target: "app", "iterating offline = {:?}", &offline);

                // if not in cache, add it and send the notification, otherwise skip this device
                let uuid = offline.uuid.clone();
                if cache.get(&uuid).await.is_none() {
                    // add uuid in cache (no need to use the value, so it's fixed to 0) with
                    // a defined timeout
                    cache.insert(uuid, 0, Duration::from_secs(3 * 60)).await;
                } else {
                    continue;
                }

                // build the notification and send it
                let message = Message {
                    data: Some(json!({
                       "message": "Hello msg!",
                    })),
                    notification: Some(Notification {
                        title: Some("Hello".to_string()),
                        body: Some("message body".to_string()),
                        image: None,
                    }),
                    target: Target::Token(offline.fcmToken.clone()),
                    android: None,
                    webpush: None,
                    apns: None,
                    fcm_options: None,
                };
                let response: FcmResponse = client.send(message).await.unwrap();
                debug!(target: "app", "response = {:?}", &response);
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
