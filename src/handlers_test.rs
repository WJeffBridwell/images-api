#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use image::{ImageBuffer, Rgb};
    use std::path::PathBuf;
    use std::sync::{Arc, RwLock};
    use std::time::SystemTime;
    use tempfile::TempDir;

    async fn setup_test_app() -> (TempDir, actix_web::App) {
        // Create a temporary directory for test images
        let temp_dir = TempDir::new().unwrap();
        let images_dir = PathBuf::from(temp_dir.path());
        let images_dir_data = web::Data::new(images_dir.clone());
        
        let image_cache = web::Data::new(Arc::new(RwLock::new(ImageCache::new())));
        let image_processor = web::Data::new(ImageProcessor::new());

        // Initialize the cache with the temp directory
        if let Ok(mut cache) = image_cache.write() {
            cache.populate(&images_dir).unwrap();
        }

        // Create the app with our handlers
        let app = App::new()
            .app_data(images_dir_data.clone())
            .app_data(image_cache.clone())
            .app_data(image_processor.clone())
            .service(health_check)
            .service(serve_image)
            .service(image_info)
            .service(list_images);

        (temp_dir, app)
    }

    #[actix_rt::test]
    async fn test_health_check() {
        let (_temp_dir, app) = setup_test_app().await;
        let app = test::init_service(app).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        
        let body = test::read_body(resp).await;
        let health_response: HealthResponse = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(health_response.status, "healthy");
    }

    #[actix_rt::test]
    async fn test_list_images_empty_directory() {
        let (_temp_dir, app) = setup_test_app().await;
        let app = test::init_service(app).await;
        let req = test::TestRequest::get().uri("/images").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        
        assert!(images.is_empty());
    }

    #[actix_rt::test]
    async fn test_list_images_with_pagination() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create some test images
        let test_images = vec!["test1.jpg", "test2.jpg", "test3.jpg"];
        for image in &test_images {
            std::fs::write(temp_dir.path().join(image), "fake image data").unwrap();
        }

        // Re-initialize the app with the new images
        let app = test::init_service(app).await;
        
        // Test with pagination
        let req = test::TestRequest::get()
            .uri("/images?per_page=2&page=1")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(images.len(), 2);
    }

    #[actix_rt::test]
    async fn test_image_info() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create a test image
        let test_image = "test.jpg";
        std::fs::write(temp_dir.path().join(test_image), "fake image data").unwrap();

        let app = test::init_service(app).await;
        let req = test::TestRequest::get()
            .uri("/images/test.jpg/info")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        
        let body = test::read_body(resp).await;
        let info: ImageMetadata = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(info.filename, "test.jpg");
    }

    #[actix_rt::test]
    async fn test_serve_image() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create a real test image using the image crate
        let test_image = "test.jpg";
        let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(100, 100);
        img.save(temp_dir.path().join(test_image)).unwrap();

        // Re-initialize the app with the new image
        let app = test::init_service(app).await;
        let req = test::TestRequest::get()
            .uri("/images/test.jpg")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        
        let body = test::read_body(resp).await;
        assert!(!body.is_empty());
    }

    #[actix_rt::test]
    async fn test_image_not_found() {
        let (_temp_dir, app) = setup_test_app().await;
        let app = test::init_service(app).await;
        let req = test::TestRequest::get()
            .uri("/images/nonexistent.jpg")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 404);
    }
}
