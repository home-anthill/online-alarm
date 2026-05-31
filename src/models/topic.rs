use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub family: String,
    pub device_id: String,
    pub feature_name: String,
}

impl Topic {
    pub fn new(topic: &str) -> Option<Self> {
        // topic form is:
        //  online/device_uuid/features/feature_uuid
        let items: Vec<&str> = topic.split('/').collect();
        if items.len() != 4
            || items[0] != "online"
            || items[2] != "features"
            || items[1].is_empty()
            || items[3].is_empty()
        {
            return None;
        }
        Some(Self { family: items[0].to_string(), device_id: items[1].to_string(), feature_name: items[3].to_string() })
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}/{}/features/{}", self.family, self.device_id, self.feature_name)
    }
}

#[cfg(test)]
mod tests {
    use crate::models::topic::Topic;
    use pretty_assertions::assert_eq;

    #[test_log::test]
    fn check_topic_display() {
        let device_uuid = "246e3256-f0dd-4fcb-82c5-ee20c2267eeb";
        let feature_uuid = "b6505821-3ac9-45e6-9018-72d3ecb9b591";
        let topic: Topic = Topic::new(&format!("online/{}/features/{}", device_uuid, feature_uuid)).unwrap();
        let expected = topic.to_string();
        assert_eq!(format!("online/{}/features/{}", device_uuid, feature_uuid), expected);
    }

    #[test_log::test]
    fn check_topic_new_parses_family_device_and_feature() {
        let topic = Topic::new("online/device-uuid/features/feature-uuid").unwrap();

        assert_eq!("online", topic.family);
        assert_eq!("device-uuid", topic.device_id);
        assert_eq!("feature-uuid", topic.feature_name);
    }

    #[test_log::test]
    fn check_topic_new_returns_none_when_device_is_missing() {
        assert!(Topic::new("online").is_none());
    }

    #[test_log::test]
    fn check_topic_new_returns_none_when_feature_segment_is_missing() {
        assert!(Topic::new("online/device-uuid/feature-uuid").is_none());
    }

    #[test_log::test]
    fn check_topic_new_returns_none_when_static_segments_are_invalid() {
        assert!(Topic::new("offline/device-uuid/features/feature-uuid").is_none());
        assert!(Topic::new("online/device-uuid/feature/feature-uuid").is_none());
    }

    #[test_log::test]
    fn check_topic_new_returns_none_when_identifiers_are_empty() {
        assert!(Topic::new("online//features/feature-uuid").is_none());
        assert!(Topic::new("online/device-uuid/features/").is_none());
    }

    #[test_log::test]
    fn check_topic_display_uses_canonical_features_segment() {
        let topic = Topic {
            family: "test".to_string(),
            device_id: "device-uuid".to_string(),
            feature_name: "feature-uuid".to_string(),
        };

        assert_eq!("test/device-uuid/features/feature-uuid", topic.to_string());
    }
}
