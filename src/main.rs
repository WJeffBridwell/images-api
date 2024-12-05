use actix_web::{web, App, HttpServer};
use log::{info, error};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use serde::Serialize;

mod handlers;
mod image_processor;

#[derive(Serialize, Clone, Default)]
pub struct Config {
    pub images_dir: PathBuf,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let images_dir = PathBuf::from("/Volumes/VideosNew/Models");
    if !images_dir.exists() {
        error!("Images directory does not exist: {:?}", images_dir);
        return Ok(());
    }

    let config = Config {
        images_dir: images_dir.clone(),
    };

    let images_dir_data = web::Data::new(images_dir.clone());
    let image_cache = web::Data::new(Arc::new(RwLock::new(handlers::ImageCache::new())));
    let image_processor = web::Data::new(image_processor::ImageProcessor::new());

    // Populate the cache initially
    if let Ok(mut cache) = image_cache.write() {
        if let Err(e) = cache.populate(&images_dir) {
            error!("Failed to populate image cache: {}", e);
        }
    }

    info!("Starting server on http://127.0.0.1:8081");
    
    HttpServer::new(move || {
        App::new()
            .app_data(images_dir_data.clone())
            .app_data(image_cache.clone())
            .app_data(image_processor.clone())
            .service(handlers::health_check)
            .service(handlers::serve_image)
            .service(handlers::image_info)
            .service(handlers::list_images)
    })
    .bind("127.0.0.1:8081")?
    .run()
    .await
}
