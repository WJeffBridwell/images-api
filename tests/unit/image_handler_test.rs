use actix_web::{test, web, App};
use assert_fs::prelude::*;
use images_api::handlers::*;  // Update this with your actual handler module
use predicates::prelude::*;

#[actix_rt::test]
async fn test_get_image_success() {
    // Create a temporary directory with a test image
    let temp = assert_fs::TempDir::new().unwrap();
    let test_image = temp.child("test.jpg");
    test_image.write_binary(b"fake image content").unwrap();

    // Build test app with the temporary directory path
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(temp.path().to_path_buf()))
            .service(serve_image),
    )
    .await;

    // Create test request
    let req = test::TestRequest::get()
        .uri("/images/test.jpg")
        .to_request();

    // Perform the request and check response
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn test_get_image_not_found() {
    // Create empty temporary directory
    let temp = assert_fs::TempDir::new().unwrap();

    // Build test app
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(temp.path().to_path_buf()))
            .service(serve_image),
    )
    .await;

    // Create test request for non-existent image
    let req = test::TestRequest::get()
        .uri("/images/nonexistent.jpg")
        .to_request();

    // Perform the request and check response
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}
