use thiserror::Error;

// custom error, based on 'thiserror' library
#[derive(Error, Debug)]
pub enum RedisError {
    #[error("Cannot get keys error")]
    GetKeysError,
    #[error("Cannot HGet all values error")]
    HGetAllError,
}
