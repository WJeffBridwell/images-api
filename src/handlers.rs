use actix_web::{get, web, HttpResponse, Responder};
use chrono::Utc;
use image::{GenericImageView, guess_format};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub version: String,
}

#[derive(Serialize)]
pub struct ImageInfo {
    pub filename: String,
    pub size_bytes: u64,
    pub format: Option<String>,
    pub dimensions: Option<(u32, u32)>,
}

#[get("/health")]
pub async fn health_check() -> impl Responder {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    HttpResponse::Ok().json(response)
}

#[get("/images/{filename}")]
pub async fn serve_image(
    filename: web::Path<String>,
    images_dir: web::Data<PathBuf>,
) -> impl Responder {
    let path = images_dir.join(filename.as_ref());
    
    if !path.exists() {
        return HttpResponse::NotFound().body("Image not found");
    }

    match std::fs::read(&path) {
        Ok(contents) => HttpResponse::Ok()
            .content_type("image/jpeg") // You might want to make this dynamic based on the file type
            .body(contents),
        Err(_) => HttpResponse::InternalServerError().body("Failed to read image"),
    }
}

#[get("/images/{filename}/info")]
pub async fn image_info(
    filename: web::Path<String>,
    images_dir: web::Data<PathBuf>,
) -> impl Responder {
    let path = images_dir.join(filename.as_ref());
    
    if !path.exists() {
        return HttpResponse::NotFound().body("Image not found");
    }

    let metadata = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to read image metadata"),
    };

    let format = guess_format(&std::fs::read(&path).unwrap_or_default()).ok();
    let dimensions = image::open(&path).ok().map(|img| img.dimensions());

    let info = ImageInfo {
        filename: filename.to_string(),
        size_bytes: metadata.len(),
        format: format.map(|f| format!("{:?}", f)),
        dimensions,
    };

    HttpResponse::Ok().json(info)
}
