use actix_http::HttpService;
use actix_http_test::TestServer;
use actix_web::{http, web::Path, App, HttpResponse, Responder};
use actix_web_codegen::{delete, get, post, put};
use futures::{future, Future};

#[get("/test")]
fn test() -> impl Responder {
    HttpResponse::Ok()
}

#[put("/test")]
fn put_test() -> impl Responder {
    HttpResponse::Created()
}

#[post("/test")]
fn post_test() -> impl Responder {
    HttpResponse::NoContent()
}

#[get("/test")]
fn auto_async() -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    future::ok(HttpResponse::Ok().finish())
}

#[get("/test")]
fn auto_sync() -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    future::ok(HttpResponse::Ok().finish())
}

#[put("/test/{param}")]
fn put_param_test(_: Path<String>) -> impl Responder {
    HttpResponse::Created()
}

#[delete("/test/{param}")]
fn delete_param_test(_: Path<String>) -> impl Responder {
    HttpResponse::NoContent()
}

#[get("/test/{param}")]
fn get_param_test(_: Path<String>) -> impl Responder {
    HttpResponse::Ok()
}

#[test]
fn test_params() {
    let mut srv = TestServer::new(|| {
        HttpService::new(
            App::new()
                .service(get_param_test)
                .service(put_param_test)
                .service(delete_param_test),
        )
    });

    let request = srv.request(http::Method::GET, srv.url("/test/it"));
    let response = srv.block_on(request.send()).unwrap();
    assert_eq!(response.status(), http::StatusCode::OK);

    let request = srv.request(http::Method::PUT, srv.url("/test/it"));
    let response = srv.block_on(request.send()).unwrap();
    assert_eq!(response.status(), http::StatusCode::CREATED);

    let request = srv.request(http::Method::DELETE, srv.url("/test/it"));
    let response = srv.block_on(request.send()).unwrap();
    assert_eq!(response.status(), http::StatusCode::NO_CONTENT);
}

#[test]
fn test_body() {
    let mut srv = TestServer::new(|| {
        HttpService::new(
            App::new()
                .service(post_test)
                .service(put_test)
                .service(test),
        )
    });
    let request = srv.request(http::Method::GET, srv.url("/test"));
    let response = srv.block_on(request.send()).unwrap();
    assert!(response.status().is_success());

    let request = srv.request(http::Method::PUT, srv.url("/test"));
    let response = srv.block_on(request.send()).unwrap();
    assert!(response.status().is_success());
    assert_eq!(response.status(), http::StatusCode::CREATED);

    let request = srv.request(http::Method::POST, srv.url("/test"));
    let response = srv.block_on(request.send()).unwrap();
    assert!(response.status().is_success());
    assert_eq!(response.status(), http::StatusCode::NO_CONTENT);

    let mut srv = TestServer::new(|| HttpService::new(App::new().service(auto_sync)));
    let request = srv.request(http::Method::GET, srv.url("/test"));
    let response = srv.block_on(request.send()).unwrap();
    assert!(response.status().is_success());
}

#[test]
fn test_auto_async() {
    let mut srv = TestServer::new(|| HttpService::new(App::new().service(auto_async)));

    let request = srv.request(http::Method::GET, srv.url("/test"));
    let response = srv.block_on(request.send()).unwrap();
    assert!(response.status().is_success());
}
