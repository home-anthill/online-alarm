# CHANGELOG_CLAUDE.md

Changes made by GitHub Copilot (Claude Sonnet 4.6) during security and code-quality review sessions.

---

## Security

### Credential redaction in logs and debug output
Redacted Redis URI and password in `print_env()` and the `Debug` impl for `Env` to prevent credential leaks via logs or `{:?}` formatting. Added `redact_redis_uri()` which masks everything between `://` and `@`. Also removed the `env = {:?}` debug dump that exposed the full `Env` struct.

### Remove hardcoded secrets from source and configuration
Removed Base64 secret key from `Rocket.toml` [release] section; the comment now instructs operators to supply `ROCKET_SECRET_KEY` as an environment variable (`openssl rand -base64 32`). Made FCM service-account key path configurable via `FCM_SERVICE_ACCOUNT_KEY_PATH` env var (defaults to `./serviceAccountKey.json`), allowing per-environment overrides without recompiling instead of using hardcoded `FcmClient::new("./serviceAccountKey.json")`.

### Remove secrets baked into Docker runtime image
Removed `COPY .env_template /.env` and `COPY serviceAccountKey.json_template /serviceAccountKey.json` from Dockerfile. Secrets and defaults are no longer baked into image layers; they must be injected at deploy time (Docker secret, volume mount, or orchestrator env).

### URL-encode credentials before injecting into Redis URI
Username and password are now wrapped with `urlencoding::encode()` before splicing into the URI. Special characters (`:`, `@`, `/`) in either field would corrupt the URI or redirect the connection. Fixed credential injection for TLS URIs (`rediss://`) by using `find("://")` to locate the scheme boundary generically instead of `replacen("redis://", ...)`, which found no match in `rediss://` URIs and silently failed to inject credentials for TLS connections.

### Warn on configuration issues instead of silent failures
Added `warn!` log when `REDIS_USERNAME` is set but `REDIS_PASSWORD` is empty, so authentication is not silently skipped. Split `LOG_LEVEL` parsing into a two-step process: if the value is present but invalid, `eprintln!` fires immediately (before tracing subscriber is installed) with explicit fallback to DEBUG, preventing typos from silently running at an unexpected level.

### Handle per-device errors instead of silently dropping them
Skip devices with missing or empty `fcmToken` with a warning log instead of `value.get("fcmToken").map_or("", …)` which would silently attempt FCM sends to an empty token. Log individual Redis SCAN iteration errors instead of discarding them via `filter_map(|r| async { r.ok() })`; missing errors could allow partial scan results to be indistinguishable from complete ones, causing devices to falsely appear online.

### Error handling in HTTP responses
Removed `.unwrap()` in `ApiError::respond_to` which could panic in a Rocket response handler; replaced with `?` to propagate the error, consistent with `ApiResponse::respond_to` above it. Registered the missing 503 catcher (`catchers::service_unavailable`) so 503 responses return the application's JSON format instead of Rocket's default plain-text body.

### HGETALL failure handling
Changed `HGETALL` errors from propagating via `?` (which discards all already-fetched keys and skips remaining ones) to `error!` + `continue` for per-key handling, consistent with other per-device errors in `find_all`. Transient errors on one Redis key no longer abort the entire operation.

---

## Idiomatic Rust

### Error handling and fallibility
Error variants now wrap their source errors via `#[source]` instead of discarding them with `map_err(|_| …)`. Call sites use `map_err(RedisError::GetKeysError)` / `map_err(DbError::DbStrToNumError)` (variant-as-function) to preserve error context.

Added `DbError::DbMissingFieldError(String)` and changed `get_date_field_by_name` from `None => Ok(0u64)` to `Err(DbMissingFieldError(...))` for missing fields. Missing `modifiedAt` was silently treated as epoch (Jan 1 1970), making every such device appear permanently offline and triggering FCM spam.

Changed `Topic::new` to return `Option<Self>` instead of panicking; replaced `.unwrap()` calls with `?` to propagate errors. Changed `Message::new_as_json` to return `Result<String, serde_json::Error>` instead of `.unwrap()`, propagating serialization errors to callers. Updated `message_payload_to_bytes` to handle the `Result`.

### Config abstraction and testability
Removed `std::env` calls from `get_all_keys_pattern` and `find_all` functions. Functions previously called `std::env::var("ENV")` internally, breaking the config abstraction and making them untestable without real env vars. Now accept `is_testing: bool` parameter; `main.rs` computes `is_testing` once from `std::env` at startup.

### Remove dead code
Removed dead MQTT messaging code copied from `online-receiver`: `get_msg_byte`, `message_payload_to_bytes`, local `Message<T>`, `Notification<T>`, `PayloadTrait`, and `OnlineMqttPayload` structs (all unused). Main code imports `fcm_rs::models::{Message, Notification}` directly. Removed unused `IntoIterator` impl on `Online`, unused `filter_target` / `skip_filter_target` helper functions (init() uses inline closures instead), and the `Metadata` import that was only used by them.

Removed unused `Serialize` / `Deserialize` derives from `Online` (after dead messaging code was removed, `Online` is constructed directly in `find_all` and never passed through serde).

Removed dead `DbError` variants (`DbNotFound`, `UnknownFieldNameError`) and the unreachable whitelist check in `get_date_field_by_name` that guarded `field_name != "createdAt" && field_name != "modifiedAt"` but could never be triggered from real call sites. Removed dead `RedisError::HGetAllError` variant.

### Memory efficiency and performance
Changed `filter_offline` / `filter_online` to accept `&[Online]` slices instead of consuming `Vec<Online>` by value, eliminating unnecessary clones at call sites in `main.rs`. Return types remain `Vec<Online>` with `.cloned().collect()` on matching items.

Changed `filter_online` to use zero-allocation `HashSet<(&str, &str)>` of string-slice pairs `(device_uuid, feature_uuid)` instead of heap-allocating via `cache_key()` (which was called for every element during construction and again for each lookup).

Extracted duplicate `format!("{}-{}", x.device_uuid, x.feature_uuid)` to `Online::cache_key()` method; eliminated duplication at two call sites in the notification loop.

Removed unnecessary cloning in `Notification` field destructuring: changed `val.api_token.clone(), val.device_uuid.clone(), val.feature_uuid.clone()` to `let Notification { api_token, device_uuid, feature_uuid, payload } = val;` to move fields directly (val is not used afterward).

### Type correctness and overflow prevention
Stored `created_at` / `modified_at` as `u64` in `Online` instead of `String`, eliminating round-trip parsing where `find_all` parsed to `u64` then called `.to_string()`, and `filter_offline` parsed back again.

Replaced unchecked `u64` multiplications (`cache_timeout_seconds * 1000`, `offline_timeout_seconds * 1000`) with `saturating_mul(1000)` to prevent silent overflow.

Replaced `Duration::as_millis() as u64` (silent truncation of u128) with `.as_millis().try_into().unwrap_or(u64::MAX)` to make the narrowing explicit.

### Task management and cleanup
Stored the notification background task's `JoinHandle` instead of immediately dropping it after `tokio::task::spawn(...)`. Now calls `abort()` after Rocket shuts down. Without this, the task could exit silently in debug builds (when panic != "abort") while Rocket continued running with notifications stopped.

### Naming and code style
Renamed shadowed `client` variable: `let client = redis::Client::open(…)` was immediately shadowed by `let client = FcmClient::new(…)`. Renamed to `redis_client` and `fcm_client` respectively.

Renamed `appEnv` to `app_env` (Clippy `non_snake_case` violation); also replaced the duplicate `std::env::var("ENV")` re-read with `app_env.is_testing()`.

Collapsed nested `if let Some(…)` blocks in `redact_redis_uri` into a single `if let … && let …` guard (Clippy `collapsible_if`).

Replaced `format!(…).as_str()` with `&format!(…)`. Changed `OnlineMqttPayload {}` to unit-struct syntax `OnlineMqttPayload;`. Replaced five chained `fmt.write_str(…)?` calls in `Display for Topic` with a single `write!(fmt, ...)` call. Replaced UFCS `IntoIterator::into_iter([…])` with method-call syntax `[…].into_iter()`.

Removed redundant `Sized` bounds from generic type parameters (all implicitly `Sized`): changed `T: PayloadTrait + Sized + Serialize` and `T: … + Sized`.

Replaced `env::var("ENV") != Ok("testing".to_string())` with `env::var("ENV").as_deref() != Ok("testing")` to avoid allocating a `String`.

Removed duplicate `#[test]` attribute appearing directly above `#[test_log::test]` (which already expands to include `#[test]`).

---

## Configuration

### Removed dead config field
Removed `hide_rocket_log_stdout: bool` field from `Env` struct. The field was parsed from environment, stored in `Env`, printed by `print_env`, and referenced in the `Debug` impl — but no code path ever read its value to suppress Rocket's stdout output. The feature was never implemented, so the field was removed from `Env`, `Debug`, `print_env`, and `.env_template`.
