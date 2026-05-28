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

#[cfg(test)]
mod tests {
    use super::Online;
    use pretty_assertions::assert_eq;

    fn online(device_uuid: &str, feature_uuid: &str) -> Online {
        Online {
            api_token: "api-token".to_string(),
            device_uuid: device_uuid.to_string(),
            feature_uuid: feature_uuid.to_string(),
            fcm_token: "fcm-token".to_string(),
            created_at: 1,
            modified_at: 2,
        }
    }

    #[test_log::test]
    fn cache_key_joins_device_and_feature_uuid() {
        let model = online("device-uuid", "feature-uuid");

        assert_eq!("device-uuid-feature-uuid", model.cache_key());
    }

    #[test_log::test]
    fn cache_key_keeps_uuid_contents_unchanged() {
        let model = online("246e3256-f0dd-4fcb-82c5-ee20c2267eeb", "b6505821-3ac9-45e6-9018-72d3ecb9b591");

        assert_eq!("246e3256-f0dd-4fcb-82c5-ee20c2267eeb-b6505821-3ac9-45e6-9018-72d3ecb9b591", model.cache_key());
    }
}
