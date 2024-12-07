pub mod handlers;
pub mod image_processor;
pub mod finder;

#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};
    use crate::handlers::*;
    use crate::image_processor::ImageProcessor;
    use std::sync::{Arc, RwLock};
    use std::collections::HashMap;
    use std::io::Cursor;
    use image::{ImageBuffer, Rgb};
    use log::{debug, LevelFilter};
    use env_logger;
    use tempfile::TempDir;

    type ImageCache = HashMap<String, Vec<u8>>;

    async fn setup_test_app() -> (TempDir, impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>) {
        // Initialize logger
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let temp_dir = tempfile::tempdir().unwrap();
        let images_dir = temp_dir.path().to_owned();
        
        debug!("Test images directory: {:?}", images_dir);
        
        // Create test images directory and ensure it exists
        std::fs::create_dir_all(&images_dir).unwrap();
        
        // Create a test RGB image
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(100, 100);
        let mut buffer = Vec::new();
        img.write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Jpeg)
            .expect("Failed to create test image");
        
        let test_image_path = images_dir.join("test.jpg");
        debug!("Writing test image to: {:?}", test_image_path);
        std::fs::write(&test_image_path, &buffer).unwrap();
        
        // Verify the file was created
        assert!(test_image_path.exists(), "Test image file was not created");
        
        let processor = ImageProcessor::new();
        let image_cache = Arc::new(RwLock::new(ImageCache::new()));
        let images_dir_clone = images_dir.clone();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(processor))
                .app_data(web::Data::new(image_cache))
                .app_data(web::Data::new(images_dir_clone))
                .service(health_check)
                .service(serve_image)
                .service(image_info)
                .service(list_images)
        ).await;

        (temp_dir, app)
    }

    #[actix_web::test]
    async fn test_health_check() {
        let (_temp_dir, app) = setup_test_app().await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        
        let body: HealthResponse = test::read_body_json(resp).await;
        assert_eq!(body.status, "healthy");
    }

    #[actix_web::test]
    async fn test_serve_image() {
        let (_temp_dir, app) = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images/test.jpg")
            .to_request();
        let resp = test::call_service(&app, req).await;
        debug!("Serve image response status: {}", resp.status());
        assert!(resp.status().is_success());
        
        let content_type = resp.headers().get("content-type").unwrap();
        assert_eq!(content_type, "image/jpeg");
    }

    #[actix_web::test]
    async fn test_image_info() {
        let (_temp_dir, app) = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images/test.jpg/info")
            .to_request();
        let resp = test::call_service(&app, req).await;
        debug!("Image info response status: {}", resp.status());
        assert!(resp.status().is_success());
        
        let body: ImageMetadata = test::read_body_json(resp).await;
        assert_eq!(body.filename, "test.jpg");
        assert!(body.dimensions.is_some());
    }

    #[actix_web::test]
    async fn test_list_images() {
        let (_temp_dir, app) = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images")
            .to_request();
        let resp = test::call_service(&app, req).await;
        debug!("List images response status: {}", resp.status());
        assert!(resp.status().is_success());
        
        let body: Vec<ImageMetadata> = test::read_body_json(resp).await;
        assert!(!body.is_empty());
        assert_eq!(body[0].filename, "test.jpg");
    }
}
