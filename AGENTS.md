# AGENTS.md

This file provides guidance to coding agents when working with code in this repository.

## Project Overview

Rust microservice that monitors device online status via Redis and sends Firebase Cloud Messaging (FCM) notifications when devices go offline. Part of the home-anthill ecosystem.

## Build & Development Commands

All commands use the Makefile:

- `make build` — format, lint (clippy), and build (default target)
- `make release` — production build with optimizations
- `make run` — hot-reload development server via cargo-watch
- `make test` — run all tests (single-threaded, with backtrace)
- `make test-coverage` — generate HTML/LCOV coverage reports via grcov
- `make check` — run `cargo audit` to find known vulnerabilities
- `make fmt` — format code with `cargo fmt`
- `make lint` — run `cargo clippy`
- `make clean` — clean build artifacts
- `make deps` — install dev dependencies (cargo-watch, grcov, llvm-tools)

Run a single test:
```bash
ENV=testing RUST_BACKTRACE=full cargo test <test_name> -- --nocapture --test-threads 1
```

**Tests**: The codebase has **unit tests only** (in `src/models/topic.rs`) that test parsing and model logic. No external infrastructure (Redis, FCM, etc.) is required — `ENV=testing` just switches the Redis key pattern for integration tests if they were added. Tests run single-threaded to ensure deterministic behavior.

## Architecture

**Main flow** (`src/main.rs`):
1. Initializes logging (rolling file appenders split by level) and loads env config
2. Connects to Redis (`redis_client`) and initializes FCM hub (`fcm_hub`) via `google-fcm1`
3. Creates a `DashMap` cache for notification deduplication (tracks recently-sent device notifications with configurable TTL)
4. Spawns a background tokio task (`notification_handle`) that:
   - Polls Redis every 10 seconds for all devices
   - Detects devices whose `modifiedAt` timestamp exceeds the offline threshold (`OFFLINE_TIMEOUT_SECONDS`)
   - Sends FCM notifications only for devices not in the deduplication cache
   - Updates the cache to prevent duplicate notifications within the timeout window
   - Logs and continues on per-device errors (missing FCM tokens, transient Redis failures)

   The `JoinHandle` is stored and `abort()`ed when Rocket shuts down to ensure clean shutdown.
5. Starts the Rocket HTTP server

**Deduplication mechanism**: A `DashMap<String, u64>` cache stores `cache_key()` → `timestamp` entries for recently-notified devices. When an offline device is detected, the cache is checked; if the device key exists and was inserted less than `CACHE_TIMEOUT_SECONDS` ago, the notification is skipped (preventing FCM spam for a device stuck offline). Cache entries are not explicitly expired; stale entries stay in memory until the next poll encounters the same device and sees its timestamp. This is acceptable because: (1) devices eventually come back online (cache key is never checked again), and (2) the DashMap is small (typically <1000 entries) relative to typical device counts.

**Module structure**:
- `config/` — logging setup, env var loading (`REDIS_URI`, `REDIS_USERNAME`, `REDIS_PASSWORD`, `CACHE_TIMEOUT_SECONDS`, `OFFLINE_TIMEOUT_SECONDS`, `FCM_SERVICE_ACCOUNT_KEY_PATH`). Logging uses rolling file appenders (daily rotation, 5 files max): `info*.log` for INFO and below (no target filter — includes all crates), `error*.log` for ERROR only (filtered to `target: "app"`), stdout also filtered to `target: "app"`. `set_global_default` is used instead of `.init()` intentionally so Rocket can install its own `RocketLogger` for startup output. An invalid `LOG_LEVEL` value is reported via `eprintln!` before the subscriber is installed (tracing is not yet available at that point), then falls back to `DEBUG`. Redis credentials are redacted in logs and debug output to prevent accidental exposure.
- `routes/` — single REST endpoint: `GET /keepalive` (health check for Kubernetes probes)
- `models/` — `Online` (device status with `device_uuid`, `feature_uuid`, `api_token`, `fcm_token`; `created_at`/`modified_at` are `u64` millisecond epoch timestamps; `cache_key()` → `"{device_uuid}-{feature_uuid}"`), `Topic` (MQTT topic parser: `online/{device_uuid}/features/{feature_uuid}`). `Online` is constructed directly from Redis hash fields with no serde derives.
- `db/` — Redis operations: `find_all` scans `online_*` keys (or `test_*` when `ENV=testing`). Redis hash fields per key: `apiToken`, `fcmToken`, `createdAt`, `modifiedAt` (millisecond epoch timestamps stored as decimal strings). SCAN iteration errors and per-key `HGETALL` errors are logged and skipped per-device (not propagated). `filter_offline` compares `modified_at` (u64) directly against the computed threshold. `filter_online` uses a zero-allocation `HashSet<(&str, &str)>` of `(device_uuid, feature_uuid)` string-slice pairs for efficient dedup during update.
- `errors/` — custom error types (`DbError`, `RedisError`, `ApiError`) implementing Rocket's `Responder`; error variants wrap their source errors via `#[source]`
- `catchers/` — HTTP error handlers (400, 404, 500, 503)

## Local Development Setup

Before running the service locally:

1. **Start Redis** (required):
   ```bash
   # Option 1: Via Docker
   docker run --name redis -p 6379:6379 -d redis redis-server \
     --save 60 1 --loglevel warning --user redisuser on '>Password1!' '~*' '+@all'

   # Option 2: Via Docker Compose (from home-anthill root)
   cd sharded-mongodb-compose && docker compose up -d redis && cd ..
   ```

2. **Generate Firebase service account key** (for FCM notifications):
   - Visit [Firebase Console](https://console.firebase.google.com/)
   - Project Settings → Service Accounts → Generate new private key
   - Save as `serviceAccountKey.json` in the project root (gitignored)

3. **Copy `.env_template` to `.env`** and verify values match your local Redis setup.

4. **Run the service**:
   ```bash
   make run   # Hot-reload development server via cargo-watch
   ```

## Configuration

- **Rocket.toml**: Debug port 8088 (localhost), Release port 80 (0.0.0.0). **Note:** Redis, environment, and secrets are not configured here — all come from `.env` and runtime env vars. `secret_key` is **not** stored in the file; set `ROCKET_SECRET_KEY` env var at runtime (`openssl rand -base64 32`).
- **Environment**: Copy `.env_template` to `.env` for local dev. Required vars: `REDIS_URI`, `REDIS_USERNAME`, `REDIS_PASSWORD`, `CACHE_TIMEOUT_SECONDS`, `OFFLINE_TIMEOUT_SECONDS`, `FCM_SERVICE_ACCOUNT_KEY_PATH`, `ROCKET_SECRET_KEY`. Optional: `LOG_LEVEL` (default: `debug`).
- **Redis URI**: Supports both `redis://` and `rediss://` (TLS). When `REDIS_PASSWORD` is set, credentials are injected as `scheme://username:password@host:port` using percent-encoding via the `urlencoding` crate. Special characters in username/password are URL-encoded to prevent URI corruption. If `REDIS_USERNAME` is set but `REDIS_PASSWORD` is empty, a warning is logged and no authentication is attempted.
- **FCM credentials**: Path to `serviceAccountKey.json` is set via `FCM_SERVICE_ACCOUNT_KEY_PATH` (defaults to `./serviceAccountKey.json`). Must be injected at runtime — not baked into the Docker image.
- **Rust edition**: 2024; release profile uses LTO, opt-level 3, panic=abort
- **Formatting**: `rustfmt.toml` — 4 spaces, max width 120

## Error Handling Strategy

The notification loop prioritizes resilience over consistency. When processing all devices:

- **Per-device errors are logged and skipped**, not propagated to abort the entire operation. Examples:
  - Missing or empty FCM token for a device: log warning, skip notification for that device, continue to next device
  - HGETALL failure on a Redis key: log error, skip that device, continue scanning
  - Transient FCM API failures: log error, skip notification for that device (next poll will retry)

- **SCAN errors during Redis key iteration**: logged but don't prevent processing of keys that were successfully fetched

- **HTTP error responses** are handled via custom error types (`DbError`, `RedisError`, `ApiError`) that implement Rocket's `Responder` trait to return JSON-formatted errors.

This design ensures one misbehaving device or transient Redis issue doesn't disable notifications for all other devices. The tradeoff: a device might miss a notification due to a transient error, but it will be retried on the next poll cycle (10 seconds later).

## Runtime secrets (never hardcode)

| Secret | How to supply |
|--------|--------------|
| `ROCKET_SECRET_KEY` | `openssl rand -base64 32` → env var |
| `FCM_SERVICE_ACCOUNT_KEY_PATH` | Mount file via Docker volume / secret |
| `REDIS_URI` | Env var; credentials in URI are redacted in logs |

## CI/CD

GitHub Actions workflow (`.github/workflows/docker-image.yml`):
- Test job: installs Rust, Redis, Mosquitto; runs `make test-coverage`
- Build job: multi-platform Docker image via buildx, published to `ks89/online-alarm` on Docker Hub
- The Docker runtime image does **not** bundle `.env` or `serviceAccountKey.json` — both must be injected at deploy time

## Recent Changes

See `CHANGELOG.md` for detailed security and idiomatic Rust improvements made in recent reviews. Key areas:
- Credential redaction in logs and debug output (Redis URI, password)
- URL-encoding of special characters in Redis credentials
- Proper error handling for Redis SCAN failures and missing FCM tokens
- Support for TLS Redis URIs (`rediss://`)
- Removal of dead MQTT messaging code and unused imports
