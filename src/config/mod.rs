use dotenvy::dotenv;
use serde::Deserialize;
use tracing::{Metadata, info};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::writer::MakeWriterExt;

#[derive(Deserialize, Debug)]
pub struct Env {
    pub redis_uri: String,
    pub cache_timeout_seconds: String,
    pub offline_timeout_seconds: String,
    pub hide_rocket_log_stdout: bool,
}

fn filter_target(meta: &Metadata) -> bool {
    meta.target() == "app"
}

fn skip_filter_target(_meta: &Metadata) -> bool {
    true
}

pub fn init() -> Env {
    // Load the .env file
    dotenv().ok();
    let env = envy::from_env::<Env>().ok().unwrap();

    // Configure logging
    let filter = if env.hide_rocket_log_stdout {
        filter_target
    } else {
        skip_filter_target
    };
    let stdout = std::io::stdout.with_filter(filter);
    let debug_file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("all")
        .filename_suffix("log")
        .max_log_files(5)
        .build("./logs")
        .expect("initializing rolling debug_file appender failed")
        .with_filter(|meta| meta.target() == "app");
    let error_file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("error")
        .filename_suffix("log")
        .max_log_files(5)
        .build("./logs")
        .expect("initializing rolling error_file appender failed")
        .with_filter(|meta| meta.target() == "app")
        .with_max_level(tracing::Level::ERROR);
    let writer = debug_file.and(error_file).and(stdout);
    tracing_subscriber::fmt()
        .compact()
        .with_writer(writer)
        .with_ansi(false)
        .init();

    info!(target: "app", "Starting application...");

    // Print .env vars
    print_env(&env);
    env
}

fn print_env(env: &Env) {
    let redis_uri = env.redis_uri.clone();
    let cache_timeout_seconds = env.cache_timeout_seconds.clone();
    let offline_timeout_seconds = env.offline_timeout_seconds.clone();
    let hide_rocket_log_stdout = env.hide_rocket_log_stdout;
    info!(target: "app", "env = {:?}", env);
    info!(target: "app", "redis_uri = {}", redis_uri);
    info!(target: "app", "cache_timeout_seconds = {}", cache_timeout_seconds);
    info!(target: "app", "offline_timeout_seconds = {}", offline_timeout_seconds);
    info!(target: "app", "hide_rocket_log_stdout = {}", hide_rocket_log_stdout);
}
