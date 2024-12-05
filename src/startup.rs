use actix_web::{web, App, HttpServer};
use std::path::PathBuf;
use crate::handlers::*;

pub async fn run(images_dir: PathBuf) -> std::io::Result<actix_web::dev::Server> {
    let images_dir = web::Data::new(images_dir);
    
    let server = HttpServer::new(move || {
        App::new()
            .app_data(images_dir.clone())
            .service(health_check)
            .service(list_images)
            .service(serve_image)
            .service(image_info)
    })
    .bind(("127.0.0.1", 8081))?
    .run();
    
    Ok(server)
}
