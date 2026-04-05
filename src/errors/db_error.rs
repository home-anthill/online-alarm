use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Cannot parse string to numeric value: {0}")]
    DbStrToNumError(#[source] std::num::ParseIntError),
    #[error("Required field '{0}' is missing")]
    DbMissingFieldError(String),
}
