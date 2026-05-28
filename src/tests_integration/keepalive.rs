use online::routes::api::keep_alive;
use pretty_assertions::assert_eq;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::routes;
use rocket::serde::json::Value;

#[rocket::async_test]
#[test_log::test]
async fn keepalive() {
    let client = Client::tracked(rocket::build().mount("/", routes![keep_alive])).await.expect("valid rocket instance");

    let response = client.get("/keepalive").dispatch().await;
    let status = response.status();
    let content_type = response.content_type();
    let body = response.into_json::<Value>().await;

    assert_eq!(Status::Ok, status);
    assert_eq!(Some(ContentType::JSON), content_type);
    assert_eq!(Some(rocket::serde::json::json!({ "alive": true })), body);
}
