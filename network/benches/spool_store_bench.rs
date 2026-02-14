use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::{distributions::Alphanumeric, Rng};
use solana_sdk::pubkey::Pubkey;
use spool_api::prelude::*;
use spool_network::store::*;
use tempdir::TempDir;

const SEGMENTS_PER_SPOOL: u64 = 1000;
const NUM_SPOOLS: usize = 10;

fn generate_random_data(size: usize) -> Vec<u8> {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(size)
        .collect()
}

fn bench_put_segment(c: &mut Criterion) {
    let temp_dir = TempDir::new("bench_put_segment").unwrap();
    let store = SpoolStore::new(temp_dir.path()).unwrap();

    let mut group = c.benchmark_group("put_segment");
    group.bench_function("put_segment", |b| {
        let spool_address = Pubkey::new_unique();
        let global_seg_idx = 0;
        let data = generate_random_data(PACKED_SEGMENT_SIZE);

        b.iter(|| {
            store
                .put_segment(
                    black_box(&spool_address),
                    black_box(global_seg_idx),
                    black_box(data.clone()),
                )
                .unwrap();
        })
    });
    group.finish();
}

fn bench_put_spool(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_spool");

    group.bench_function("put_spool_with_segments", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new("bench_put_spool").unwrap();
            let store = SpoolStore::new(temp_dir.path()).unwrap();
            let spool_address = Pubkey::new_unique();
            let spool_number = 1;

            for global_seg_idx in 0..SEGMENTS_PER_SPOOL {
                let data = generate_random_data(PACKED_SEGMENT_SIZE);
                store
                    .put_segment(&spool_address, global_seg_idx, data)
                    .unwrap();
            }

            store
                .put_spool_address(black_box(spool_number), black_box(&spool_address))
                .unwrap();
        })
    });
    group.finish();
}

fn bench_put_many_spools(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_many_spools");

    group.bench_function("put_many_spools", |b| {
        b.iter(|| {
            let temp_dir = TempDir::new("bench_put_many").unwrap();
            let store = SpoolStore::new(temp_dir.path()).unwrap();

            for spool_idx in 0..NUM_SPOOLS {
                let spool_address = Pubkey::new_unique();
                let spool_number = (spool_idx + 1) as u64;

                for global_seg_idx in 0..SEGMENTS_PER_SPOOL {
                    let data = generate_random_data(PACKED_SEGMENT_SIZE);
                    store
                        .put_segment(&spool_address, global_seg_idx, data)
                        .unwrap();
                }

                store
                    .put_spool_address(black_box(spool_number), black_box(&spool_address))
                    .unwrap();
            }
        })
    });
    group.finish();
}

fn bench_get_segment(c: &mut Criterion) {
    let temp_dir = TempDir::new("bench_get_segment").unwrap();
    let store = SpoolStore::new(temp_dir.path()).unwrap();

    let mut spool_addresses = Vec::with_capacity(NUM_SPOOLS);
    for spool_idx in 0..NUM_SPOOLS {
        let spool_address = Pubkey::new_unique();
        let spool_number = (spool_idx + 1) as u64;
        spool_addresses.push(spool_address);

        for global_seg_idx in 0..SEGMENTS_PER_SPOOL {
            let data = generate_random_data(PACKED_SEGMENT_SIZE);
            store
                .put_segment(&spool_address, global_seg_idx, data)
                .unwrap();
        }
        store.put_spool_address(spool_number, &spool_address).unwrap();
    }

    let mut group = c.benchmark_group("get_segment");
    group.bench_function("get_segment_many_spools", |b| {
        let spool_address = spool_addresses[NUM_SPOOLS / 2];
        let global_seg_idx = SEGMENTS_PER_SPOOL / 2;

        b.iter(|| {
            store
                .get_segment(black_box(&spool_address), black_box(global_seg_idx))
                .unwrap();
        })
    });
    group.finish();
}


fn customized_criterion() -> Criterion {
    Criterion::default().sample_size(20)
}

criterion_group! {
    name = benches;
    config = customized_criterion();
    targets = 
        bench_put_segment,
        bench_put_spool,
        bench_put_many_spools,
        bench_get_segment,
}

criterion_main!(benches);
