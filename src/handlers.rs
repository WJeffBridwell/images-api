use actix_web::{get, web, HttpResponse, Responder, HttpRequest};
use chrono::Utc;
use log::error;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::process::Command;
use crate::image_processor::ImageProcessor;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: String,
    timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ImageQueryParams {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageMetadata {
    pub filename: String,
    pub path: String,
    pub size: u64,
    pub last_modified: SystemTime,
    pub dimensions: Option<(u32, u32)>,
}

pub struct ImageCache {
    pub metadata: HashMap<String, ImageMetadata>,
    pub last_updated: SystemTime,
}

impl ImageCache {
    pub fn new() -> Self {
        ImageCache {
            metadata: HashMap::new(),
            last_updated: SystemTime::now(),
        }
    }

    pub fn populate(&mut self, images_dir: &PathBuf) -> std::io::Result<()> {
        self.metadata.clear();
        for entry in std::fs::read_dir(images_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        if ["jpg", "jpeg", "png", "gif"].contains(&ext_str.to_lowercase().as_str()) {
                            let metadata = entry.metadata()?;
                            let filename = path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                            
                            self.metadata.insert(filename.clone(), ImageMetadata {
                                filename,
                                path: path.to_string_lossy().into_owned(),
                                size: metadata.len(),
                                last_modified: metadata.modified()?,
                                dimensions: None, // We'll add this later if needed
                            });
                        }
                    }
                }
            }
        }
        self.last_updated = SystemTime::now();
        Ok(())
    }
}

#[get("/health")]
pub async fn health_check(_req: HttpRequest) -> impl Responder {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
    };
    HttpResponse::Ok().json(response)
}

#[get("/images/{filename}")]
pub async fn serve_image(
    _req: HttpRequest,
    filename: web::Path<String>,
    images_dir: web::Data<PathBuf>,
) -> impl Responder {
    let path = images_dir.join(&*filename);
    
    if !path.exists() {
        return HttpResponse::NotFound().json("Image not found");
    }

    // Check file extension instead of using file command
    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase());

    let is_image = matches!(extension.as_deref(), 
        Some("jpg") | Some("jpeg") | Some("png") | Some("gif") | Some("webp"));

    if !is_image {
        return HttpResponse::BadRequest().json("Not an image file");
    }

    match std::fs::read(&path) {
        Ok(contents) => {
            let content_type = match extension.as_deref() {
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("png") => "image/png",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                _ => "application/octet-stream",
            };
            HttpResponse::Ok()
                .content_type(content_type)
                .body(contents)
        },
        Err(_) => HttpResponse::InternalServerError().json("Failed to read image file"),
    }
}

#[get("/images/{filename}/info")]
pub async fn image_info(
    _req: HttpRequest,
    filename: web::Path<String>,
    images_dir: web::Data<PathBuf>,
    processor: web::Data<ImageProcessor>,
) -> impl Responder {
    let path = images_dir.join(&*filename);
    
    if !path.exists() {
        return HttpResponse::NotFound().json("Image not found");
    }

    match processor.get_image_data(&path, false).await {
        Ok(image_data) => {
            let metadata = ImageMetadata {
                filename: filename.to_string(),
                path: path.to_string_lossy().to_string(),
                size: image_data.size_bytes as u64,
                last_modified: SystemTime::now(),
                dimensions: Some(image_data.dimensions),
            };
            HttpResponse::Ok().json(metadata)
        },
        Err(e) => {
            error!("Failed to get image info: {}", e);
            HttpResponse::InternalServerError().json("Failed to get image info")
        }
    }
}

#[get("/images")]
pub async fn list_images(
    _req: HttpRequest,
    query: web::Query<ImageQueryParams>,
    _processor: web::Data<ImageProcessor>,
    _images_dir: web::Data<PathBuf>,
    image_cache: web::Data<Arc<RwLock<ImageCache>>>,
) -> impl Responder {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(10);
    
    let cache = image_cache.read().unwrap();
    let mut images: Vec<_> = cache.metadata.values().cloned().collect();

    // Sort images if requested
    if let Some(sort_by) = &query.sort_by {
        match sort_by.as_str() {
            "name" => {
                images.sort_by(|a, b| a.filename.cmp(&b.filename));
            }
            "date" => {
                images.sort_by(|a, b| a.last_modified.cmp(&b.last_modified));
            }
            "size" => {
                images.sort_by(|a, b| a.size.cmp(&b.size));
            }
            _ => {}
        }

        // Apply order if specified
        if let Some(order) = &query.order {
            if order == "desc" {
                images.reverse();
            }
        }
    }

    // Apply pagination
    let start = (page - 1) * per_page;
    let paginated_images: Vec<_> = images
        .into_iter()
        .skip(start)
        .take(per_page)
        .collect();

    HttpResponse::Ok().json(paginated_images)
}

fn get_macos_tags(path: &std::path::Path) -> Vec<String> {
    match Command::new("mdls")
        .arg("-raw")
        .arg("-name")
        .arg("kMDItemUserTags")
        .arg(path)
        .output()
    {
        Ok(output) => {
            String::from_utf8_lossy(&output.stdout)
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        Err(_) => Vec::new()
    }
}
