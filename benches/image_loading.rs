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

pub fn image_loading_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("image_loading");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

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

    // Test with full images
    group.bench_function("with_full_images", |b| {
        b.iter(|| {
            runtime.block_on(async {
                fetch_images(black_box(1), false, true).await
            })
        });
    });

    // Test with both thumbnails and images
    group.bench_function("with_both", |b| {
        b.iter(|| {
            runtime.block_on(async {
                fetch_images(black_box(1), true, true).await
            })
        });
    });

    group.finish();
}

criterion_group!(benches, image_loading_benchmark);
criterion_main!(benches);
