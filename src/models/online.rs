use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Online {
    pub uuid: String,
    pub apiToken: String,
    pub fcmToken: String,
    pub createdAt: String,
    pub modifiedAt: String,
}

impl IntoIterator for Online {
    type Item = String;
    type IntoIter = std::array::IntoIter<String, 5>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter([self.uuid, self.apiToken, self.fcmToken, self.createdAt, self.modifiedAt])
    }
}
