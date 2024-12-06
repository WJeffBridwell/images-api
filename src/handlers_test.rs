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

        let app = test::init_service(app).await;

        // Test default pagination (page 1, per_page 10)
        let req = test::TestRequest::get()
            .uri("/images")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images.len(), 3); // All images returned

        // Test custom pagination
        let req = test::TestRequest::get()
            .uri("/images?per_page=2&page=1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images.len(), 2); // Only 2 images per page

        // Test second page
        let req = test::TestRequest::get()
            .uri("/images?per_page=2&page=2")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images.len(), 1); // Last image on second page
    }

    #[actix_rt::test]
    async fn test_list_images_with_sorting() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create test images with different timestamps
        let test_images = vec![
            ("test1.jpg", 100), // size in bytes
            ("test3.jpg", 300),
            ("test2.jpg", 200),
        ];
        
        for (image, size) in &test_images {
            let content = "x".repeat(*size);
            std::fs::write(temp_dir.path().join(image), content).unwrap();
        }

        let app = test::init_service(app).await;

        // Test sorting by name ascending
        let req = test::TestRequest::get()
            .uri("/images?sort_by=name&order=asc")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images[0].filename, "test1.jpg");
        assert_eq!(images[1].filename, "test2.jpg");
        assert_eq!(images[2].filename, "test3.jpg");

        // Test sorting by size descending
        let req = test::TestRequest::get()
            .uri("/images?sort_by=size&order=desc")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images[0].filename, "test3.jpg");
        assert_eq!(images[1].filename, "test2.jpg");
        assert_eq!(images[2].filename, "test1.jpg");
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

    #[actix_rt::test]
    async fn test_pagination_edge_cases() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create 5 test images
        for i in 1..=5 {
            std::fs::write(
                temp_dir.path().join(format!("test{}.jpg", i)),
                "fake image data"
            ).unwrap();
        }

        let app = test::init_service(app).await;

        // Test page size larger than total images
        let req = test::TestRequest::get()
            .uri("/images?per_page=10&page=1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images.len(), 5);

        // Test empty page
        let req = test::TestRequest::get()
            .uri("/images?per_page=5&page=2")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images.len(), 0);

        // Test invalid page number
        let req = test::TestRequest::get()
            .uri("/images?per_page=5&page=0")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_rt::test]
    async fn test_malformed_requests() {
        let (_temp_dir, app) = setup_test_app().await;
        let app = test::init_service(app).await;

        // Test negative per_page
        let req = test::TestRequest::get()
            .uri("/images?per_page=-1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        // Test non-numeric page
        let req = test::TestRequest::get()
            .uri("/images?page=abc")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_rt::test]
    async fn test_concurrent_image_access() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create a test image
        std::fs::write(
            temp_dir.path().join("test.jpg"),
            "fake image data"
        ).unwrap();

        let app = test::init_service(app).await;

        // Create multiple concurrent requests
        let reqs: Vec<_> = (0..10)
            .map(|_| {
                test::TestRequest::get()
                    .uri("/images/test.jpg")
                    .to_request()
            })
            .collect();

        // Execute requests concurrently
        let futures: Vec<_> = reqs
            .into_iter()
            .map(|req| test::call_service(&app, req))
            .collect();

        let responses = futures::future::join_all(futures).await;

        // Verify all requests succeeded
        for resp in responses {
            assert!(resp.status().is_success());
        }
    }

    #[actix_rt::test]
    async fn test_image_format_validation() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create files with different extensions
        let valid_extensions = vec!["jpg", "jpeg", "png", "gif"];
        let invalid_extensions = vec!["txt", "pdf", "doc"];

        // Test valid extensions
        for ext in valid_extensions {
            let filename = format!("test.{}", ext);
            std::fs::write(
                temp_dir.path().join(&filename),
                "fake image data"
            ).unwrap();

            let app = test::init_service(app.clone()).await;
            let req = test::TestRequest::get()
                .uri(&format!("/images/{}", filename))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert!(resp.status().is_success(), "Failed for extension: {}", ext);
        }

        // Test invalid extensions
        for ext in invalid_extensions {
            let filename = format!("test.{}", ext);
            std::fs::write(
                temp_dir.path().join(&filename),
                "fake image data"
            ).unwrap();

            let app = test::init_service(app.clone()).await;
            let req = test::TestRequest::get()
                .uri(&format!("/images/{}", filename))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), http::StatusCode::UNSUPPORTED_MEDIA_TYPE, 
                      "Should fail for extension: {}", ext);
        }
    }

    #[actix_rt::test]
    async fn test_list_images_sorting() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create test images with different timestamps
        let test_files = vec![
            ("test3.jpg", SystemTime::now()),
            ("test1.jpg", SystemTime::now() - std::time::Duration::from_secs(100)),
            ("test2.jpg", SystemTime::now() - std::time::Duration::from_secs(50))
        ];

        for (name, time) in test_files {
            let path = temp_dir.path().join(name);
            std::fs::write(&path, "test data").unwrap();
            filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(time)).unwrap();
        }

        let app = test::init_service(app).await;

        // Test default sorting (by name)
        let req = test::TestRequest::get()
            .uri("/images")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let images: Vec<ImageMetadata> = serde_json::from_slice(&body).unwrap();
        assert_eq!(images.len(), 3);
        assert!(images[0].filename.contains("test1"));
    }

    #[actix_rt::test]
    async fn test_image_info_details() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create a test image
        let image_path = temp_dir.path().join("test.jpg");
        let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(100, 150);
        img.save(&image_path).unwrap();

        let app = test::init_service(app).await;

        // Test image info endpoint
        let req = test::TestRequest::get()
            .uri("/images/test.jpg/info")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        
        let body = test::read_body(resp).await;
        let info: ImageMetadata = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(info.filename, "test.jpg");
        assert_eq!(info.dimensions, Some((100, 150)));
        assert!(info.size_bytes > 0);
        assert!(info.format.is_some());
    }

    #[actix_rt::test]
    async fn test_serve_image_headers() {
        let (temp_dir, app) = setup_test_app().await;
        
        // Create a test image
        let image_path = temp_dir.path().join("test.jpg");
        let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(100, 100);
        img.save(&image_path).unwrap();

        let app = test::init_service(app).await;

        // Test image serving with headers
        let req = test::TestRequest::get()
            .uri("/images/test.jpg")
            .to_request();
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        let headers = resp.headers();
        assert!(headers.contains_key("content-type"));
        assert_eq!(
            headers.get("content-type").unwrap().to_str().unwrap(),
            "image/jpeg"
        );
    }

    #[actix_rt::test]
    async fn test_health_check_details() {
        let (_temp_dir, app) = setup_test_app().await;
        let app = test::init_service(app).await;
        
        let req = test::TestRequest::get()
            .uri("/health")
            .to_request();
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let health: serde_json::Value = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(health["status"], "healthy");
        assert!(health["timestamp"].is_string());
    }

    #[actix_web::test]
    async fn test_serve_image_not_found() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images/nonexistent.jpg")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_image_info_not_found() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images/nonexistent.jpg/info")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_list_images_invalid_path() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images?page=invalid&per_page=10")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_list_images_empty_dir() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::OK);
        
        let body: Value = test::read_body_json(resp).await;
        assert!(body.get("images").unwrap().as_array().unwrap().is_empty());
    }

    #[actix_web::test]
    async fn test_serve_image_not_found() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images/nonexistent.jpg")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_image_info_not_found() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images/nonexistent.jpg/info")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_list_images_invalid_page() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images?page=invalid&per_page=10")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_list_images_invalid_per_page() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images?page=1&per_page=invalid")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_list_images_negative_page() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images?page=-1&per_page=10")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_list_images_zero_per_page() {
        let app = setup_test_app().await;
        let req = test::TestRequest::get()
            .uri("/images?page=1&per_page=0")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_list_images_invalid_page() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(ImageProcessor::new()))
                .service(list_images),
        ).await;

        let req = test::TestRequest::get()
            .uri("/images?page=-1&per_page=10")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_list_images_invalid_per_page() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(ImageProcessor::new()))
                .service(list_images),
        ).await;

        let req = test::TestRequest::get()
            .uri("/images?page=1&per_page=0")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_serve_image_not_found() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(ImageProcessor::new()))
                .service(serve_image),
        ).await;

        let req = test::TestRequest::get()
            .uri("/images/nonexistent.jpg")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_image_info_not_found() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(ImageProcessor::new()))
                .service(image_info),
        ).await;

        let req = test::TestRequest::get()
            .uri("/images/nonexistent.jpg/info")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_list_images_directory_error() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(ImageProcessor::new()))
                .service(list_images),
        ).await;

        // Set an invalid images directory path
        std::env::set_var("IMAGES_DIR", "/nonexistent/path");

        let req = test::TestRequest::get()
            .uri("/images")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        // Reset the images directory path
        std::env::remove_var("IMAGES_DIR");
    }
}
