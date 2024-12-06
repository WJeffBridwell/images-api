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

use actix_web::{web, App, HttpServer};
use env_logger;
use std::env;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use images_api::image_processor::ImageProcessor;
use images_api::handlers::*;

type ImageCache = HashMap<String, Vec<u8>>;

/// Configures and initializes the web application
/// 
/// Sets up:
/// - Route handlers
/// - Shared state (ImageProcessor)
/// - CORS and logging middleware
fn init_app(
    cfg: &mut web::ServiceConfig
) {
    cfg.service(handlers::health_check)
       .service(handlers::list_images)
       .service(handlers::serve_image)
       .service(handlers::image_info);
}

/// Application entry point
/// 
/// Initializes:
/// - Environment variables
/// - Logging
/// - Web server with configured routes
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let images_dir = std::env::var("IMAGES_DIR")
        .unwrap_or_else(|_| "./images".to_string());
    let images_dir = std::path::PathBuf::from(images_dir);

    // Create images directory if it doesn't exist
    if !images_dir.exists() {
        std::fs::create_dir_all(&images_dir)?;
    }

    let processor = web::Data::new(ImageProcessor::new());
    let image_cache = web::Data::new(Arc::new(RwLock::new(ImageCache::new())));
    let images_dir = web::Data::new(images_dir);

    HttpServer::new(move || {
        App::new()
            .app_data(processor.clone())
            .app_data(image_cache.clone())
            .app_data(images_dir.clone())
            .configure(init_app)
    })
    .bind("127.0.0.1:8081")?
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
        let image_cache = web::Data::new(Arc::new(RwLock::new(ImageCache::new())));
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
