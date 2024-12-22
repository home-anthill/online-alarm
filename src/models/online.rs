use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Online {
    pub uuid: String,
    pub createdAt: u64,
    pub modifiedAt: u64,
    pub online: bool,
}
