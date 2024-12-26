use thiserror::Error;

// custom error, based on 'thiserror' library
#[derive(Error, Debug)]
pub enum DbError {
    #[error("Value not found in db object")]
    DbNotFound,
    #[error("Cannot parse string to numeric value")]
    DbStrToNumError,
}
