use online::catchers;
use pretty_assertions::assert_eq;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::{catchers as rocket_catchers, routes};

#[rocket::async_test]
#[test_log::test]
async fn not_found_catcher_returns_json_error_response() {
    let client =
        Client::tracked(rocket::build().mount("/", routes![]).register("/", rocket_catchers![catchers::not_found]))
            .await
            .expect("valid rocket instance");

    let response = client.get("/missing").dispatch().await;
    let status = response.status();
    let content_type = response.content_type();
    let body = response.into_string().await;

    assert_eq!(Status::NotFound, status);
    assert_eq!(Some(ContentType::JSON), content_type);
    assert_eq!(Some("Not found".to_string()), body);
}
