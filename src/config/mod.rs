use dotenvy::dotenv;
use log::info;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Env {
    pub mongo_uri: String,
    pub mongo_db_name: String,
}

pub fn init() -> Env {
    // Init logger if not in testing environment
    let _ = log4rs::init_file("log4rs.yaml", Default::default());
    info!(target: "app", "Starting application...");
    // Load the .env file
    dotenv().ok();
    let env = envy::from_env::<Env>().ok().unwrap();
    // Print .env vars
    print_env(&env);
    env
}

fn print_env(env: &Env) {
    let mongo_uri = env.mongo_uri.clone();
    let mongo_db_name = env.mongo_db_name.clone();
    info!(target: "app", "env = {:?}", env);
    info!(target: "app", "mongo_uri = {}", mongo_uri);
    info!(target: "app", "mongo_db_name = {}", mongo_db_name);
}
