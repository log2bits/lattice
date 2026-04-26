use criterion::{Criterion, criterion_group, criterion_main};
use lattice::{
	chunk,
	shape::{Sphere, edit_packet_for_shape},
	tree::{Aabb, EditPacket, TreePath},
	types::{BitpackedArray, Lut, Voxel},
};
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;

const DEPTH: usize = chunk::DEPTH;

struct Data {
	paths: Vec<TreePath<DEPTH>>,
	values: Vec<u32>,
}

fn generate() -> Data {
	let mut rng = SmallRng::seed_from_u64(42);
	let root = Aabb {
		min: [0, 0, 0],
		max: [chunk::SIDE as i64; 3],
	};
	let sphere = Sphere {
		center: [128, 128, 128],
		radius: 100,
		material: Voxel::from_rgb_flags([95, 88, 80], 13, false, false, false, false),
	};
	let packet = edit_packet_for_shape::<DEPTH>(&sphere, root);

	let mut edits: Vec<(TreePath<DEPTH>, u32)> = (0..packet.paths.len())
		.map(|i| {
			let path = packet.paths[i];
			let value = packet.lut.get(packet.values.get(i as u32));
			(path, value)
		})
		.collect();
	edits.shuffle(&mut rng);

	Data {
		paths: edits.iter().map(|&(p, _)| p).collect(),
		values: edits.iter().map(|&(_, v)| v).collect(),
	}
}

fn make_packet(data: &Data) -> EditPacket<DEPTH> {
	let mut lut = Lut::new();
	let mut values = BitpackedArray::new();
	for &val in &data.values {
		values.push(lut.get_or_add(val));
	}
	EditPacket::<DEPTH> {
		paths: data.paths.clone(),
		lut,
		values,
		sorted: false,
	}
}

fn bench_sort(c: &mut Criterion) {
	let data = generate();

	// Our implementation.
	c.bench_function("custom_radix", |b| {
		b.iter(|| make_packet(&data).sort());
	});

	// Comparison sort on indices using TreePath's derived Ord.
	c.bench_function("std_sort", |b| {
		b.iter(|| {
			let mut idx: Vec<u32> = (0..data.paths.len() as u32).collect();
			idx.sort_unstable_by(|&a, &b| data.paths[a as usize].cmp(&data.paths[b as usize]));
			idx
		});
	});

	// Radix sort on indices: u32 keys (28 bits for DEPTH=4) vs u128 keys.
	c.bench_function("radsort_u32", |b| {
		b.iter(|| {
			let mut idx: Vec<u32> = (0..data.paths.len() as u32).collect();
			radsort::sort_by_key(&mut idx, |&i| pack_key_u32(&data.paths[i as usize]));
			idx
		});
	});

	c.bench_function("radsort_u128", |b| {
		b.iter(|| {
			let mut idx: Vec<u32> = (0..data.paths.len() as u32).collect();
			radsort::sort_by_key(&mut idx, |&i| pack_key_u128(&data.paths[i as usize]));
			idx
		});
	});
}

fn pack_key_u32(path: &TreePath<DEPTH>) -> u32 {
	let mut key = 0u32;
	for &b in path.as_bytes() {
		key = (key << 7) | b as u32;
	}
	key
}

fn pack_key_u128(path: &TreePath<DEPTH>) -> u128 {
	let mut key = 0u128;
	for &b in path.as_bytes() {
		key = (key << 7) | b as u128;
	}
	key
}

criterion_group!(benches, bench_sort);
criterion_main!(benches);
