use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder, Error};
use actix_files::NamedFile;
use mime_guess::from_path;
use std::path::{Path, PathBuf};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Serialize, Deserialize};
use log::{debug, error};
use mongodb::Database;
use mongodb::bson::{self, doc, Document, Bson};
use mongodb::options::FindOptions;
use futures::TryStreamExt;
use std::sync::LazyLock;
use image::{GenericImageView, io::Reader as ImageReader};
use std::io::Cursor;
use tokio::fs;
use tokio_util::codec::{BytesCodec, FramedRead};
use std::process::Command;
use serde_json::json;
use futures::StreamExt;

pub struct AppState {
    // Add any fields your application needs to share across requests
}

fn default_page() -> usize {
    1
}

fn default_limit() -> usize {
    20
}

fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            return matches!(ext_str.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp");
        }
    }
    false
}

#[derive(Debug, Serialize)]
pub struct PaginatedImageResponse {
    pub items: Vec<ImageDetail>,
    pub total: i32,
    pub page: i32,
    pub total_pages: i32,
    pub page_size: i32,
}

#[derive(Debug, Serialize)]
pub struct ImageDetail {
    pub name: String,
    pub path: String,
    pub size: u64,
    #[serde(rename = "modifiedDate")]
    pub modified_date: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(rename = "type")]
    pub image_type: String,
    pub dimensions: ImageDimensions,
}

#[derive(Debug, Serialize)]
pub struct ImageDimensions {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize)]
pub struct GalleryImagesQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub sort: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListImagesQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub sort: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenInPreviewRequest {
    pub filepath: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageContentQuery {
    pub image_name: String,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Response structure for health check endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Status of the service
    pub status: String,
    /// Commit hash of the service
    pub commit: String,
    /// Timestamp of the response
    pub timestamp: String,
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
    // Get the current commit hash
    let commit_hash = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let response = HealthResponse {
        status: "healthy".to_string(),
        commit: commit_hash,
        timestamp: Utc::now().to_rfc3339(),
    };
    HttpResponse::Ok().json(response)
}

/// Image listing endpoint handler
/// 
/// Returns a paginated list of available images
/// 
/// Query parameters:
/// - page: Page number (default: 1)
/// - limit: Items per page (default: 20)
/// - sort: Sort order (default: name-asc)
/// - tag: Filter by tag (optional)
#[get("/gallery/images")]
pub async fn list_images(
    db: web::Data<Database>,
    query: web::Query<ListImagesQuery>,
) -> Result<HttpResponse, Error> {
    let page = query.page;
    let limit = query.limit;
    let skip = (page - 1) * limit;

    let mut images = Vec::with_capacity(limit);
    
    // Initialize static empty document and vector for defaults
    static EMPTY_DOC: LazyLock<Document> = LazyLock::new(|| Document::new());
    static EMPTY_VEC: LazyLock<Vec<Bson>> = LazyLock::new(|| Vec::new());
    
    let collection = db.collection::<Document>("models");

    // Build sort document based on sort parameter
    let sort_doc = match query.sort.as_deref() {
        Some("name-asc") => doc! { "name": 1 },
        Some("name-desc") => doc! { "name": -1 },
        Some("date-asc") => doc! { "modified": 1 },
        Some("date-desc") => doc! { "modified": -1 },
        _ => doc! { "name": 1 }, // Default sort
    };

    // Build filter document for tag filtering
    let filter = match &query.tag {
        Some(tag) => doc! { "tags": tag },
        None => Document::new(),
    };

    let mut cursor = collection
        .find(filter.clone(), FindOptions::builder()
            .sort(sort_doc)
            .skip(skip as u64)
            .limit(limit as i64)
            .build())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    while let Some(doc_result) = cursor.try_next().await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))? {
        let filename = doc_result.get_str("filename").unwrap_or("unknown");
        let path = doc_result.get_str("path").unwrap_or("unknown");
        
        // Get size from base_attributes
        let size = if let Ok(base_attrs) = doc_result.get_document("base_attributes") {
            base_attrs.get_i32("size").unwrap_or(0) as u64
        } else {
            0
        };

        let modified = if let Ok(base_attrs) = doc_result.get_document("base_attributes") {
            if let Ok(modified_time) = base_attrs.get_f64("modified") {
                Utc.timestamp_opt(modified_time as i64, 0)
                    .single()
                    .unwrap_or_else(|| Utc::now())
            } else {
                Utc::now()
            }
        } else {
            Utc::now()
        };

        // Get image metadata from macos_attributes.mdls
        let (width, height, image_type) = if let Ok(macos_attrs) = doc_result.get_document("macos_attributes") {
            if let Ok(mdls) = macos_attrs.get_document("mdls") {
                let width = mdls.get_str("kMDItemPixelWidth")
                    .map(|w| w.parse::<i32>().unwrap_or(0))
                    .unwrap_or(0);
                let height = mdls.get_str("kMDItemPixelHeight")
                    .map(|h| h.parse::<i32>().unwrap_or(0))
                    .unwrap_or(0);
                let image_type = mdls.get_str("kMDItemKind")
                    .map(|t| t.to_string())
                    .unwrap_or_else(|_| "unknown".to_string());
                (width, height, image_type)
            } else {
                (0, 0, "unknown".to_string())
            }
        } else {
            (0, 0, "unknown".to_string())
        };

        let tags = if let Ok(base_attrs) = doc_result.get_document("base_attributes") {
            base_attrs.get_array("tags")
                .unwrap_or(&EMPTY_VEC)
                .iter()
                .filter_map(|tag| tag.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        let image = ImageDetail {
            name: filename.to_string(),
            path: format!("/api/gallery/proxy-image/{}", filename),
            size,
            modified_date: modified.to_rfc3339(),
            tags,
            image_type,
            dimensions: ImageDimensions {
                width,
                height,
            },
        };
        images.push(image);
    }

    let total = collection.count_documents(filter, None).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let response = json!({
        "items": images,
        "total": total,
        "page": page,
        "totalPages": (total as f64 / limit as f64).ceil() as i32,
        "pageSize": limit as i32
    });

    Ok(HttpResponse::Ok().json(response))
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
) -> impl Responder {
    let path = images_dir.join(filename.as_ref());
    
    if !path.exists() {
        return HttpResponse::NotFound().body("Image not found");
    }

    let metadata = ImageMetadata {
        filename: filename.to_string(),
        dimensions: None,
        size_bytes: 0,
        last_modified: Utc::now(),
        format: None,
        data: None,
    };
    HttpResponse::Ok().json(metadata)
}

/// Image content search handler
/// 
/// This endpoint searches for content (movies, archives, folders) related to an image name
/// using the macOS Finder API.
#[get("/image-content")]
pub async fn search_image_content(
    req: HttpRequest,
    query: web::Query<ImageContentQuery>,
) -> Result<HttpResponse, Error> {
    let image_name = &query.image_name;
    let page = query.page;
    let limit = query.limit;

    // Check if image_name is provided
    if image_name.is_empty() {
        error!("No image_name provided in request");
        return Ok(HttpResponse::BadRequest().json(json!({
            "error": "No image_name provided"
        }))); 
    }

    debug!("Searching for content related to image: {} (page {}, limit {})", image_name, page, limit);
    let content = crate::finder::search_content(image_name, page, limit);
    debug!("Found {} content items out of {} total", content.items.len(), content.total);

    Ok(HttpResponse::Ok().json(content))
}

#[post("/open-in-preview")]
pub async fn open_in_preview(form: web::Form<OpenInPreviewRequest>, _images_dir: web::Data<std::path::PathBuf>) -> impl Responder {
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

/// Initialize all routes for the application
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check)
        .service(list_images)
        .service(serve_image)
        .service(image_info)
        .service(search_image_content)
        .service(open_in_preview)
        .service(view_content)
        .service(
            actix_files::Files::new("/static", "static")
                .show_files_listing()
                .use_last_modified(true)
                .prefer_utf8(true)
                .use_etag(true)
        );
}
