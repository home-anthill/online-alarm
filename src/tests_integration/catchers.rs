use online::catchers;
use pretty_assertions::assert_eq;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::{catchers as rocket_catchers, get, routes};

#[get("/bad-request")]
fn trigger_bad_request() -> Status {
    Status::BadRequest
}

#[get("/internal-server-error")]
fn trigger_internal_server_error() -> Status {
    Status::InternalServerError
}

#[get("/service-unavailable")]
fn trigger_service_unavailable() -> Status {
    Status::ServiceUnavailable
}

#[rocket::async_test]
#[test_log::test]
async fn catchers_return_json_error_responses() {
    let client = Client::tracked(
        rocket::build()
            .mount("/", routes![trigger_bad_request, trigger_internal_server_error, trigger_service_unavailable])
            .register(
                "/",
                rocket_catchers![
                    catchers::bad_request,
                    catchers::not_found,
                    catchers::internal_server_error,
                    catchers::service_unavailable
                ],
            ),
    )
    .await
    .expect("valid rocket instance");

    for (path, expected_status, expected_body) in [
        ("/bad-request", Status::BadRequest, "Bad request"),
        ("/missing", Status::NotFound, "Not found"),
        ("/internal-server-error", Status::InternalServerError, "Internal server error"),
        ("/service-unavailable", Status::ServiceUnavailable, "Service Unavailable"),
    ] {
        let response = client.get(path).dispatch().await;
        let status = response.status();
        let content_type = response.content_type();
        let body = response.into_string().await;

        assert_eq!(expected_status, status, "{path}");
        assert_eq!(Some(ContentType::JSON), content_type, "{path}");
        assert_eq!(Some(expected_body.to_string()), body, "{path}");
    }
}
