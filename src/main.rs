#[macro_use]
extern crate rocket;

use log::info;
use redis::aio::MultiplexedConnection;

use online::catchers;
use online::config::{init, Env};
use online::db::online::find_all_online;
use online::routes;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    // 1. Init logger and env
    let env: Env = init();

    // 2. Init and connect to Redis
    let client = redis::Client::open(env.redis_uri.clone()).unwrap();
    let con: MultiplexedConnection = client.get_multiplexed_async_connection().await.unwrap();

    // 3. find offline devices
    tokio::task::spawn(async move {
        loop {
            let results = find_all_online(&con).await;
            for result in results {
                info!(target: "app", "iterating result = {:?}", &result);
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
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
