use actix_web::{test, App};
use assert_fs::prelude::*;
use images_api::startup;  // You'll need to create this module
use std::time::Duration;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[actix_rt::test]
async fn test_full_image_workflow() {
    // Set up a temporary directory for our test server
    let temp = assert_fs::TempDir::new().unwrap();
    
    // Create a test image
    let test_image = temp.child("test.jpg");
    test_image.write_binary(b"fake image content").unwrap();

    // Start the application
    let app = startup::run(temp.path().to_path_buf()).await.expect("Failed to start application");
    
    // Create a test client
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    // Test health check
    let health_response = client
        .get("http://localhost:8081/health")
        .send()
        .await
        .unwrap();
    assert!(health_response.status().is_success());

    // Test image retrieval
    let image_response = client
        .get("http://localhost:8081/images/test.jpg")
        .send()
        .await
        .unwrap();
    assert!(image_response.status().is_success());

    // Test image info
    let info_response = client
        .get("http://localhost:8081/images/test.jpg/info")
        .send()
        .await
        .unwrap();
    assert!(info_response.status().is_success());
    
    // Clean up
    drop(app);
}
