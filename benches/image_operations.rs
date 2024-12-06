use criterion::{criterion_group, criterion_main, Criterion};
use actix_web::{test, web, App};
use images_api::handlers;
use std::path::PathBuf;
use std::time::Duration;

async fn benchmark_health_check() {
    let app = test::init_service(
        App::new()
            .service(handlers::health_check)
    ).await;
    
    let req = test::TestRequest::get()
        .uri("/health")
        .to_request();
        
    let _resp = test::call_service(&app, req).await;
}

async fn benchmark_image_serving() {
    let images_dir = web::Data::new(PathBuf::from("/Volumes/VideosNew/Models"));
    let app = test::init_service(
        App::new()
            .app_data(images_dir.clone())
            .service(handlers::serve_image)
    ).await;
    
    let req = test::TestRequest::get()
        .uri("/images/eva-p.jpg")
        .to_request();
        
    let mut resp = test::call_service(&app, req).await;
    let _bytes = test::read_body(resp).await;
}

async fn benchmark_image_serving_large() {
    let images_dir = web::Data::new(PathBuf::from("/Volumes/VideosNew/Models"));
    let app = test::init_service(
        App::new()
            .app_data(images_dir.clone())
            .service(handlers::serve_image)
    ).await;
    
    // Use a larger image for this benchmark
    let req = test::TestRequest::get()
        .uri("/images/large-sample.jpg")  // Adjust this to a large image in your dataset
        .to_request();
        
    let mut resp = test::call_service(&app, req).await;
    let _bytes = test::read_body(resp).await;
}

fn api_operations_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("api_operations");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(20);
    
    // Health check benchmark
    group.bench_function("health_check", |b| {
        b.iter(|| {
            rt.block_on(benchmark_health_check());
        });
    });
    
    // Regular image serving benchmark
    group.bench_function("serve_image", |b| {
        b.iter(|| {
            rt.block_on(benchmark_image_serving());
        });
    });
    
    // Large image serving benchmark
    group.bench_function("serve_large_image", |b| {
        b.iter(|| {
            rt.block_on(benchmark_image_serving_large());
        });
    });
    
    group.finish();
}

criterion_group!(benches, api_operations_benchmark);
criterion_main!(benches);
