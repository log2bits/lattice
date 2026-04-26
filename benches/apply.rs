use criterion::{Criterion, criterion_group, criterion_main};
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

fn sphere_packet(center: [i64; 3], radius: i64) -> EditPacket<DEPTH> {
	let sphere = Sphere {
		center,
		radius,
		material: Voxel::from_rgb_flags([100, 150, 200], 0, false, false, false, false),
	};
	edit_packet_for_shape::<DEPTH>(&sphere, root_aabb())
}

// ── correctness tests ────────────────────────────────────────────────────────

fn assert_tree_valid(tree: &TestTree) {
	// Every non-leaf slot at each level must point to a valid child node index.
	for d in 0..DEPTH {
		let level = &tree.levels[d];
		for n in 0..level.node_count() {
			let occ = level.occupancy_mask[n as usize];
			let leaf = level.leaf_mask[n as usize];
			let base = level.children_offset[n as usize];
			let count = occ.count_ones();
			// children must be in bounds
			assert!(
				base + count <= level.node_children.len(),
				"level {d} node {n}: children out of bounds (base={base} count={count} len={})",
				level.node_children.len()
			);
			// non-leaf slots must point to a valid node at the next level
			if d + 1 < DEPTH {
				let mut mask = occ & !leaf;
				while mask != 0 {
					let slot = mask.trailing_zeros() as u8;
					let rank = (occ & ((1u64 << slot) - 1)).count_ones();
					let child = level.node_children.get(base + rank);
					assert!(
						(child as usize) < tree.levels[d + 1].node_count() as usize,
						"level {d} node {n} slot {slot}: child {child} out of range (next level has {} nodes)",
						tree.levels[d + 1].node_count()
					);
					mask &= mask - 1;
				}
			}
		}
	}
}

fn run_tests() {
	// 1. Empty tree stays empty after empty packet.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(EditPacket::new(true));
		assert!(!tree.occupied, "empty tree should remain unoccupied");
	}

	// 2. Single leaf edit makes tree occupied.
	{
		let mut tree = TestTree::new(1);
		let mut packet = EditPacket::new(false);
		packet.add_edit(lattice::tree::Edit::new(42, [0, 0, 0], 0, 1));
		tree.apply_edit_packet(packet);
		assert!(tree.occupied, "tree should be occupied after leaf edit");
		assert_tree_valid(&tree);
	}

	// 3. Root-level edit collapses entire tree to leaf.
	{
		let mut tree = TestTree::new(1);
		// Fill with sphere first.
		tree.apply_edit_packet(sphere_packet([128, 128, 128], 100));
		assert_tree_valid(&tree);
		// Then overwrite with root-level edit.
		let mut packet = EditPacket::new(false);
		packet.add_edit(lattice::tree::Edit::new(99, [0, 0, 0], DEPTH as u8, 1));
		tree.apply_edit_packet(packet);
		assert!(tree.is_leaf && tree.value == 99, "root edit should collapse tree");
		for d in 0..DEPTH {
			assert_eq!(tree.levels[d].node_count(), 0, "levels should be empty after root collapse");
		}
	}

	// 4. DELETE on root clears tree.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(sphere_packet([128, 128, 128], 100));
		let mut packet = EditPacket::new(false);
		packet.add_edit(lattice::tree::Edit::new(DELETE, [0, 0, 0], DEPTH as u8, 1));
		tree.apply_edit_packet(packet);
		assert!(!tree.occupied, "tree should be empty after root DELETE");
	}

	// 5. Sphere fill produces valid tree.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(sphere_packet([128, 128, 128], 100));
		assert!(tree.occupied);
		assert_tree_valid(&tree);
	}

	// 6. Two overlapping spheres.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(sphere_packet([100, 128, 128], 80));
		assert_tree_valid(&tree);
		tree.apply_edit_packet(sphere_packet([160, 128, 128], 80));
		assert_tree_valid(&tree);
	}

	// 7. Applying then deleting a sphere leaves a valid (possibly empty) tree.
	{
		let mut tree = TestTree::new(1);
		tree.apply_edit_packet(sphere_packet([128, 128, 128], 100));
		let del_sphere = Sphere {
			center: [128, 128, 128],
			radius: 100,
			material: Voxel::delete(),
		};
		let del_packet = edit_packet_for_shape::<DEPTH>(&del_sphere, root_aabb());
		tree.apply_edit_packet(del_packet);
		assert_tree_valid(&tree);
	}

	// 8. Subtree-level edit expands existing leaf correctly.
	{
		let mut tree = TestTree::new(1);
		// Fill entire tree with value 1 via root edit.
		let mut packet = EditPacket::new(false);
		packet.add_edit(lattice::tree::Edit::new(1, [0, 0, 0], DEPTH as u8, 1));
		tree.apply_edit_packet(packet);
		// Now edit a single leaf to value 2.
		let mut packet2 = EditPacket::new(false);
		packet2.add_edit(lattice::tree::Edit::new(2, [0, 0, 0], 0, 1));
		tree.apply_edit_packet(packet2);
		assert!(tree.occupied);
		assert!(!tree.is_leaf, "tree should no longer be a single leaf");
		assert_tree_valid(&tree);
	}

	// 9. Deleting a single voxel from a full leaf tree leaves valid tree.
	{
		let mut tree = TestTree::new(1);
		let mut packet = EditPacket::new(false);
		packet.add_edit(lattice::tree::Edit::new(7, [0, 0, 0], DEPTH as u8, 1));
		tree.apply_edit_packet(packet);
		let mut packet2 = EditPacket::new(false);
		packet2.add_edit(lattice::tree::Edit::new(DELETE, [0, 0, 0], 0, 1));
		tree.apply_edit_packet(packet2);
		assert!(tree.occupied);
		assert_tree_valid(&tree);
	}

	// 10. Applying the same packet twice is idempotent.
	{
		let mut tree1 = TestTree::new(1);
		let mut tree2 = TestTree::new(1);
		let p = sphere_packet([128, 128, 128], 60);
		tree1.apply_edit_packet(p.clone());
		tree2.apply_edit_packet(p.clone());
		tree2.apply_edit_packet(p);
		// Both should produce valid trees; tree2 is idempotent.
		assert_tree_valid(&tree1);
		assert_tree_valid(&tree2);
	}

	println!("all apply tests passed");
}

// ── benchmarks ───────────────────────────────────────────────────────────────

fn bench_apply(c: &mut Criterion) {
	run_tests();

	let packet = sphere_packet([128, 128, 128], 100);

	c.bench_function("apply_sphere_fresh", |b| {
		b.iter(|| {
			let mut tree = TestTree::new(1);
			tree.apply_edit_packet(packet.clone());
			tree
		});
	});

	// Apply sphere onto an already-filled tree (worst case for leaf expansion).
	let mut base_tree = TestTree::new(1);
	let mut full_packet = EditPacket::new(false);
	full_packet.add_edit(lattice::tree::Edit::new(1, [0, 0, 0], DEPTH as u8, 1));
	base_tree.apply_edit_packet(full_packet);

	c.bench_function("apply_sphere_onto_full_tree", |b| {
		b.iter(|| {
			let mut tree = base_tree.clone();
			tree.apply_edit_packet(packet.clone());
			tree
		});
	});

	// Two spheres sequentially.
	c.bench_function("apply_two_spheres", |b| {
		b.iter(|| {
			let mut tree = TestTree::new(1);
			tree.apply_edit_packet(sphere_packet([90, 128, 128], 80));
			tree.apply_edit_packet(sphere_packet([170, 128, 128], 80));
			tree
		});
	});
}

criterion_group!(benches, bench_apply);
criterion_main!(benches);
