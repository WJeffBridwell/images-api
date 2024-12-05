use assert_fs::TempDir;
use std::path::PathBuf;

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub temp_dir: TempDir,
}

impl TestApp {
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let port = 8082; // Use a different port for tests
        
        TestApp {
            address: format!("http://localhost:{}", port),
            port,
            temp_dir,
        }
    }

    pub fn get_test_image_path(&self) -> PathBuf {
        self.temp_dir.path().join("test.jpg")
    }

    pub async fn health_check(&self) -> reqwest::Response {
        reqwest::Client::new()
            .get(&format!("{}/health", self.address))
            .send()
            .await
            .expect("Failed to execute health check request")
    }
}

// Create a sample image for testing
pub fn create_test_image(path: &PathBuf) {
    std::fs::write(path, b"fake image content").expect("Failed to create test image");
}
