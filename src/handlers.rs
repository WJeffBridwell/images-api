/*! 
 * Images API - Request Handlers
 * 
 * This module contains all HTTP request handlers for the Images API service.
 * It provides endpoints for:
 * - Health check monitoring
 * - Image listing and pagination
 * - Image serving with caching
 * - Image metadata retrieval
 */

use actix_web::{get, web, HttpResponse, Responder, HttpRequest};
use chrono::Utc;
use log::error;
use serde::{Serialize, Deserialize};
use std::path::Path;
use tokio::fs;
use futures_util::stream::StreamExt;
use tokio_util::codec::{BytesCodec, FramedRead};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Response structure for health check endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Status of the service
    pub status: String,
    /// Timestamp of the response
    pub timestamp: chrono::DateTime<Utc>,
}

/// Response structure for image metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct ImageMetadata {
    /// Filename of the image
    pub filename: String,
    /// Dimensions of the image
    pub dimensions: Option<(u32, u32)>,
    /// Size of the image in bytes
    pub size_bytes: u64,
    /// Last modified timestamp of the image
    pub last_modified: chrono::DateTime<Utc>,
    /// Format of the image
    pub format: Option<ImageFormat>,
    /// Base64 encoded image data
    pub data: Option<String>,
}

/// Image format structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFormat(String);

impl From<image::ImageFormat> for ImageFormat {
    fn from(format: image::ImageFormat) -> Self {
        ImageFormat(format.extensions_str()[0].to_string())
    }
}

/// Health check endpoint handler
/// 
/// Returns a 200 OK response if the service is healthy
#[get("/health")]
pub async fn health_check() -> impl Responder {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
    };
    HttpResponse::Ok().json(response)
}

/// Image listing endpoint handler
/// 
/// Returns a paginated list of available images
/// 
/// Query parameters:
/// - page: Page number (default: 1)
/// - limit: Items per page (default: 10)
/// - sort_by: Field to sort by (default: none)
/// - order: Sort order (default: asc)
/// - include_data: Whether to include image data in response (default: false)
#[get("/images")]
pub async fn list_images(
    query: web::Query<ListImagesQuery>,
    images_dir: web::Data<std::path::PathBuf>,
    processor: web::Data<crate::image_processor::ImageProcessor>,
) -> impl Responder {
    log::info!("Starting list_images request with limit={}, include_data={}", 
        query.limit.unwrap_or(10),
        query.include_data.unwrap_or(false)
    );
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let start = (page - 1) * limit;

    let mut images = Vec::with_capacity(limit);
    let mut count = 0;
    let mut skipped = 0;
    
    log::info!("Reading directory: {}", images_dir.display());
    let mut read_dir = match fs::read_dir(images_dir.as_ref()).await {
        Ok(dir) => dir,
        Err(e) => {
            error!("Failed to read images directory: {}", e);
            return HttpResponse::InternalServerError().body("Failed to read images directory");
        }
    };

    loop {
        if count >= limit {
            log::info!("Reached limit of {}", limit);
            break;
        }

        match read_dir.next_entry().await {
            Ok(Some(entry)) => {
                let path = entry.path();
                log::info!("Processing file: {}", path.display());
                if is_image_file(&path) {
                    if skipped < start {
                        log::info!("Skipping image for pagination: {}", path.display());
                        skipped += 1;
                        continue;
                    }

                    log::info!("Found image file: {}", path.display());
                    match processor.get_image_data(&path, query.include_data.unwrap_or(false)).await {
                        Ok(data) => {
                            log::info!("Successfully processed image: {}, size: {}", path.display(), data.size_bytes);
                            let metadata = ImageMetadata {
                                filename: path.file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown")
                                    .to_string(),
                                dimensions: Some(data.dimensions),
                                size_bytes: data.size_bytes as u64,
                                last_modified: Utc::now(),
                                format: Some(ImageFormat::from(data.format)),
                                data: if query.include_data.unwrap_or(false) {
                                    Some(STANDARD.encode(&data.content))
                                } else {
                                    None
                                },
                            };
                            images.push(metadata);
                            count += 1;
                        }
                        Err(e) => {
                            error!("Failed to get image data for {}: {}", path.display(), e);
                            continue;
                        }
                    }
                } else {
                    log::info!("Skipping non-image file: {}", path.display());
                }
            }
            Ok(None) => {
                log::info!("No more files in directory");
                break;
            }
            Err(e) => {
                error!("Failed to read directory entry: {}", e);
                continue;
            }
        }
    }

    // Sort images if requested
    if let Some(sort_by) = &query.sort_by {
        let asc = query.order.as_deref() != Some("desc");
        match sort_by.as_str() {
            "name" => {
                if asc {
                    images.sort_by(|a, b| a.filename.cmp(&b.filename));
                } else {
                    images.sort_by(|a, b| b.filename.cmp(&a.filename));
                }
            }
            "size" => {
                if asc {
                    images.sort_by_key(|img| img.size_bytes);
                } else {
                    images.sort_by_key(|img| std::cmp::Reverse(img.size_bytes));
                }
            }
            "date" => {
                if asc {
                    images.sort_by_key(|img| img.last_modified);
                } else {
                    images.sort_by_key(|img| std::cmp::Reverse(img.last_modified));
                }
            }
            _ => {}
        }
    }

    HttpResponse::Ok().json(images)
}

/// Image serving endpoint handler
/// 
/// Serves an image file with caching support
/// 
/// Path parameters:
/// - filename: Name of the image file to serve
#[get("/images/{filename}")]
pub async fn serve_image(
    _req: HttpRequest,
    filename: web::Path<String>,
    images_dir: web::Data<std::path::PathBuf>,
) -> impl Responder {
    let path = images_dir.join(filename.as_ref());
    
    if !path.exists() {
        error!("Image not found: {}", path.display());
        return HttpResponse::NotFound().body("Image not found");
    }

    let file = match fs::File::open(&path).await {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open image file: {}", e);
            return HttpResponse::InternalServerError().body("Failed to open image file");
        }
    };

    let stream = FramedRead::new(file, BytesCodec::new())
        .map(|r| r.map(|b| b.freeze()));

    // Determine content type based on file extension
    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    };

    HttpResponse::Ok()
        .content_type(content_type)
        .streaming(stream)
}

/// Image metadata endpoint handler
/// 
/// Returns metadata about a specific image
/// 
/// Path parameters:
/// - filename: Name of the image file to get info for
#[get("/images/{filename}/info")]
pub async fn image_info(
    filename: web::Path<String>,
    images_dir: web::Data<std::path::PathBuf>,
    processor: web::Data<crate::image_processor::ImageProcessor>,
) -> impl Responder {
    let path = images_dir.join(filename.as_ref());
    
    if !path.exists() {
        return HttpResponse::NotFound().body("Image not found");
    }

    match processor.get_image_data(&path, false).await {
        Ok(data) => {
            let metadata = ImageMetadata {
                filename: filename.to_string(),
                dimensions: Some(data.dimensions),
                size_bytes: data.size_bytes as u64,
                last_modified: Utc::now(),
                format: Some(ImageFormat::from(data.format)),
                data: None,
            };
            HttpResponse::Ok().json(metadata)
        }
        Err(e) => {
            error!("Failed to get image data: {}", e);
            HttpResponse::InternalServerError().body("Failed to get image data")
        }
    }
}

/// Query parameters for image listing endpoint
#[derive(Debug, Deserialize)]
pub struct ListImagesQuery {
    /// Page number (default: 1)
    pub page: Option<usize>,
    /// Items per page (default: 10)
    pub limit: Option<usize>,
    /// Field to sort by (default: none)
    pub sort_by: Option<String>,
    /// Sort order (default: asc)
    pub order: Option<String>,
    /// Whether to include image data in response (default: false)
    pub include_data: Option<bool>,
}

/// Checks if a file is an image
fn is_image_file(path: &Path) -> bool {
    let extension = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    
    matches!(extension.as_deref(), Some("jpg") | Some("jpeg") | Some("png") | Some("gif"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, http::StatusCode, App, web};
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
        
        let processor = web::Data::new(crate::image_processor::ImageProcessor::new());
        let image_cache = web::Data::new(Arc::new(RwLock::new(ImageCache::new())));
        let images_dir_data = web::Data::new(images_dir);

        let app = test::init_service(
            App::new()
                .app_data(processor)
                .app_data(image_cache)
                .app_data(images_dir_data)
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
        assert_eq!(resp.status(), StatusCode::OK);
        
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
        assert_eq!(resp.status(), StatusCode::OK);
        
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
        assert_eq!(resp.status(), StatusCode::OK);
        
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
        assert_eq!(resp.status(), StatusCode::OK);
        
        let body: Vec<ImageMetadata> = test::read_body_json(resp).await;
        assert!(!body.is_empty());
        assert_eq!(body[0].filename, "test.jpg");
    }
}
