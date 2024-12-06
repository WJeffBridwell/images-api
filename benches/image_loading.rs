use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;
use actix_web::{test, web, App};
use images_api::handlers::*;
use images_api::image_processor::ImageProcessor;
use std::path::PathBuf;

async fn fetch_images(page: usize, include_thumbnails: bool, include_data: bool) {
    let processor = web::Data::new(ImageProcessor::new());
    let app = test::init_service(
        App::new()
            .app_data(processor.clone())
            .service(list_images)
    ).await;

    let query = format!(
        "/images?page={}&per_page=100&include_thumbnail={}&include_data={}",
        page,
        include_thumbnails,
        include_data
    );

    let req = test::TestRequest::get().uri(&query).to_request();
    let _resp = test::call_service(&app, req).await;
}

async fn stream_image(image_size: &str) {
    let images_dir = web::Data::new(PathBuf::from("/Volumes/VideosNew/Models"));
    let app = test::init_service(
        App::new()
            .app_data(images_dir.clone())
            .service(serve_image)
    ).await;

    // Select image based on size category
    let image_path = match image_size {
        "small" => "/images/eva-p.jpg",  // Known small image
        "medium" => "/images/medium-sample.jpg",  // Add a medium-sized image
        "large" => "/images/large-sample.jpg",    // Add a large image
        _ => "/images/eva-p.jpg"
    };

    let req = test::TestRequest::get()
        .uri(image_path)
        .to_request();
        
    let mut resp = test::call_service(&app, req).await;
    let _bytes = test::read_body(resp).await;
}

pub fn image_loading_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("image_loading");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(20);

    // Test listing without images
    group.bench_function("list_only", |b| {
        b.iter(|| {
            runtime.block_on(async {
                fetch_images(black_box(1), false, false).await
            })
        });
    });

    // Test with thumbnails
    group.bench_function("with_thumbnails", |b| {
        b.iter(|| {
            runtime.block_on(async {
                fetch_images(black_box(1), true, false).await
            })
        });
    });

    // Test streaming performance for different image sizes
    group.bench_function("stream_small_image", |b| {
        b.iter(|| {
            runtime.block_on(async {
                stream_image("small").await
            })
        });
    });

    group.bench_function("stream_medium_image", |b| {
        b.iter(|| {
            runtime.block_on(async {
                stream_image("medium").await
            })
        });
    });

    group.bench_function("stream_large_image", |b| {
        b.iter(|| {
            runtime.block_on(async {
                stream_image("large").await
            })
        });
    });

    group.finish();
}

criterion_group!(benches, image_loading_benchmark);
criterion_main!(benches);
