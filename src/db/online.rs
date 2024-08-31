use log::info;

use futures::stream::TryStreamExt as _;
use mongodb::bson::{doc, Document};
use mongodb::{bson, Database};

use crate::errors::db_error::DbError;
use crate::models::online::OnlineDocument;

pub trait MongoDbModel {
    fn collection_name() -> String;
}

pub async fn find_online_by_uuid(db: &Database, uuid: &str) -> Result<Document, DbError> {
    info!(target: "app", "find_online_by_uuid - Called with uuid = {} to get online status from db", uuid);
    let collection = db.collection::<Document>("online");

    let filter = doc! { "uuid": uuid };
    let projection = doc! {"_id": 0, "online": 1, "createdAt": 1, "modifiedAt": 1};
    match collection.find_one(filter).projection(projection).await {
        Ok(doc_result) => match doc_result {
            Some(doc) => Ok(doc),
            None => Err(DbError::new(String::from("Cannot find online"))),
        },
        Err(err) => Err(DbError::new(err.to_string())),
    }
}

pub async fn find_all_online(db: &Database) -> Result<Vec<OnlineDocument>, DbError> {
    info!(target: "app", "find_online - To get all online elements from db");
    let pipeline = vec![
        doc! {
          "$match": {
            "$expr": {
              "$lt": [
                "$modifiedAt",
                {
                  "$dateSubtract": {
                    "startDate": "$$NOW",
                    "unit": "second",
                    "amount": 60
                  }
                }
              ]
            }
          }
       }
    ];
    let results = db.collection::<OnlineDocument>("online").aggregate(pipeline).await.unwrap();
    // to use try_collect you have to use 'futures::stream::TryStreamExt' on top
    let vec_cursor = results.try_collect().await.unwrap_or_else(|_| vec![]);
    let new_bills: Vec<OnlineDocument> = vec_cursor
        .into_iter()
        .map(|e| bson::from_document::<OnlineDocument>(e).unwrap())
        .collect();
    Ok(new_bills)
}


// async fn get_list_as_vec<T>(db: &Database, collection_name: &str, filter: Document) -> Vec<T>
// where
//     T: DeserializeOwned + Unpin + Send + Sync + Debug,
// {
//     let col = db.collection::<T>(collection_name);
//     let mut cursor = match col.find(filter).await {
//         Ok(cursor) => cursor,
//         Err(_) => return vec![],
//     };
//
//     let mut documents: Vec<T> = Vec::new();
//     while let Ok(Some(doc)) = cursor.try_next().await {
//         documents.push(doc);
//     }
//
//     documents
// }
