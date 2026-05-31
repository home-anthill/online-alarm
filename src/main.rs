use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use google_fcm1::{
    FirebaseCloudMessaging,
    api::{Message, Notification, SendMessageRequest},
    hyper_rustls, hyper_util, yup_oauth2,
};
use redis::aio::ConnectionManager;
use rocket::{self, catchers, routes};
use tracing::{debug, error, info, warn};

use online::catchers as app_catchers;
use online::config::{AppEnv, Env, init};
use online::db::notification::{
    SentNotification, api_tokens_for_devices, next_notification_id, save_sent_notification,
};
use online::db::online::{filter_offline, filter_online, find_all};
use online::notifications::{collect_due_offline_notifications, offline_notification_body};
use online::routes as app_routes;

#[rocket::main]
#[allow(clippy::result_large_err)]
async fn main() -> Result<(), rocket::Error> {
    // 1. Init logger and env
    let (env, app_env): (Env, AppEnv) = init();
    let cache_timeout_seconds = env.cache_timeout_seconds;
    let offline_timeout_seconds = env.offline_timeout_seconds;
    let is_testing = app_env.is_testing();

    // 2. Init and connect to Redis
    // If credentials are configured, inject them into the URI:
    //   redis://host:port -> redis://username:password@host:port
    if !env.redis_username.is_empty() && env.redis_password.is_empty() {
        warn!(target: "app", "REDIS_USERNAME is set but REDIS_PASSWORD is empty — no authentication will be attempted");
    }
    let redis_url = redis_url_with_credentials(&env.redis_uri, &env.redis_username, &env.redis_password);
    let redis_client = redis::Client::open(redis_url).expect("invalid Redis URI");
    let con: ConnectionManager = redis_client.get_connection_manager().await.expect("failed to connect to Redis");
    let notifications_redis_uri = env.notifications_redis_uri.as_deref().unwrap_or(&env.redis_uri);
    let notifications_redis_url =
        redis_url_with_credentials(notifications_redis_uri, &env.redis_username, &env.redis_password);
    let notifications_redis_client =
        redis::Client::open(notifications_redis_url).expect("invalid notifications Redis URI");
    let notifications_con: ConnectionManager =
        notifications_redis_client.get_connection_manager().await.expect("failed to connect to notifications Redis");

    // 3. Init Firebase client
    // To download the service account file, follow this procedure:
    // a. Go to https://console.firebase.google.com/
    // b. Open your project
    // c. Click on the settings button and choose the Project settings option
    // d. Click on the Service Account tab. You should be in a page like this:
    //    https://console.firebase.google.com/project/<YOUR_PROJECT_ID>/settings/serviceaccounts/adminsdk
    // e. Click on the "Generate new private key" button to download the service account .json file
    // f. Place `serviceAccountKey.json` at the root of this project
    let service_account_key = yup_oauth2::read_service_account_key(&env.fcm_service_account_key_path)
        .await
        .expect("failed to read FCM service account key file");
    let project_id = service_account_key.project_id.clone().expect("project_id missing in FCM service account key");
    let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .expect("failed to load native TLS roots")
        .https_or_http()
        .enable_http2()
        .build();
    // Auth client (body type inferred by yup-oauth2 internally)
    let auth_hyper_client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(https_connector.clone());
    let auth = yup_oauth2::ServiceAccountAuthenticator::with_client(
        service_account_key,
        yup_oauth2::CustomHyperClientBuilder::from(auth_hyper_client),
    )
    .build()
    .await
    .expect("failed to build FCM authenticator");
    // Hub client (body type BoxBody<Bytes, Error> inferred by FirebaseCloudMessaging::new)
    let fcm_hyper_client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new()).build(https_connector);
    let fcm_hub = FirebaseCloudMessaging::new(fcm_hyper_client, auth);

    // 4. Init cache
    // It's used to store UUIDs as keys and insertion date as value to prevent too many notifications
    let cache: DashMap<String, u64> = DashMap::new();

    // 5. send notifications for all offline devices
    let notification_handle = tokio::task::spawn(async move {
        loop {
            // read all elements
            // TODO this is bad, because I have to improve logic to clean old devices from redis and so on
            let all_devices = match find_all(&con, is_testing).await {
                Ok(devices) => devices,
                Err(err) => {
                    error!(target: "app", "cannot find all elements in db, err = {:?}", err);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };
            let offline_devices = filter_offline(&all_devices, offline_timeout_seconds);
            let online_devices = filter_online(&all_devices, &offline_devices);

            // clean from cache all devices that become online
            for online in online_devices {
                let key = online.cache_key();
                if cache.remove(&key).is_some() {
                    info!(target: "app", "cleaned online device key={} from cache", &key);
                }
            }

            // process all offline devices
            let curr_date: u64 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is before UNIX epoch")
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX);

            for offline in &offline_devices {
                let key = offline.cache_key();
                debug!(target: "app", "offline device key={} (created_at={}, modified_at={})", &key, &offline.created_at, &offline.modified_at);
            }

            let notification_batches =
                collect_due_offline_notifications(&cache, offline_devices, curr_date, cache_timeout_seconds);

            for batch in notification_batches {
                let device_count = batch.devices.len();
                let title = "home anthill".to_string();
                let body = offline_notification_body(device_count);
                warn!(
                    target: "app",
                    "sending grouped message to FCM for {} offline device(s)",
                    device_count
                );
                let req = SendMessageRequest {
                    message: Some(Message {
                        token: Some(batch.fcm_token.clone()),
                        notification: Some(Notification {
                            title: Some(title.clone()),
                            body: Some(body.clone()),
                            image: None,
                        }),
                        ..Default::default()
                    }),
                    validate_only: None,
                };
                let parent = format!("projects/{}", project_id);
                match fcm_hub.projects().messages_send(req, &parent).doit().await {
                    Ok((_resp, msg)) => {
                        debug!(target: "app", "FCM response = {:?}", &msg);
                        let notification_id = next_notification_id(curr_date);
                        let api_tokens = api_tokens_for_devices(&batch.devices);

                        let sent_notification = SentNotification {
                            id: &notification_id,
                            api_tokens: &api_tokens,
                            sent_at: curr_date,
                            title: &title,
                            body: &body,
                            devices: &batch.devices,
                            provider_message_id: msg.name.as_deref(),
                        };

                        if let Err(err) = save_sent_notification(&notifications_con, &sent_notification).await {
                            error!(target: "app", "cannot save sent notification to Redis, err = {:?}", err);
                        }
                    }
                    Err(err) => {
                        error!(target: "app", "cannot send message to FCM, err = {:?}", err);
                    }
                }
                for offline in batch.devices {
                    let key = offline.cache_key();
                    // renew cache with a new date (insert overwrites existing entry)
                    warn!(target: "app", "re-adding offline device key={} to cache", &key);
                    cache.insert(key, curr_date);
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // 6. Launch Rocket for the health endpoint
    info!(target: "app", "Starting Rocket...");
    let _rocket = rocket::build()
        .mount("/", routes![app_routes::api::keep_alive])
        .register(
            "/",
            catchers![
                app_catchers::bad_request,
                app_catchers::not_found,
                app_catchers::internal_server_error,
                app_catchers::service_unavailable,
            ],
        )
        .launch()
        .await?;

    notification_handle.abort();
    Ok(())
}

fn redis_url_with_credentials(redis_uri: &str, redis_username: &str, redis_password: &str) -> String {
    if redis_password.is_empty() {
        return redis_uri.to_string();
    }

    match redis_uri.find("://") {
        Some(scheme_end) => format!(
            "{scheme}{username}:{password}@{rest}",
            scheme = &redis_uri[..scheme_end + 3],
            username = urlencoding::encode(redis_username),
            password = urlencoding::encode(redis_password),
            rest = &redis_uri[scheme_end + 3..],
        ),
        None => {
            warn!(target: "app", "Redis URI has no recognizable scheme (missing '://'), skipping credential injection");
            redis_uri.to_string()
        }
    }
}

// testing
#[cfg(test)]
mod tests_integration;
