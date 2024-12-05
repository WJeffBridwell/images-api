use criterion::{criterion_group, criterion_main, Criterion};
use actix_web::{test, App};
use images_api::handlers;

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

fn health_check_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("api_operations");
    group.sample_size(10);
    
    group.bench_function("health_check", |b| {
        b.iter(|| {
            rt.block_on(benchmark_health_check());
        });
    });
    
    group.finish();
}

criterion_group!(benches, health_check_benchmark);
criterion_main!(benches);
