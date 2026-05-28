use thiserror::Error;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error("Cannot get keys: {0}")]
    GetKeysError(#[source] redis::RedisError),
}

#[cfg(test)]
mod tests {
    use crate::errors::redis_error::RedisError;
    use redis::ErrorKind;

    #[test_log::test]
    fn get_keys_error_includes_the_redis_failure() {
        let source = redis::RedisError::from((ErrorKind::Io, "scan failed"));
        let err = RedisError::GetKeysError(source);

        assert_eq!("Cannot get keys: scan failed - Io", err.to_string());
    }
}
