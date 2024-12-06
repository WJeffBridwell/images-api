use std::path::PathBuf;
use actix_web::{App, web, dev::{Service, ServiceResponse}, test, Error as ActixError, body::BoxBody};
use tempfile::TempDir;
use anyhow::Result;
use std::sync::{Arc, RwLock};
use actix_http::Request;
use crate::handlers::{health_check, serve_image, image_info, list_images};
use crate::image_processor::ImageProcessor;
use crate::handlers::ImageCache;

// Test image data for different formats
pub const TEST_PNG_2X2: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
    0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00, 0xFD,
    0xD4, 0x9A, 0x73, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0x60,
    0x60, 0x60, 0x60, 0x00, 0x00, 0x00, 0x04, 0x00, 0x01, 0xE8, 0x5B, 0x91, 0x99, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82
];

pub const TEST_JPEG_2X2: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00,
    0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x02, 0x00, 0x02, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4,
    0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x03, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00,
    0x37, 0xFF, 0xD9
];

pub const TEST_GIF_2X2: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x02, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0xFF, 0xFF,
    0xFF, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x00, 0x02,
    0x02, 0x44, 0x01, 0x00, 0x3B
];

// Malformed image data for testing
pub const TEST_MALFORMED_DATA: &[u8] = &[0xFF; 32];

#[derive(Debug)]
pub struct TestImage {
    pub path: PathBuf,
    pub filename: String,
    pub format: String,
    pub data: &'static [u8],
}

pub struct TestContext {
    pub temp_dir: TempDir,
    pub images: Vec<TestImage>,
}

impl TestContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            temp_dir: TempDir::new()?,
            images: Vec::new(),
        })
    }

    pub fn create_test_image(&mut self, format: &'static str, data: &'static [u8]) -> Result<TestImage> {
        let filename = format!("test.{}", format);
        let path = self.temp_dir.path().join(&filename);
        std::fs::write(&path, data)?;
        
        let test_image = TestImage {
            path,
            filename: filename.to_string(),
            format: format.to_string(),
            data,
        };
        self.images.push(test_image.clone());
        Ok(test_image)
    }

    pub fn create_all_test_images(&mut self) -> Result<()> {
        self.create_test_image("png", TEST_PNG_2X2)?;
        self.create_test_image("jpg", TEST_JPEG_2X2)?;
        self.create_test_image("gif", TEST_GIF_2X2)?;
        Ok(())
    }

    pub fn create_malformed_image(&mut self, format: &str) -> Result<TestImage> {
        let filename = format!("malformed.{}", format);
        let path = self.temp_dir.path().join(&filename);
        
        Ok(TestImage {
            path,
            filename: filename.to_string(),
            format: format.to_string(),
            data: TEST_MALFORMED_DATA,
        })
    }
}

impl Clone for TestImage {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            filename: self.filename.clone(),
            format: self.format.clone(),
            data: self.data,
        }
    }
}

// Helper function to setup test app with proper services
pub async fn setup_test_app() -> (TestContext, impl Service<Request, Response = ServiceResponse<BoxBody>, Error = ActixError>) {
    let ctx = TestContext::new().unwrap();
    let images_dir = web::Data::new(ctx.temp_dir.path().to_path_buf());
    let processor = web::Data::new(ImageProcessor::new());
    let image_cache = web::Data::new(Arc::new(RwLock::new(ImageCache::new())));

    let app = test::init_service(
        App::new()
            .app_data(images_dir.clone())
            .app_data(processor.clone())
            .app_data(image_cache.clone())
            .service(health_check)
            .service(serve_image)
            .service(image_info)
            .service(list_images)
    ).await;

    (ctx, app)
}
