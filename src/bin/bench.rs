use lattice::{shapes, tree::World, voxel::Voxel};
use std::time::{Duration, Instant};

// ── configuration ────────────────────────────────────────────────────────────
const TARGET_FPS: u32 = 240;
const FRAME_BUDGET: Duration = Duration::from_nanos(1_000_000_000 / TARGET_FPS as u64);

const WORLD: [u32; 3] = [16, 16, 16];
const DEPTH: u8 = 4;
const CENTER: [u32; 3] = [2048, 2048, 2048]; // center of 8×256=2048 voxel world
const WORLD_SIDE: u32 = 4096; // 8 chunks × 256 voxels/chunk

fn main() {
	if std::env::var("PROFILE").as_deref() == Ok("debug") {
		eprintln!("warning: run with --release for meaningful numbers\n");
	}

	let stone = Voxel::from_rgb_flags([120, 110, 100], 15, false, false, false);
	let red = Voxel::from_rgb_flags([220, 60, 60], 15, false, false, false);

	println!(
		"world: {WORLD:?} chunks  depth={DEPTH}  chunk=256³  world={}³  center={CENTER:?}",
		WORLD_SIDE
	);
	println!(
		"target: {TARGET_FPS} fps  ({:.2}ms/frame)\n",
		FRAME_BUDGET.as_secs_f64() * 1000.0
	);

	let mut world = World::new(WORLD, DEPTH);

	bench_spheres(&mut world, stone);
	println!();
	bench_voxels(&mut world, red);
}

// Concentric sphere fills from r=1 to r=1024 (powers of 2), applied cumulatively.
// Each sphere overwrites the previous, so memory reflects the current state.
fn bench_spheres(world: &mut World, fill: Voxel) {
	println!("--- sphere fills (concentric, center={CENTER:?}) ---");
	println!(
		"{:>6}  {:>13}  {:>7}  {:>9}  {:>6}  {:>9}  {:>8}",
		"r", "~voxels", "chunks", "flush", "frames", "memory", "B/voxel"
	);

	let radii = std::iter::successors(Some(1u32), |&r| (r < 2048).then_some(r * 2));
	for r in radii {
		world.queue_fill_shape(shapes::sphere(CENTER, r), fill);
		let t = Instant::now();
		world.flush_edits();
		let elapsed = t.elapsed();

		let frames = est_frames(elapsed);
		let chunks = world.chunks.len();
		let bytes: usize = world.chunks.iter().map(|c| c.memory_bytes()).sum();
		let voxels = approx_sphere_voxels(r).max(1);

		println!(
			"{:>6}  {:>13}  {:>7}  {:>9}  {:>6}  {:>9}  {:>8.4}",
			r,
			fmt_num(voxels),
			chunks,
			fmt(elapsed),
			frames,
			fmt_bytes(bytes),
			bytes as f64 / voxels as f64
		);
	}
}

// Random single-voxel edits applied cumulatively on top of the sphere-filled world.
// Each row queues the incremental batch and times only that flush.
fn bench_voxels(world: &mut World, fill: Voxel) {
	println!("--- voxel edits (random positions in world, cumulative after sphere fills) ---");
	println!(
		"{:>7}  {:>9}  {:>6}  {:>9}  {:>7}",
		"edits", "flush", "frames", "memory", "chunks"
	);

	let mut rng = Lcg::new(0xdeadbeef);
	let mut prev = 0usize;
	let mut positions = Vec::new();

	for &count in &[1usize, 10, 100, 1000, 10000] {
		for _ in 0..count - prev {
			let pos = rand_pos(&mut rng, WORLD_SIDE);
			world.set_voxel(pos, fill);
			positions.push(pos);
		}
		prev = count;

		let t = Instant::now();
		world.flush_edits();
		let elapsed = t.elapsed();

		let bytes: usize = world.chunks.iter().map(|c| c.memory_bytes()).sum();
		let chunks: usize = world.chunks.len();

		// Spot-check that the last few written positions read back correctly.
		let wrong = positions
			.iter()
			.rev()
			.take(10)
			.filter(|&&p| world.get_voxel(p) != Some(fill))
			.count();

		let integrity = if wrong == 0 {
			String::new()
		} else {
			format!("  !! {wrong} readback failures")
		};

		println!(
			"{:>7}  {:>9}  {:>6}  {:>9}  {:>7}{integrity}",
			fmt_num(count as u64),
			fmt(elapsed),
			est_frames(elapsed),
			fmt_bytes(bytes),
			chunks
		);
	}
}

// ── helpers ──────────────────────────────────────────────────────────────────

// Estimated frames the flush would cost if run synchronously on the game thread.
fn est_frames(t: Duration) -> u32 {
	((t.as_secs_f64() / FRAME_BUDGET.as_secs_f64()).ceil() as u32).max(1)
}

fn approx_sphere_voxels(r: u32) -> u64 {
	((4.0 / 3.0) * std::f64::consts::PI * (r as f64).powi(3)) as u64
}

fn rand_pos(rng: &mut Lcg, bound: u32) -> [u32; 3] {
	[rng.next() % bound, rng.next() % bound, rng.next() % bound]
}

struct Lcg(u64);
impl Lcg {
	fn new(seed: u64) -> Self {
		Self(seed)
	}
	fn next(&mut self) -> u32 {
		self.0 = self
			.0
			.wrapping_mul(6364136223846793005)
			.wrapping_add(1442695040888963407);
		(self.0 >> 33) as u32
	}
}

fn fmt_num(n: u64) -> String {
	// Insert commas for readability.
	let s = n.to_string();
	let mut out = String::new();
	for (i, c) in s.chars().rev().enumerate() {
		if i > 0 && i % 3 == 0 {
			out.push(',');
		}
		out.push(c);
	}
	out.chars().rev().collect()
}

fn fmt(d: Duration) -> String {
	let ns = d.as_secs_f64() * 1e9;
	if ns >= 1_000_000_000.0 {
		format!("{:.2}s", ns / 1e9)
	} else if ns >= 1_000_000.0 {
		format!("{:.1}ms", ns / 1e6)
	} else if ns >= 1_000.0 {
		format!("{:.1}µs", ns / 1e3)
	} else {
		format!("{:.0}ns", ns)
	}
}

fn fmt_bytes(b: usize) -> String {
	if b >= 1 << 20 {
		format!("{:.1}MB", b as f64 / (1 << 20) as f64)
	} else if b >= 1 << 10 {
		format!("{:.1}KB", b as f64 / (1 << 10) as f64)
	} else {
		format!("{b}B")
	}
}
