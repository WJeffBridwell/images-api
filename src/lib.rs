pub mod handlers;
pub mod startup;

pub use handlers::*;
pub use startup::*;

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use assert_fs::prelude::*;

    #[actix_rt::test]
    async fn test_health_check() {
        let app = test::init_service(
            App::new()
                .service(health_check)
        ).await;

        let req = test::TestRequest::get()
            .uri("/health")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_rt::test]
    async fn test_serve_image() {
        let temp = assert_fs::TempDir::new().unwrap();
        let test_image = temp.child("test.jpg");
        test_image.write_binary(b"fake image content").unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(temp.path().to_path_buf()))
                .service(serve_image)
        ).await;

        let req = test::TestRequest::get()
            .uri("/images/test.jpg")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}
