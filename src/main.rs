use log::{debug, error, info, warn};
use mongodb::Database;

use online::config::{init, Env};
use online::db::connect;
use online::db::online::find_all_online;

#[tokio::main]
async fn main() {
    // 1. Init logger and env
    let env: Env = init();

    // 2. Init and connect to the Database
    let database: Database = connect(&env).await.unwrap_or_else(|error| {
        error!(target: "app", "MongoDB - cannot connect {:?}", error);
        panic!("cannot connect to MongoDB:: {:?}", error)
    });

    // 3. find offline devices
    tokio::task::spawn(async move {
        loop {
            let result_db = find_all_online(&database).await;
            match &result_db {
                Ok(results) => {
                    for result in results.iter() {
                        info!(target: "app", "iterating result = {:?}", &result);
                    }
                },
                Err(err) => error!(target: "app", "err = {:?}", err),
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
