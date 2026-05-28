use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Cannot parse string to numeric value: {0}")]
    DbStrToNumError(#[source] std::num::ParseIntError),
    #[error("Required field '{0}' is missing")]
    DbMissingFieldError(String),
}

#[cfg(test)]
mod tests {
    use crate::errors::db_error::DbError;

    #[test_log::test]
    fn missing_field_error_mentions_the_missing_field() {
        let err = DbError::DbMissingFieldError("createdAt".to_string());
        assert_eq!("Required field 'createdAt' is missing", err.to_string());
    }

    #[test_log::test]
    fn parse_error_includes_the_parse_failure() {
        let source = "not-a-number".parse::<u64>().unwrap_err();
        let err = DbError::DbStrToNumError(source);
        assert_eq!("Cannot parse string to numeric value: invalid digit found in string", err.to_string());
    }
}
