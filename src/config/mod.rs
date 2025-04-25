use log::info;

use dotenvy::dotenv;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Env {
    pub redis_uri: String,
    pub cache_timeout_seconds: String,
    pub offline_timeout_seconds: String,
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
    let redis_uri = env.redis_uri.clone();
    let cache_timeout_seconds = env.cache_timeout_seconds.clone();
    let offline_timeout_seconds = env.offline_timeout_seconds.clone();
    info!(target: "app", "env = {:?}", env);
    info!(target: "app", "redis_uri = {}", redis_uri);
    info!(target: "app", "cache_timeout_seconds = {}", cache_timeout_seconds);
    info!(target: "app", "offline_timeout_seconds = {}", offline_timeout_seconds);
}
