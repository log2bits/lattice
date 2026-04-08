// Benchmark and stress test for BitpackedArray.
// Run with: cargo run --bin bench_bitpacked --release

use lattice::lattice::BitpackedArray;
use std::hint::black_box;
use std::time::Instant;

fn bench_push_uniform(bits: u8, count: u32) -> f64 {
	let max_val = if bits == 32 { u32::MAX } else { (1u32 << bits) - 1 };
	let mut arr = BitpackedArray::new();

	let t = Instant::now();
	for i in 0..count {
		arr.push(black_box(i % (max_val + 1).max(1)));
	}
	let elapsed = t.elapsed();
	black_box(&arr);

	count as f64 / elapsed.as_secs_f64()
}

fn bench_get(arr: &BitpackedArray) -> f64 {
	let t = Instant::now();
	let mut sum = 0u64;
	for i in 0..arr.len() {
		sum += black_box(arr.get(i)) as u64;
	}
	let elapsed = t.elapsed();
	black_box(sum);

	arr.len() as f64 / elapsed.as_secs_f64()
}

// Measures how long each repack takes by pushing values that force
// the array through each width transition.
fn bench_repacks() {
	let transitions: &[(u32, u8, u8)] = &[
		(2, 1, 2),       // 1 -> 2: push value 2
		(4, 2, 4),       // 2 -> 4: push value 4
		(16, 4, 8),      // 4 -> 8: push value 16
		(256, 8, 16),    // 8 -> 16: push value 256
		(65536, 16, 32), // 16 -> 32: push value 65536
	];

	println!("\nrepack timings (filling array first, then triggering repack):");
	println!("{:<12} {:>10} {:>10} {:>12}", "transition", "entries", "repack ns", "ns/entry");

	for &(trigger_val, old_bits, new_bits) in transitions {
		// Fill array to ~1M entries at old_bits width
		let count = 1_000_000u32;
		let fill_val = (1u32 << old_bits) - 1;
		let mut arr = BitpackedArray::new();
		for _ in 0..count {
			arr.push(fill_val);
		}
		assert_eq!(arr.bits, old_bits);

		// Time the repack by pushing the triggering value
		let t = Instant::now();
		arr.push(black_box(trigger_val));
		let ns = t.elapsed().as_nanos();
		black_box(&arr);

		println!(
			"{:<12} {:>10} {:>10} {:>12.1}",
			format!("{}->{}b", old_bits, new_bits),
			count,
			ns,
			ns as f64 / count as f64,
		);
	}
}

fn bench_stress() {
	println!("\nstress test: push 10M mixed values, verify correctness");
	use rand::rngs::SmallRng;
	use rand::{Rng, SeedableRng};

	let mut rng = SmallRng::seed_from_u64(42);
	let count = 10_000_000u32;
	let mut arr = BitpackedArray::new();
	let mut expected = Vec::with_capacity(count as usize);

	let t = Instant::now();
	for _ in 0..count {
		let v: u32 = rng.r#gen();
		expected.push(v);
		arr.push(v);
	}
	let push_elapsed = t.elapsed();

	assert_eq!(arr.bits, 32, "expected bits=32 after full u32 range");

	let t = Instant::now();
	for i in 0..count {
		assert_eq!(arr.get(i), expected[i as usize], "mismatch at index {i}");
	}
	let get_elapsed = t.elapsed();

	println!("  final bits: {}", arr.bits);
	println!("  push: {:.1}M/s", count as f64 / push_elapsed.as_secs_f64() / 1e6);
	println!("  get:  {:.1}M/s", count as f64 / get_elapsed.as_secs_f64() / 1e6);
	println!("  all {} values verified correct", count);
}

fn main() {
	println!("=== BitpackedArray benchmark ===\n");

	println!("push throughput (entries/sec) by final bit width:");
	println!("{:<8} {:>12} {:>12}", "bits", "count", "entries/sec");

	let configs: &[(u8, u32)] = &[
		(1, 10_000_000),
		(2, 10_000_000),
		(4, 10_000_000),
		(8, 10_000_000),
		(16, 10_000_000),
		(32, 10_000_000),
	];

	for &(bits, count) in configs {
		let eps = bench_push_uniform(bits, count);
		println!("{:<8} {:>12} {:>12.0}", bits, count, eps);
	}

	println!("\nget throughput at each bit width:");
	println!("{:<8} {:>12} {:>12}", "bits", "count", "entries/sec");

	for &(bits, count) in configs {
		let max_val = if bits == 32 { u32::MAX } else { (1u32 << bits) - 1 };
		let mut arr = BitpackedArray::new();
		for i in 0..count {
			arr.push(i % (max_val + 1).max(1));
		}
		let eps = bench_get(&arr);
		println!("{:<8} {:>12} {:>12.0}", bits, count, eps);
	}

	bench_repacks();
	bench_stress();
}
