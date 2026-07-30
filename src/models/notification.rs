use crate::models::alarm::AlarmEvent;
use crate::models::online::Online;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationDevice {
    pub api_token: String,
    pub device_uuid: String,
    pub feature_uuid: String,
    pub created_at: u64,
    pub modified_at: u64,
    pub alarm_type: Option<String>,
}

impl From<&Online> for NotificationDevice {
    fn from(value: &Online) -> Self {
        Self {
            api_token: value.api_token.clone(),
            device_uuid: value.device_uuid.clone(),
            feature_uuid: value.feature_uuid.clone(),
            created_at: value.created_at,
            modified_at: value.modified_at,
            alarm_type: None,
        }
    }
}

impl From<&AlarmEvent> for NotificationDevice {
    fn from(value: &AlarmEvent) -> Self {
        Self {
            api_token: value.api_token.clone(),
            device_uuid: value.device_uuid.clone(),
            feature_uuid: value.feature_uuid.clone(),
            created_at: value.created_at,
            modified_at: value.received_at,
            alarm_type: Some(value.alarm_type.clone()),
        }
    }
}
