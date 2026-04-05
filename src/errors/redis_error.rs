use thiserror::Error;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error("Cannot get keys: {0}")]
    GetKeysError(#[source] redis::RedisError),
}
