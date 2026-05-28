use std::env;
use std::fmt;

use dotenvy::dotenv;
use serde::Deserialize;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// Which runtime environment the application is running in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Testing,
    Production,
}

impl AppEnv {
    /// Reads the `ENV` environment variable. Returns `Testing` only when the
    /// value is exactly `"testing"`; any other value (including absent) is
    /// treated as `Production`.
    pub fn from_env() -> Self {
        match env::var("ENV").as_deref() {
            Ok("testing") => Self::Testing,
            _ => Self::Production,
        }
    }

    pub fn is_testing(&self) -> bool {
        matches!(self, Self::Testing)
    }
}

#[derive(Deserialize)]
pub struct Env {
    pub log_level: Option<String>,
    pub redis_uri: String,
    pub redis_username: String,
    pub redis_password: String,
    pub cache_timeout_seconds: u64,
    pub offline_timeout_seconds: u64,
    #[serde(default = "default_fcm_key_path")]
    pub fcm_service_account_key_path: String,
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Env")
            .field("log_level", &self.log_level)
            .field("redis_uri", &redact_redis_uri(&self.redis_uri))
            .field("redis_username", &self.redis_username)
            .field("redis_password", &"***")
            .field("cache_timeout_seconds", &self.cache_timeout_seconds)
            .field("offline_timeout_seconds", &self.offline_timeout_seconds)
            .field("fcm_service_account_key_path", &self.fcm_service_account_key_path)
            .finish()
    }
}

fn default_fcm_key_path() -> String {
    "./serviceAccountKey.json".to_string()
}

/// Returns the Redis URI with any embedded password replaced by `***`.
/// e.g. `redis://:secret@host:6379` → `redis://:***@host:6379`
pub fn redact_redis_uri(uri: &str) -> String {
    if let Some(at_pos) = uri.rfind('@')
        && let Some(scheme_end) = uri.find("://")
    {
        let scheme_and_authority = &uri[..scheme_end + 3];
        let host_and_rest = &uri[at_pos..];
        return format!("{scheme_and_authority}***{host_and_rest}");
    }
    uri.to_string()
}

pub fn init() -> (Env, AppEnv) {
    // Load the .env file
    dotenv().ok();
    let env = envy::from_env::<Env>().expect("failed to parse environment variables");
    let app_env = AppEnv::from_env();

    // Configure logging if not in test env.
    // We use set_global_default (not .init()) intentionally: .init() would also install
    // a LogTracer bridge for the `log` crate, which prevents Rocket from installing its
    // own RocketLogger. Without RocketLogger, Rocket's startup output (routes, config,
    // launched URL) is silently dropped. By skipping LogTracer, Rocket gets to install
    // its own logger and prints its startup info directly to stdout.

    if !app_env.is_testing() {
        let parsed_level = env.log_level.as_deref().and_then(|s| s.parse::<tracing::Level>().ok());
        if env.log_level.is_some() && parsed_level.is_none() {
            eprintln!(
                "WARNING: LOG_LEVEL '{}' is invalid, falling back to DEBUG",
                env.log_level.as_deref().unwrap_or("")
            );
        }
        let stdout_max_level = parsed_level.unwrap_or(tracing::Level::DEBUG);
        let stdout = std::io::stdout.with_filter(|meta| meta.target() == "app").with_max_level(stdout_max_level);
        let debug_file = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("info")
            .filename_suffix("log")
            .max_log_files(5)
            .build("./logs")
            .expect("initializing rolling info_file appender failed")
            .with_max_level(tracing::Level::INFO);
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
        let subscriber = tracing_subscriber::fmt()
            .compact()
            .with_writer(writer)
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("Unable to install global subscriber");
    }

    info!(target: "app", "Starting application...");

    // Print .env vars
    print_env(&env);
    (env, app_env)
}

fn print_env(env: &Env) {
    info!(target: "app", "log_level = {}", env.log_level.as_deref().unwrap_or("debug"));
    info!(target: "app", "redis_uri = {}", redact_redis_uri(&env.redis_uri));
    info!(target: "app", "redis_username = {}", env.redis_username);
    info!(target: "app", "redis_password = {}", !env.redis_password.is_empty());
    info!(target: "app", "cache_timeout_seconds = {}", env.cache_timeout_seconds);
    info!(target: "app", "offline_timeout_seconds = {}", env.offline_timeout_seconds);
    info!(target: "app", "fcm_service_account_key_path = {}", env.fcm_service_account_key_path);
}

#[cfg(test)]
mod tests {
    use super::{Env, redact_redis_uri};
    use pretty_assertions::assert_eq;

    #[test_log::test]
    fn redact_redis_uri_replaces_embedded_credentials() {
        assert_eq!("redis://***@localhost:6379/0", redact_redis_uri("redis://user:secret@localhost:6379/0"));
    }

    #[test_log::test]
    fn redact_redis_uri_replaces_password_only_credentials() {
        assert_eq!("redis://***@localhost:6379", redact_redis_uri("redis://:secret@localhost:6379"));
    }

    #[test_log::test]
    fn redact_redis_uri_leaves_uri_without_credentials_unchanged() {
        assert_eq!("redis://localhost:6379", redact_redis_uri("redis://localhost:6379"));
    }

    #[test_log::test]
    fn env_debug_redacts_secrets_but_keeps_operational_fields() {
        let env = Env {
            log_level: Some("INFO".to_string()),
            redis_uri: "redis://user:secret@localhost:6379/0".to_string(),
            redis_username: "redis-user".to_string(),
            redis_password: "redis-password".to_string(),
            cache_timeout_seconds: 30,
            offline_timeout_seconds: 60,
            fcm_service_account_key_path: "./key.json".to_string(),
        };

        let output = format!("{env:?}");

        assert!(output.contains("log_level: Some(\"INFO\")"));
        assert!(output.contains("redis_uri: \"redis://***@localhost:6379/0\""));
        assert!(output.contains("redis_username: \"redis-user\""));
        assert!(output.contains("redis_password: \"***\""));
        assert!(output.contains("cache_timeout_seconds: 30"));
        assert!(output.contains("offline_timeout_seconds: 60"));
        assert!(!output.contains("redis-password"));
        assert!(!output.contains("user:secret"));
    }
}
