use std::time::Instant;
use lattice::{
	chunk,
	shape::{Sphere, edit_packet_for_shape},
	tree::{Aabb, Edit, EditPacket, Tree, DELETE},
	types::Voxel,
};

const DEPTH: usize = chunk::DEPTH;
type TestTree = Tree<DEPTH>;

fn root_aabb() -> Aabb {
	Aabb { min: [0, 0, 0], max: [chunk::SIDE as i64; 3] }
}

// Center at the middle of the smallest node that fully contains the sphere,
// so the sphere doesn't straddle node boundaries at any level.
fn aligned_center(radius: i64) -> [i64; 3] {
	for &n in &[4i64, 16, 64, 256] {
		if radius <= n / 2 - 1 {
			return [n / 2; 3];
		}
	}
	[128; 3]
}

fn make_sphere(radius: i64) -> EditPacket<DEPTH> {
	let sphere = Sphere {
		center: aligned_center(radius),
		radius,
		material: Voxel::from_rgb_flags([100, 150, 200], 0, false, false, false, false),
	};
	edit_packet_for_shape::<DEPTH>(&sphere, root_aabb())
}

fn print_stats(label: &str, tree: &TestTree) {
	let mut t = tree.clone();
	t.compact();
	let leaves_actual  = t.leaf_count();
	let leaves_stored  = t.unique_leaf_count();
	let volume_actual  = t.stored_volume();
	let volume_stored  = t.unique_volume();
	let leaf_ratio     = leaves_actual as f64 / leaves_stored.max(1) as f64;
	let volume_ratio   = volume_actual as f64 / volume_stored.max(1) as f64;
	println!(
		"  {label}: {} bytes | leaves {leaves_stored} stored / {leaves_actual} actual ({leaf_ratio:.1}x) | volume {volume_stored} stored / {volume_actual} actual ({volume_ratio:.1}x)",
		t.bytes(),
	);
}

fn print_packet_levels(label: &str, packet: &EditPacket<DEPTH>) {
	let mut counts = [0u32; DEPTH + 1];
	for path in &packet.paths {
		counts[path.depth() as usize] += 1;
	}
	print!("  {label} ({} edits):", packet.paths.len());
	for (depth, &count) in counts.iter().enumerate() {
		if count > 0 {
			print!(" D{depth}={count}");
		}
	}
	println!();
}

// ── correctness tests ────────────────────────────────────────────────────────

fn assert_tree_valid(tree: &TestTree) {
	let mut t = tree.clone();
	t.compact();
	for d in 0..DEPTH {
		let level = &t.levels[d];
		for n in 0..level.node_count() {
			let occ = level.occupancy_mask[n as usize];
			let leaf = level.leaf_mask[n as usize];
			let base = level.children_offset[n as usize];
			let count = occ.count_ones();
			assert!(
				base + count <= level.node_children.len(),
				"level {d} node {n}: children out of bounds (base={base} count={count} len={})",
				level.node_children.len()
			);
			if d + 1 < DEPTH {
				let mut mask = occ & !leaf;
				while mask != 0 {
					let slot = mask.trailing_zeros() as u8;
					let rank = (occ & ((1u64 << slot) - 1)).count_ones();
					let child = level.node_children.get(base + rank);
					assert!(
						(child as usize) < t.levels[d + 1].node_count() as usize,
						"level {d} node {n} slot {slot}: child {child} out of range ({} nodes at next level)",
						t.levels[d + 1].node_count()
					);
					mask &= mask - 1;
				}
			}
		}
	}
}

fn run_tests() {
	// 1. Empty tree stays empty.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(EditPacket::new(true));
		assert!(!tree.occupied);
	}

	// 2. Single voxel edit (depth=DEPTH).
	{
		let mut tree = TestTree::new(1);
		let mut p = EditPacket::new(false);
		p.add_edit(Edit::new(42, [0, 0, 0], DEPTH as u8, 1));
		tree.apply_edit_packet(p);
		assert!(tree.occupied);
		assert_tree_valid(&tree);
	}

	// 3. Root-level edit (depth=0) collapses tree.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(make_sphere(100));
		let mut p = EditPacket::new(false);
		p.add_edit(Edit::new(99, [0, 0, 0], 0, 1));
		tree.apply_edit_packet(p);
		assert!(tree.is_leaf && tree.value == 99);
	}

	// 4. Root DELETE clears tree.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(make_sphere(100));
		let mut p = EditPacket::new(false);
		p.add_edit(Edit::new(DELETE, [0, 0, 0], 0, 1));
		tree.apply_edit_packet(p);
		assert!(!tree.occupied);
	}

	// 5. Sphere produces valid tree.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(make_sphere(100));
		assert!(tree.occupied);
		assert_tree_valid(&tree);
	}

	// 6. Two overlapping spheres.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(make_sphere(80));
		assert_tree_valid(&tree);
		tree.apply_edit_packet(make_sphere(40));
		assert_tree_valid(&tree);
	}

	// 7. Sphere then delete sphere.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(make_sphere(100));
		let del = {
			let center = [chunk::SIDE as i64 / 2; 3];
			let sphere = Sphere { center, radius: 100, material: Voxel::delete() };
			edit_packet_for_shape::<DEPTH>(&sphere, root_aabb())
		};
		tree.apply_edit_packet(del);
		assert_tree_valid(&tree);
	}

	// 8. Expanding a root leaf with a sub-edit.
	{
		let mut tree = TestTree::new(1);
		let mut p = EditPacket::new(false);
		p.add_edit(Edit::new(1, [0, 0, 0], 0, 1));
		tree.apply_edit_packet(p);
		let mut p2 = EditPacket::new(false);
		p2.add_edit(Edit::new(2, [0, 0, 0], DEPTH as u8, 1));
		tree.apply_edit_packet(p2);
		assert!(tree.occupied && !tree.is_leaf);
		assert_tree_valid(&tree);
	}

	// 9. Deleting a voxel from a full leaf tree.
	{
		let mut tree = TestTree::new(1);
		let mut p = EditPacket::new(false);
		p.add_edit(Edit::new(7, [0, 0, 0], 0, 1));
		tree.apply_edit_packet(p);
		let mut p2 = EditPacket::new(false);
		p2.add_edit(Edit::new(DELETE, [0, 0, 0], DEPTH as u8, 1));
		tree.apply_edit_packet(p2);
		assert!(tree.occupied);
		assert_tree_valid(&tree);
	}

	// 10. Idempotency.
	{
		let mut t1 = TestTree::new(1);
		let mut t2 = TestTree::new(1);
		let p = make_sphere(60);
		t1.apply_edit_packet(p.clone());
		t2.apply_edit_packet(p.clone());
		t2.apply_edit_packet(p);
		assert_tree_valid(&t1);
		assert_tree_valid(&t2);
	}

	println!("all apply tests passed");
}

fn time_one<F: FnOnce()>(f: F) -> std::time::Duration {
	let t = Instant::now();
	f();
	t.elapsed()
}

fn fmt_duration(d: std::time::Duration) -> String {
	let ns = d.as_nanos();
	if ns < 1_000 { format!("{ns}ns") }
	else if ns < 1_000_000 { format!("{:.1}µs", ns as f64 / 1_000.0) }
	else if ns < 1_000_000_000 { format!("{:.2}ms", ns as f64 / 1_000_000.0) }
	else { format!("{:.3}s", ns as f64 / 1_000_000_000.0) }
}

fn make_grid_spheres() -> TestTree {
	// 16x16x16 = 4096 spheres, each radius 7 centered at (8,8,8) within its 16^3 cell.
	// Every sphere is identical relative to its levels[2] node, so compact() should
	// deduplicate all 4096 copies down to a single unique subtree.
	let material = Voxel::from_rgb_flags([100, 150, 200], 0, false, false, false, false);
	let aabb = root_aabb();
	let mut tree = TestTree::new(1);
	for x in 0..16i64 {
		for y in 0..16i64 {
			for z in 0..16i64 {
				let center = [x * 16 + 8, y * 16 + 8, z * 16 + 8];
				let sphere = Sphere { center, radius: 7, material };
				tree.apply_edit_packet(edit_packet_for_shape::<DEPTH>(&sphere, aabb));
			}
		}
	}
	tree
}

fn print_node_counts(label: &str, tree: &TestTree) {
	let mut t = tree.clone();
	t.compact();
	let counts: Vec<u32> = t.levels.iter().map(|l| l.node_count()).collect();
	println!("  {label}: {:?}", counts);
}

fn main() {
	run_tests();

	let radii: Vec<i64> = (0..=7).map(|i| 1i64 << i).collect();

	println!("\nsphere edit packet depth distribution:");
	for &r in &radii {
		let packet = make_sphere(r);
		print_packet_levels(&format!("r={r:3}"), &packet);
	}

	println!("\nsphere stats (fresh tree, leaf_size=1):");
	for &r in &radii {
		let packet = make_sphere(r);
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(packet);
		print_stats(&format!("r={r:3}"), &tree);
	}

	println!("\napply_sphere_fresh:");
	for &r in &radii {
		let packet = make_sphere(r);
		let d = time_one(|| {
			let mut tree = TestTree::new(1);
			tree.apply_edit_packet(packet.clone());
			std::hint::black_box(tree);
		});
		println!("  r={r:3}: {}", fmt_duration(d));
	}

	let mut full_tree = TestTree::new(1);
	{
		let mut p = EditPacket::new(false);
		p.add_edit(Edit::new(1, [0, 0, 0], 0, 1));
		full_tree.apply_edit_packet(p);
	}

	println!("\napply_sphere_onto_full:");
	for &r in &radii {
		let packet = make_sphere(r);
		let d = time_one(|| {
			let mut tree = full_tree.clone();
			tree.apply_edit_packet(packet.clone());
			std::hint::black_box(tree);
		});
		println!("  r={r:3}: {}", fmt_duration(d));
	}

	println!("\nsingle r=7 sphere (same alignment as grid):");
	{
		let material = Voxel::from_rgb_flags([100, 150, 200], 0, false, false, false, false);
		let sphere = Sphere { center: [8, 8, 8], radius: 7, material };
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(edit_packet_for_shape::<DEPTH>(&sphere, root_aabb()));
		print_stats("stats", &tree);
		print_node_counts("node counts per level", &tree);
	}

	println!("\ngrid spheres (4096 x r=7, perfectly aligned for DAG dedup):");
	let grid = make_grid_spheres();
	print_stats("stats", &grid);
	print_node_counts("node counts per level", &grid);
}
