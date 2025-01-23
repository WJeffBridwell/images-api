use actix_web::{web, App, HttpServer};
use std::path::PathBuf;
use mongodb::{Client, Database};
use crate::handlers::*;

pub async fn run(images_dir: PathBuf) -> std::io::Result<actix_web::dev::Server> {
    let images_dir = web::Data::new(images_dir);
    
    // Connect to MongoDB
    let client = Client::with_uri_str("mongodb://localhost:27017")
        .await
        .expect("Failed to connect to MongoDB");
    let db = client.database("media");
    let db = web::Data::new(db);
    
    let server = HttpServer::new(move || {
        App::new()
            .app_data(images_dir.clone())
            .app_data(db.clone())
            .configure(init_routes)
    })
    .bind(("0.0.0.0", 8081))?
    .run();
    
    Ok(server)
}
