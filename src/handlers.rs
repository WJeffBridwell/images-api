/*! 
 * Images API - Request Handlers
 * 
 * This module contains all HTTP request handlers for the Images API service.
 * It provides endpoints for:
 * - Health check monitoring
 * - Image listing and pagination
 * - Image serving with caching
 * - Image metadata retrieval
 * - Image content search
 */

use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest};
use actix_files::NamedFile;
use chrono::Utc;
use log::error;
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use futures::StreamExt;
use tokio_util::codec::{BytesCodec, FramedRead};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{self, json};
use std::process::Command;
use actix_web::http::header::{ContentDisposition, DispositionType};
use mime_guess::from_path;

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

/// Image content search handler
/// 
/// This endpoint searches for content (movies, archives, folders) related to an image name
/// using the macOS Finder API.
#[get("/image-content/{image_name}")]
pub async fn search_image_content(
    image_name: web::Path<String>,
) -> actix_web::Result<impl Responder> {
    log::debug!("Received request for image content search with image_name: {}", image_name);
    log::debug!("Calling search_content");
    let content = crate::finder::search_content(&image_name);
    log::debug!("Search complete, found {} content items", content.len());
    log::debug!("Content items: {:?}", content);
    log::debug!("About to construct HttpResponse");
    
    // Try serializing the content first to debug
    if let Ok(json_str) = serde_json::to_string(&content) {
        log::debug!("Successfully serialized content to JSON, length: {}", json_str.len());
        if json_str.len() > 1000 {
            log::debug!("First 1000 chars of JSON: {}", &json_str[..1000]);
        } else {
            log::debug!("Full JSON: {}", json_str);
        }
    } else {
        log::error!("Failed to serialize content to JSON");
    }
    
    let response = HttpResponse::Ok().json(content);
    log::debug!("Response constructed successfully");
    log::debug!("About to return response");
    
    Ok(response)
}

#[post("/open-in-preview")]
pub async fn open_in_preview(form: web::Form<OpenInPreviewRequest>, images_dir: web::Data<std::path::PathBuf>) -> impl Responder {
    let filepath = &form.filepath;
    
    // Check if the file exists
    let path = std::path::Path::new(filepath);
    if !path.exists() {
        error!("File not found: {}", filepath);
        return HttpResponse::NotFound().json(json!({
            "status": "error",
            "message": format!("File not found: {}", filepath)
        }));
    }
    
    // Log file details
    log::debug!("Opening file in Preview: {}", filepath);
    log::debug!("File exists: {}", path.exists());
    log::debug!("File is absolute: {}", path.is_absolute());
    if let Ok(metadata) = std::fs::metadata(path) {
        log::debug!("File size: {} bytes", metadata.len());
        log::debug!("File permissions: {:?}", metadata.permissions());
    }
    
    // Construct the command
    let preview_cmd = Command::new("open")
        .args(["-a", "Preview", filepath])
        .output();
    
    // Log command details
    log::debug!("Preview command: open -a Preview {}", filepath);
    
    match preview_cmd {
        Ok(output) => {
            log::debug!("Command status: {:?}", output.status);
            log::debug!("Command stdout: {}", String::from_utf8_lossy(&output.stdout));
            log::debug!("Command stderr: {}", String::from_utf8_lossy(&output.stderr));
            
            if output.status.success() {
                log::debug!("Successfully opened file in Preview");
                HttpResponse::Ok().json(json!({ "status": "success" }))
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                error!("Preview command failed: {}", error_msg);
                HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": format!("Preview command failed: {}", error_msg)
                }))
            }
        },
        Err(e) => {
            error!("Failed to execute Preview command: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to execute Preview command: {}", e)
            }))
        }
    }
}

#[get("/view/{name}")]
pub async fn view_content(req: HttpRequest, name: web::Path<String>) -> impl Responder {
    let path = format!("static/{}", name);
    match NamedFile::open(path) {
        Ok(file) => file.into_response(&req),
        Err(_) => HttpResponse::NotFound().body("File not found"),
    }
}

#[get("/videos/haley-reed/{filename}")]
pub async fn serve_video(req: HttpRequest, filename: web::Path<String>) -> Result<HttpResponse, actix_web::Error> {
    let video_path = PathBuf::from("/Volumes/VideosHaley-Hime/haley-reed").join(filename.as_ref());
    
    if !video_path.exists() {
        return Err(actix_web::error::ErrorNotFound("Video not found"));
    }

    let file = tokio::fs::File::open(&video_path).await?;
    let metadata = file.metadata().await?;
    let file_size = metadata.len();

    let content_type = from_path(&video_path)
        .first_or_octet_stream()
        .to_string();

    // Handle range request
    if let Some(range) = req.headers().get("range") {
        let range_str = range.to_str().map_err(|_| actix_web::error::ErrorBadRequest("Invalid range header"))?;
        let (start, end) = parse_range(range_str, file_size)?;
        let length = end - start + 1;

        // Seek to the start position
        use tokio::io::AsyncSeekExt;
        let mut file = file;
        file.seek(std::io::SeekFrom::Start(start)).await?;

        // Create a chunked stream for the range
        let stream = FramedRead::new(tokio::io::AsyncReadExt::take(file, length), BytesCodec::new())
            .map(|result| {
                result.map(|bytes| bytes.freeze())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });

        Ok(HttpResponse::PartialContent()
            .insert_header(("Content-Range", format!("bytes {}-{}/{}", start, end, file_size)))
            .insert_header(("Accept-Ranges", "bytes"))
            .insert_header(("Content-Length", length.to_string()))
            .insert_header(("Content-Type", content_type))
            .streaming(stream))
    } else {
        // No range requested - serve entire file in chunks
        let stream = FramedRead::new(file, BytesCodec::new())
            .map(|result| {
                result.map(|bytes| bytes.freeze())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });

        Ok(HttpResponse::Ok()
            .insert_header(("Accept-Ranges", "bytes"))
            .insert_header(("Content-Length", file_size.to_string()))
            .insert_header(("Content-Type", content_type))
            .streaming(stream))
    }
}

fn parse_range(range: &str, file_size: u64) -> Result<(u64, u64), actix_web::Error> {
    let range = range.strip_prefix("bytes=").ok_or_else(|| {
        actix_web::error::ErrorBadRequest("Invalid range header format")
    })?;

    let (start_str, end_str) = range.split_once('-').ok_or_else(|| {
        actix_web::error::ErrorBadRequest("Invalid range header format")
    })?;

    let start: u64 = if start_str.is_empty() {
        0
    } else {
        start_str.parse().map_err(|_| {
            actix_web::error::ErrorBadRequest("Invalid range start")
        })?
    };

    let end: u64 = if end_str.is_empty() {
        file_size - 1
    } else {
        end_str.parse().map_err(|_| {
            actix_web::error::ErrorBadRequest("Invalid range end")
        })?
    };

    if start > end || end >= file_size {
        return Err(actix_web::error::ErrorBadRequest("Invalid range"));
    }

    Ok((start, end))
}

#[derive(Deserialize)]
pub struct OpenInPreviewRequest {
    filepath: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageContentQuery {
    /// Image name to search for related content
    pub image_name: Option<String>,
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

/// Initialize all routes for the application
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check)
        .service(list_images)
        .service(serve_image)
        .service(image_info)
        .service(search_image_content)
        .service(open_in_preview)
        .service(view_content)
        .service(serve_video)
        .service(
            actix_files::Files::new("/static", "static")
                .show_files_listing()
                .use_last_modified(true)
                .prefer_utf8(true)
                .use_etag(true)
        );
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
                .configure(init_routes)
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
