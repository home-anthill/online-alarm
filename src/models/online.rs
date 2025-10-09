use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Online {
    pub apiToken: String,
    pub deviceUuid: String,
    pub featureUuid: String,
    pub fcmToken: String,
    pub createdAt: String,
    pub modifiedAt: String,
}

impl IntoIterator for Online {
    type Item = String;
    type IntoIter = std::array::IntoIter<String, 6>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter([
            self.apiToken,
            self.deviceUuid,
            self.featureUuid,
            self.fcmToken,
            self.createdAt,
            self.modifiedAt,
        ])
    }
}
