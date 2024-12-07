/*! 
 * Images API - Main Application Entry Point
 * 
 * This module serves as the entry point for the Images API service. It handles:
 * - Application configuration and setup
 * - Server initialization and startup
 * - Route registration
 * - Middleware configuration
 * 
 * The service provides endpoints for:
 * - Image retrieval and serving
 * - Image metadata information
 * - Health checks
 */

use actix_web::{middleware::Logger, web, App, HttpServer};
use actix_cors::Cors;
use actix_files as fs;
use env_logger::Env;
use log;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use images_api::handlers;
use images_api::image_processor::ImageProcessor;

/// Cache type for storing image data
pub type ImageCache = HashMap<String, Vec<u8>>;

/// Application entry point
/// 
/// Initializes:
/// - Environment variables
/// - Logging
/// - Web server with configured routes
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("debug"));

    log::debug!("Starting Images API service");

    // Create images directory if it doesn't exist
    let images_dir = std::env::var("IMAGES_DIR").unwrap_or_else(|_| "./images".to_string());
    let images_dir = std::path::PathBuf::from(images_dir);
    if !images_dir.exists() {
        std::fs::create_dir_all(&images_dir)?;
    }

    let processor = web::Data::new(ImageProcessor::new());
    let image_cache = web::Data::new(Arc::new(RwLock::new(ImageCache::new())));
    let images_dir = web::Data::new(images_dir);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(processor.clone())
            .app_data(image_cache.clone())
            .app_data(images_dir.clone())
            .wrap(Logger::default())
            .wrap(cors)
            .service(fs::Files::new("/static", "static").show_files_listing())
            .configure(handlers::init_routes)
    })
    .bind(("127.0.0.1", 8081))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_app_configuration() {
        let processor = web::Data::new(ImageProcessor::new());
        let image_cache = web::Data::new(Arc::new(RwLock::new(HashMap::<String, Vec<u8>>::new())));
        let images_dir = web::Data::new(std::path::PathBuf::from("./test_images"));

        let app = App::new()
            .app_data(processor)
            .app_data(image_cache)
            .app_data(images_dir)
            .service(handlers::health_check);

        let app = test::init_service(app).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}
