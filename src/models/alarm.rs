#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlarmEvent {
    pub id: String,
    pub api_token: String,
    pub device_uuid: String,
    pub feature_uuid: String,
    pub alarm_type: String,
    pub payload: String,
    pub created_at: u64,
    pub received_at: u64,
    pub fcm_token: String,
}
