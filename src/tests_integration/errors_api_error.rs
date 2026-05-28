use online::errors::api_error::{ApiError, ApiResponse};
use pretty_assertions::assert_eq;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::serde::json::{Value, json};
use rocket::{get, routes};

#[get("/api-response")]
fn api_response() -> ApiResponse {
    ApiResponse { json: json!({ "ok": true }), code: Status::Created.code }
}

#[get("/api-error")]
fn api_error() -> ApiError {
    ApiError { message: "nope".to_string(), code: Status::BadRequest.code }
}

#[rocket::async_test]
#[test_log::test]
async fn api_response_responder_sets_status_content_type_and_json_body() {
    let client =
        Client::tracked(rocket::build().mount("/", routes![api_response])).await.expect("valid rocket instance");

    let response = client.get("/api-response").dispatch().await;
    let status = response.status();
    let content_type = response.content_type();
    let body = response.into_json::<Value>().await;

    assert_eq!(Status::Created, status);
    assert_eq!(Some(ContentType::JSON), content_type);
    assert_eq!(Some(json!({ "ok": true })), body);
}

#[rocket::async_test]
#[test_log::test]
async fn api_error_responder_sets_status_content_type_and_message_body() {
    let client = Client::tracked(rocket::build().mount("/", routes![api_error])).await.expect("valid rocket instance");

    let response = client.get("/api-error").dispatch().await;
    let status = response.status();
    let content_type = response.content_type();
    let body = response.into_string().await;

    assert_eq!(Status::BadRequest, status);
    assert_eq!(Some(ContentType::JSON), content_type);
    assert_eq!(Some("nope".to_string()), body);
}
