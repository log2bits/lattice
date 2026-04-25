use criterion::{criterion_group, criterion_main, Criterion};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use radsort::sort_by_key;

use lattice::tree::EditPacket;
use lattice::types::BitpackedArray;

const DEPTH: usize = 28;
const N: usize = 50_000;

struct Data {
    paths: Vec<[u8; DEPTH]>,
    levels: Vec<u8>,
    values: Vec<u32>,
}

fn generate() -> Data {
    let mut rng = StdRng::seed_from_u64(42);

    let mut paths = Vec::with_capacity(N);
    let mut levels = Vec::with_capacity(N);
    let mut values = Vec::with_capacity(N);

    for _ in 0..N {
        let mut path = [0u8; DEPTH];
        for d in 0..DEPTH {
            path[d] = rng.gen_range(0..64);
        }

        paths.push(path);
        levels.push(rng.gen_range(0..DEPTH as u8));
        values.push(rng.r#gen::<u32>());
    }

    Data { paths, levels, values }
}

fn bench_sort(c: &mut Criterion) {
    let data = generate();

    c.bench_function("custom_radix", |b| {
        b.iter(|| {
            let mut packet = EditPacket::<DEPTH> {
                paths: data.paths.clone(),
                levels: data.levels.clone(),
                lut: Default::default(),
                values: {
                    let mut v = BitpackedArray::new();
                    for &val in &data.values {
                        v.push(val);
                    }
                    v
                },
                sorted: false,
            };

            packet.sort();
        });
    });

    c.bench_function("std_sort", |b| {
        b.iter(|| {
            let mut idx: Vec<usize> = (0..data.paths.len()).collect();

            idx.sort_unstable_by(|&a, &b| {
                data.paths[a]
                    .cmp(&data.paths[b])
                    .then_with(|| data.levels[b].cmp(&data.levels[a]))
            });

            idx
        });
    });

    c.bench_function("radsort_partial", |b| {
        b.iter(|| {
            const PACK_DEPTH: usize = 20;

            let mut packed: Vec<([u8; DEPTH], u8, u32)> =
                Vec::with_capacity(data.paths.len());

            for i in 0..data.paths.len() {
                packed.push((
                    data.paths[i],
                    data.levels[i],
                    data.values[i],
                ));
            }

            sort_by_key(&mut packed, |(path, level, _)| {
                let mut key: u128 = 0;

                for &b in &path[..PACK_DEPTH] {
                    key = (key << 6) | b as u128;
                }

                key = (key << 8) | (255 - *level) as u128;

                key
            });

            packed
        });
    });
}

criterion_group!(benches, bench_sort);
criterion_main!(benches);