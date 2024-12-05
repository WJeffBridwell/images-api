use actix_web::{get, web, HttpResponse, Result};
use std::path::{Path, PathBuf};
use anyhow::Context;
use log::info;
use images_api::startup;
use serde::Serialize;
use chrono;
use image::{io, GenericImageView};

#[derive(Serialize)]
struct ImageInfo {
    filename: String,
    dimensions: (u32, u32),
    size_bytes: u64,
    last_modified: String,
}

async fn get_image_info(path: &Path) -> anyhow::Result<ImageInfo> {
    let reader = io::Reader::open(path)
        .context("Failed to open image")?;
    let img = reader.decode().context("Failed to decode image")?;
    let metadata = path.metadata().context("Failed to get metadata")?;
    
    Ok(ImageInfo {
        filename: path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        dimensions: img.dimensions(),
        size_bytes: metadata.len(),
        last_modified: metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
            .flatten()
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

#[get("/images/{filename}")]
async fn serve_image(filename: web::Path<String>) -> Result<HttpResponse> {
    let image_path = PathBuf::from("images").join(filename.as_ref());
    
    if !image_path.exists() {
        return Ok(HttpResponse::NotFound().body("Image not found"));
    }

    let content = std::fs::read(&image_path)
        .map_err(|e| {
            log::error!("Failed to read image: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to read image")
        })?;

    let content_type = match image_path.extension().and_then(|ext| ext.to_str()) {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    };

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(content))
}

#[get("/images/{filename}/info")]
async fn image_info(filename: web::Path<String>) -> Result<HttpResponse> {
    let image_path = PathBuf::from("images").join(filename.as_ref());
    
    if !image_path.exists() {
        return Ok(HttpResponse::NotFound().body("Image not found"));
    }

    let info = get_image_info(&image_path)
        .await
        .map_err(|e| {
            log::error!("Failed to get image info: {}", e);
            actix_web::error::ErrorInternalServerError("Failed to get image info")
        })?;

    Ok(HttpResponse::Ok().json(info))
}

#[get("/health")]
async fn health_check() -> Result<HttpResponse> {
    info!("Health check endpoint called");
    let status = serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    });
    info!("Returning health status: {:?}", status);
    Ok(HttpResponse::Ok().json(status))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
    // Create images directory if it doesn't exist
    std::fs::create_dir_all("images")?;
    
    let images_dir = PathBuf::from("images");
    info!("Starting server with images directory: {:?}", images_dir);
    let server = startup::run(images_dir).await?;
    
    server.await
}
