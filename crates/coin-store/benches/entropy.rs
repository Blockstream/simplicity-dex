use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::fs;
use tokio::runtime::Runtime;

use coin_store::executor::UtxoStore;

mod common;

fn criterion_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (store, filters, db_path) = rt.block_on(async { common::setup_db().await.expect("DB Setup failed") });

    let mut group = c.benchmark_group("UTXO Queries (with entropy)");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));

    group.bench_function("current_implementation", |b| {
        b.to_async(&rt).iter(|| async {
            store.query_utxos(black_box(&filters.1)).await.unwrap();
        })
    });

    group.finish();

    let _ = fs::remove_file(db_path);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
