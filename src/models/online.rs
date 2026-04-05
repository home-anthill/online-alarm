#[derive(Debug, Clone)]
pub struct Online {
    pub api_token: String,
    pub device_uuid: String,
    pub feature_uuid: String,
    pub fcm_token: String,
    pub created_at: u64,
    pub modified_at: u64,
}

impl Online {
    pub fn cache_key(&self) -> String {
        format!("{}-{}", self.device_uuid, self.feature_uuid)
    }
}
