use std::array::from_fn;
use crate::tree::{Level, Tree};
use crate::tree::edit::DELETE;

// Merge 64 Tree<DEPTH> children into one coarser Tree<DEPTH>.
// Each child covers 1/64th of the output space (one slot of the root).
// leaf_size must equal the children's leaf_size; output gets leaf_size * 4.
// The bottom level of each child is dropped; its LOD values become the leaf data.
pub fn merge<const DEPTH: usize>(children: &[Tree<DEPTH>; 64]) -> Tree<DEPTH> {
	assert!(DEPTH >= 2, "merge requires DEPTH >= 2");
	let leaf_size = children[0].leaf_size * 4;

	// Special case: if every child is empty, return an empty tree.
	if children.iter().all(|c| !c.occupied) {
		return Tree::new(leaf_size);
	}

	// Build the new root node at output levels[0].
	// Each child maps to one slot. After we know which children contribute
	// non-leaf slots, we can compute the child-node offsets into output levels[1].
	//
	// We need to assign output-levels[1] node indices to each structured child.
	// Input child i's root is the last node in child.levels[0] (after compact it's
	// always node_count-1, but we handle the general case by using node_count-1
	// when there is at least one node).
	//
	// We'll concatenate input.levels[0] for all structured children into output.levels[1],
	// then input.levels[1] → output.levels[2], etc.

	// For each child, determine slot type and the root node index within its levels[0].
	#[derive(Clone, Copy)]
	enum SlotKind {
		Empty,
		Leaf(u32),       // value
		Node(usize, u32), // child_array_index (index into `structured`), root node in levels[0]
	}
	let mut slots = [SlotKind::Empty; 64];
	let mut structured: Vec<usize> = Vec::new(); // indices into children[] that have a real tree

	for (s, child) in children.iter().enumerate() {
		if !child.occupied {
			slots[s] = SlotKind::Empty;
		} else if child.is_leaf {
			slots[s] = SlotKind::Leaf(child.value);
		} else if DEPTH == 1 || child.levels[0].node_count() == 0 {
			// No levels to descend into — treat as leaf using value.
			slots[s] = SlotKind::Leaf(child.value);
		} else {
			let arr_idx = structured.len();
			slots[s] = SlotKind::Node(arr_idx, child.levels[0].node_count() - 1);
			structured.push(s);
		}
	}

	// Prefix sums of node counts and children_len at each depth,
	// over the structured children in order.
	// output.levels[d+1] = concat of structured_child[i].levels[d] for i in 0..
	let n_struct = structured.len();
	// node_offset[i][d] = where structured child i's levels[d] nodes start in output.levels[d+1]
	let mut node_offset: Vec<Vec<u32>> = vec![vec![0u32; DEPTH]; n_struct];
	let mut children_offset_base: Vec<Vec<u32>> = vec![vec![0u32; DEPTH]; n_struct];

	for d in 0..(DEPTH - 1) {
		let mut nc_acc = 0u32;
		let mut co_acc = 0u32;
		for (i, &ci) in structured.iter().enumerate() {
			node_offset[i][d] = nc_acc;
			children_offset_base[i][d] = co_acc;
			let lvl = &children[ci].levels[d];
			nc_acc += lvl.node_count();
			co_acc += lvl.children_len();
		}
	}

	// Build output levels[1..DEPTH-1] by concatenating structured children's levels[0..DEPTH-2].
	// For each non-leaf slot's node_children entry, add node_offset for the appropriate child.
	let mut out_levels: [Level; DEPTH] = from_fn(|_| Level::new());

	for d in 0..(DEPTH - 1) {
		let out_d = d + 1; // output level index
		if out_d >= DEPTH { break; }

		for (i, &ci) in structured.iter().enumerate() {
			let src = &children[ci].levels[d];
			let n_node_offset = node_offset[i][d];
			let children_off_shift = children_offset_base[i][d];

			// child node index offset for this depth's children (they live in output.levels[d+2])
			let child_node_shift = if d + 1 < DEPTH - 1 {
				// structured child i's levels[d+1] starts at node_offset[i][d+1] in output.levels[d+2]
				node_offset[i][d + 1]
			} else {
				0 // leaf level — no child nodes
			};

			let is_bottom = d + 1 == DEPTH - 1;
			// When d+1 == DEPTH-1, input's levels[d] children point into input's levels[d+1]
			// which we are DROPPING. Instead we convert those slots to leaves.
			// The values (LOD values) are already in the values array.

			for n in 0..src.node_count() {
				let occ = src.occupancy_mask[n as usize];
				let leaf = src.leaf_mask[n as usize];
				let base = src.children_offset[n as usize];
				let count = occ.count_ones();

				let new_children_offset = base + children_off_shift;

				// Push children (values + node_children).
				let mut mask = occ;
				while mask != 0 {
					let s = mask.trailing_zeros() as usize;
					let rank = (occ & ((1u64 << s) - 1)).count_ones();
					let val = src.values.get(base + rank);

					if is_bottom {
						// Drop child pointer; this slot becomes a leaf in output.
						out_levels[out_d].push_child(0, val);
					} else if (leaf >> s) & 1 != 0 {
						// Already a leaf in source — stays a leaf.
						out_levels[out_d].push_child(0, val);
					} else {
						// Non-leaf: remap child node index.
						let child_node = src.node_children.get(base + rank);
						out_levels[out_d].push_child(child_node + child_node_shift, val);
					}
					mask &= mask - 1;
				}

				// Push node. If is_bottom, all occupied slots become leaves.
				let new_leaf = if is_bottom { occ } else { leaf };
				out_levels[out_d].push_node(occ, new_leaf, new_children_offset);
				let _ = n_node_offset; // used in slots building above
			}
		}
	}

	// Build output levels[0]: single root node with up to 64 slots.
	{
		let lvl = &mut out_levels[0];
		let base = lvl.children_len();
		let mut occ = 0u64;
		let mut leaf_mask = 0u64;

		for s in 0u8..64 {
			match slots[s as usize] {
				SlotKind::Empty => {}
				SlotKind::Leaf(val) => {
					lvl.push_child(0, val);
					occ |= 1u64 << s;
					leaf_mask |= 1u64 << s;
				}
				SlotKind::Node(arr_idx, root_in_src) => {
					// In output.levels[1], structured child arr_idx's nodes
					// start at node_offset[arr_idx][0].
					let out_root = root_in_src + node_offset[arr_idx][0];
					// LOD value for this slot: lod of the child's root node in levels[0].
					let child = &children[structured[arr_idx]];
					let lod = lod_of_node(&child.levels[0], root_in_src);
					lvl.push_child(out_root, lod);
					occ |= 1u64 << s;
				}
			}
		}

		lvl.push_node(occ, leaf_mask, base);
	}

	Tree {
		occupied: true,
		is_leaf: false,
		value: 0,
		leaf_size,
		edits: Default::default(),
		levels: out_levels,
	}
}

// Split one Tree<DEPTH> into 64 finer Tree<DEPTH> children.
// Each child covers one slot of the root and gets leaf_size / 4.
// A new empty level is appended at the bottom of each child (it was a leaf level in the parent).
pub fn split<const DEPTH: usize>(tree: &Tree<DEPTH>) -> [Tree<DEPTH>; 64] {
	assert!(DEPTH >= 1, "split requires DEPTH >= 1");
	assert!(tree.leaf_size % 4 == 0, "leaf_size must be divisible by 4 to split");
	let child_leaf_size = tree.leaf_size / 4;

	if !tree.occupied {
		return from_fn(|_| Tree::new(child_leaf_size));
	}

	if tree.is_leaf {
		// Every child gets the same uniform leaf tree.
		return from_fn(|_| {
			let mut t = Tree::new(child_leaf_size);
			t.occupied = true;
			t.is_leaf = true;
			t.value = tree.value;
			t
		});
	}

	if tree.levels[0].node_count() == 0 {
		return from_fn(|_| Tree::new(child_leaf_size));
	}

	let root = tree.levels[0].node_count() - 1;
	let root_occ  = tree.levels[0].occupancy_mask[root as usize];
	let root_leaf = tree.levels[0].leaf_mask[root as usize];
	let root_base = tree.levels[0].children_offset[root as usize];

	from_fn(|s| {
		let occupied = (root_occ >> s) & 1 != 0;
		if !occupied {
			return Tree::new(child_leaf_size);
		}

		let rank = (root_occ & ((1u64 << s) - 1)).count_ones();
		let is_leaf_slot = (root_leaf >> s) & 1 != 0;
		let val = tree.levels[0].values.get(root_base + rank);

		if is_leaf_slot || DEPTH == 1 {
			let mut t = Tree::new(child_leaf_size);
			t.occupied = true;
			t.is_leaf = true;
			t.value = val;
			return t;
		}

		// Non-leaf slot: extract the subtree rooted at this child.
		let child_root = tree.levels[0].node_children.get(root_base + rank);
		let levels = extract_subtree::<DEPTH>(tree, child_root);

		Tree {
			occupied: true,
			is_leaf: false,
			value: val,
			leaf_size: child_leaf_size,
			edits: Default::default(),
			levels,
		}
	})
}

// Extract the subtree rooted at `root` in tree.levels[1..] into a fresh [Level; DEPTH].
// The extracted tree uses levels[0..DEPTH-2] from tree.levels[1..DEPTH-1],
// then appends an empty levels[DEPTH-1].
fn extract_subtree<const DEPTH: usize>(tree: &Tree<DEPTH>, root: u32) -> [Level; DEPTH] {
	// BFS/DFS to collect which nodes are reachable from root at each depth.
	let mut reachable: Vec<Vec<u32>> = (0..DEPTH).map(|_| Vec::new()).collect();

	// levels[1] in the parent = levels[0] in the child
	collect_reachable::<DEPTH>(&tree.levels, 1, root, &mut reachable, 0);

	// Build new levels by copying only reachable nodes and remapping indices.
	let mut out: [Level; DEPTH] = from_fn(|_| Level::new());

	// new_idx[d][old_node] = new node index in out[d]
	let mut new_idx: Vec<Vec<u32>> = (0..DEPTH - 1).map(|d| {
		let max_node = tree.levels[d + 1].node_count() as usize;
		vec![u32::MAX; max_node]
	}).collect();

	for d in 0..(DEPTH - 1) {
		let src = &tree.levels[d + 1];
		let dst_d = d;

		// Process reachable nodes in order (preserves original ordering).
		let mut sorted = reachable[d].clone();
		sorted.sort_unstable();
		sorted.dedup();

		for &n in &sorted {
			let occ  = src.occupancy_mask[n as usize];
			let leaf = src.leaf_mask[n as usize];
			let base = src.children_offset[n as usize];
			let is_last = dst_d + 1 == DEPTH - 1;

			let new_base = out[dst_d].children_len();
			let mut mask = occ;
			while mask != 0 {
				let s = mask.trailing_zeros() as usize;
				let rank = (occ & ((1u64 << s) - 1)).count_ones();
				let val = src.values.get(base + rank);
				if (leaf >> s) & 1 != 0 || is_last {
					out[dst_d].push_child(0, val);
				} else {
					let child_node = src.node_children.get(base + rank);
					// will be patched below once we know new indices
					out[dst_d].push_child(child_node, val); // placeholder
				}
				mask &= mask - 1;
			}
			let new_n = out[dst_d].push_node(occ, leaf, new_base);
			new_idx[dst_d][n as usize] = new_n;
		}

		// Patch non-leaf child node indices.
		if dst_d + 1 < DEPTH - 1 {
			for &n in &sorted {
				let src_occ  = src.occupancy_mask[n as usize];
				let src_leaf = src.leaf_mask[n as usize];
				let src_base = src.children_offset[n as usize];
				let new_n = new_idx[dst_d][n as usize];
				let new_base = out[dst_d].children_offset[new_n as usize];

				let mut mask = src_occ & !src_leaf;
				while mask != 0 {
					let s = mask.trailing_zeros() as usize;
					let rank = (src_occ & ((1u64 << s) - 1)).count_ones();
					let old_child = src.node_children.get(src_base + rank);
					let remapped = new_idx[dst_d + 1][old_child as usize];
					debug_assert_ne!(remapped, u32::MAX, "child node not in reachable set");
					out[dst_d].node_children.set(new_base + rank, remapped);
					mask &= mask - 1;
				}
			}
		}
	}
	// out[DEPTH-1] stays empty — the new empty bottom level.
	out
}

fn collect_reachable<const DEPTH: usize>(
	levels: &[Level; DEPTH],
	src_d: usize,   // index into `levels`
	node: u32,
	reachable: &mut Vec<Vec<u32>>,
	dst_d: usize,   // index into `reachable`
) {
	if src_d >= DEPTH { return; }
	reachable[dst_d].push(node);
	let level = &levels[src_d];
	let occ  = level.occupancy_mask[node as usize];
	let leaf = level.leaf_mask[node as usize];
	let base = level.children_offset[node as usize];
	let is_last = src_d + 1 == DEPTH;
	if is_last { return; }
	let mut mask = occ & !leaf;
	while mask != 0 {
		let s = mask.trailing_zeros() as usize;
		let rank = (occ & ((1u64 << s) - 1)).count_ones();
		let child = level.node_children.get(base + rank);
		collect_reachable::<DEPTH>(levels, src_d + 1, child, reachable, dst_d + 1);
		mask &= mask - 1;
	}
}

// Compute the LOD representative value for a node (mode of occupied children).
// Returns DELETE if fewer than 32 of the 64 slots are occupied.
fn lod_of_node(level: &Level, node: u32) -> u32 {
	let occ      = level.occupancy_mask[node as usize];
	let occupied = occ.count_ones() as usize;
	if occupied < 32 { return DELETE; }
	let base = level.children_offset[node as usize];
	let mut best_val   = level.values.get(base);
	let mut best_count = 0usize;
	for rank in 0..occupied {
		let val   = level.values.get(base + rank as u32);
		let count = (0..occupied).filter(|&r| level.values.get(base + r as u32) == val).count();
		if count > best_count { best_count = count; best_val = val; }
		if best_count > occupied / 2 { break; }
	}
	best_val
}
