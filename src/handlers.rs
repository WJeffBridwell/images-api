use actix_web::{get, post, web, Error, HttpRequest, HttpResponse, Responder};
use actix_files::NamedFile;
use chrono::Utc;
use futures::{StreamExt, TryStreamExt};
use log::{debug, error};
use mime_guess::from_path;
use mongodb::{
    bson::{doc, Document},
    options::FindOptions,
    Database,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::PathBuf,
    process::Command,
    io::Write,
    fs,
};
use tokio::fs as tokio_fs;
use tokio_util::codec::{BytesCodec, FramedRead};
use percent_encoding::percent_decode_str;
use std::borrow::Cow;
use hex;
use plist;
use tempfile::NamedTempFile;

pub struct AppState {
    // Add any fields your application needs to share across requests
}

fn default_page() -> usize {
    1
}

fn default_limit() -> usize {
    20
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedImageResponse {
    pub images: Vec<ImageMetadata>,
    pub total: i64,
    pub page: i32,
    #[serde(rename = "totalPages")]
    pub total_pages: i32,
    #[serde(rename = "pageSize")]
    pub page_size: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub name: String,
    #[serde(rename = "url")]
    pub path: String,
    pub size: i64,
    #[serde(rename = "date")]
    pub modified_date: String,
    #[serde(skip_serializing)]
    pub dimensions: Option<ImageDimensions>,
    #[serde(skip_serializing, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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
pub struct ImageDetail {
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
    _query: web::Query<ListImagesQuery>,
) -> Result<HttpResponse, Error> {
    let mut images = Vec::new();
    let collection = db.collection::<Document>("models");

    // Sort by filename ascending
    let sort_doc = doc! { "filename": 1 };
    let filter = doc! { "path": { "$not": { "$regex": "/\\.thumbnails/" } } };
    let find_options = FindOptions::builder().sort(sort_doc).build();

    let mut cursor = collection
        .find(filter, find_options)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let mut seen_names = std::collections::HashSet::new();

    while let Some(doc_result) = cursor.try_next().await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))? {
        
        let filename = doc_result.get_str("filename").unwrap_or_default();
        
        // Skip .DS_Store and .thumbnails
        if filename.starts_with(".") {
            continue;
        }

        // Get size from base_attributes
        let size = match doc_result.get_document("base_attributes") {
            Ok(attrs) => {
                if filename == "aali-kali.jpeg" {
                    debug!("Found aali-kali.jpeg");
                    debug!("Full document: {:?}", doc_result);
                    debug!("Base attributes: {:?}", attrs);
                }
                match attrs.get("size") {
                    Some(size_val) => {
                        if filename == "aali-kali.jpeg" {
                            debug!("Size value: {:?}", size_val);
                        }
                        match (size_val.as_i64(), size_val.as_i32()) {
                            (Some(size_i64), _) => {
                                if filename == "aali-kali.jpeg" {
                                    debug!("Using i64 size: {}", size_i64);
                                }
                                size_i64
                            },
                            (None, Some(size_i32)) => {
                                if filename == "aali-kali.jpeg" {
                                    debug!("Using i32 size: {}", size_i32);
                                }
                                size_i32 as i64
                            },
                            _ => {
                                error!("Size value is not an integer: {:?}", size_val);
                                0
                            }
                        }
                    },
                    None => {
                        error!("No size field found in base_attributes");
                        0
                    }
                }
            },
            Err(e) => {
                error!("Error getting base_attributes: {}", e);
                0
            }
        };

        // URL encode only spaces in filename for URL
        let encoded_filename = filename.replace(" ", "%20");

        if !seen_names.contains(filename) {
            seen_names.insert(filename.to_string());

            let date = match doc_result.get_document("base_attributes") {
                Ok(attrs) => match attrs.get_datetime("modification_time") {
                    Ok(dt) => dt.to_chrono().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    Err(e) => {
                        error!("Error getting modification_time: {}", e);
                        // Try creation_time as fallback
                        match attrs.get_datetime("creation_time") {
                            Ok(dt) => dt.to_chrono().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                            Err(_) => {
                                error!("Error getting creation_time as fallback: {}", e);
                                Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("Error getting base_attributes for date: {}", e);
                    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                }
            };

            // Get tags from macos_attributes.xattr
            let tags = match doc_result.get_document("macos_attributes") {
                Ok(attrs) => match attrs.get_document("xattr") {
                    Ok(xattr) => match xattr.get_str("com.apple.metadata:_kMDItemUserTags") {
                        Ok(hex_str) => {
                            debug!("Found user tags for {}: {}", filename, hex_str);
                            parse_binary_plist_tags(hex_str)
                        },
                        Err(e) => {
                            error!("Error getting user tags for {}: {}", filename, e);
                            Vec::new()
                        }
                    },
                    Err(e) => {
                        error!("Error getting xattr for {}: {}", filename, e);
                        Vec::new()
                    }
                },
                Err(e) => {
                    error!("Error getting macos_attributes for {}: {}", filename, e);
                    Vec::new()
                }
            };

            images.push(json!({
                "name": filename,
                "url": format!("/api/gallery/proxy-image/{}", encoded_filename),
                "size": size,
                "date": date,
                "tags": tags
            }));

            if filename == "aali-kali.jpeg" {
                debug!("Response JSON for aali-kali.jpeg: {:?}", images.last().unwrap());
            }
        }
    }

    Ok(HttpResponse::Ok().json(json!({ "images": images })))
}

/// Parse binary plist tags from the hex string in macos_attributes.xattr
fn parse_binary_plist_tags(hex_str: &str) -> Vec<String> {
    debug!("Parsing tags from hex string: {}", hex_str);
    // Split by newlines, trim each line, and join
    let hex_str = hex_str
        .split('\n')
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("");
    // Remove spaces
    let hex_str = hex_str.replace(" ", "");
    debug!("Cleaned hex string: {}", hex_str);
    match hex::decode(&hex_str) {
        Ok(bytes) => {
            debug!("Decoded {} bytes", bytes.len());
            // Create a temporary file and write the bytes
            match NamedTempFile::new() {
                Ok(mut temp_file) => {
                    if let Err(e) = temp_file.write_all(&bytes) {
                        error!("Error writing to temp file: {}", e);
                        return Vec::new();
                    }
                    
                    match plist::Value::from_file(temp_file.path()) {
                        Ok(plist::Value::Array(items)) => {
                            debug!("Found array with {} items", items.len());
                            items.into_iter()
                                .filter_map(|item| {
                                    if let plist::Value::String(s) = item {
                                        debug!("Found raw tag: {}", s);
                                        // Extract just the color name from the tag
                                        let color = s.chars()
                                            .take_while(|c| c.is_alphabetic())
                                            .collect::<String>()
                                            .to_lowercase();
                                        
                                        // Only return if it's a valid macOS tag color
                                        match color.as_str() {
                                            "red" | "orange" | "yellow" | "green" | 
                                            "blue" | "purple" | "gray" | "grey" => Some(color),
                                            _ => None
                                        }
                                    } else {
                                        debug!("Found non-string item: {:?}", item);
                                        None
                                    }
                                })
                                .collect()
                        },
                        Ok(other) => {
                            debug!("Found non-array value: {:?}", other);
                            Vec::new()
                        },
                        Err(e) => {
                            error!("Error parsing plist: {}", e);
                            Vec::new()
                        }
                    }
                },
                Err(e) => {
                    error!("Error creating temp file: {}", e);
                    Vec::new()
                }
            }
        },
        Err(e) => {
            error!("Error decoding hex string: {}", e);
            Vec::new()
        }
    }
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

    let file = match tokio_fs::File::open(&path).await {
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
        name: filename.to_string(),
        path: format!("/api/gallery/proxy-image/{}", 
            percent_decode_str(&filename).decode_utf8().unwrap_or_else(|_| Cow::Owned(filename.to_string()))),
        size: 0,
        modified_date: Utc::now().to_rfc3339(),
        dimensions: None,
        kind: None,
        tags: vec![],
    };
    HttpResponse::Ok().json(metadata)
}

/// Image content search handler
/// 
/// This endpoint searches for content (movies, archives, folders) related to an image name
/// using the macOS Finder API.
#[get("/image-content")]
pub async fn search_image_content(
    _req: HttpRequest,
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

#[get("/image-content/{filename}")]
pub async fn view_content(req: HttpRequest, name: web::Path<String>) -> impl Responder {
    let filename = name.into_inner();
    let path = PathBuf::from("/Volumes/VideosNew/Models").join(&filename);

    let file = match tokio_fs::File::open(&path).await {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open file: {}", e);
            return HttpResponse::NotFound().finish();
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
