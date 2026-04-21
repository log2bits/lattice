use crate::{tree::Level, voxel::Voxel};

mod canonicalize;
mod edit;
mod material;
mod shape;

pub use edit::Edit;
pub use material::MaterialTable;
pub use shape::Coverage;

// A batch of edits submitted together. Packets are applied in submission order,
// so two intersecting operations stay ordered correctly.
// presorted=true: edits are already in tree-order — skip the sort at flush time.
struct EditPacket {
	edits: Vec<Edit>,
	presorted: bool,
}

pub struct Chunk {
	pub root: u32,
	pub materials: MaterialTable,
	pub levels: Vec<Level>,
	pub svdag_clean: bool,
	pending: Vec<EditPacket>,
	auto_flush_threshold: usize,
}

impl Chunk {
	pub fn with_depth(depth: u8) -> Self {
		// Threshold equals total voxel count — a safety valve for unbounded voxel edit batches.
		let auto_flush_threshold = 4usize.pow(3 * depth as u32);
		let mut levels = Vec::with_capacity(depth as usize);
		levels.push(Level::with_root_node());
		for _ in 1..depth {
			levels.push(Level::default());
		}
		Self {
			root: 0,
			materials: MaterialTable::default(),
			levels,
			svdag_clean: true,
			pending: Vec::new(),
			auto_flush_threshold,
		}
	}

	pub fn depth(&self) -> u8 {
		self.levels.len() as u8
	}

	pub fn memory_bytes(&self) -> usize {
		self.levels
			.iter()
			.map(|l| {
				l.occupancy_mask.len() * 8
					+ l.terminal_mask.len() * 8
					+ l.children_offset.len() * 4
					+ l.node_children.data.len() * 4
					+ l.materials.data.len() * 4
			})
			.sum::<usize>()
			+ self.materials.values.len() * std::mem::size_of::<Voxel>()
	}

	pub fn get_voxel(&self, pos: [u32; 3]) -> Option<Voxel> {
		let depth = self.depth();
		let mut node_idx = self.root;
		for level_idx in 0..depth {
			let slot = Self::slot_at_level(pos, level_idx as u32, depth);
			let level = &self.levels[level_idx as usize];
			if !level.is_occupied(node_idx, slot) {
				return None;
			}
			let child_idx = level.child_idx(node_idx, slot);
			if level_idx == depth - 1 || level.is_terminal(node_idx, slot) {
				return Some(self.materials.get(level.materials.get(child_idx)));
			}
			node_idx = level.node_children.get(child_idx);
		}
		None
	}

	/// Returns true if there are pending edits or the tree needs canonicalization.
	pub fn has_pending_edits(&self) -> bool {
		!self.pending.is_empty() || !self.svdag_clean
	}

	pub fn queue_set(&mut self, pos: [u32; 3], voxel: Voxel) {
		self.push_voxel_edit(Edit {
			pos,
			level: self.depth() - 1,
			fill: Some(voxel),
		});
	}

	pub fn queue_remove(&mut self, pos: [u32; 3]) {
		self.push_voxel_edit(Edit {
			pos,
			level: self.depth() - 1,
			fill: None,
		});
	}
}

// Helpers shared across edit, shape, and canonicalize.
impl Chunk {
	fn push_voxel_edit(&mut self, edit: Edit) {
		match self.pending.last_mut() {
			Some(p) if !p.presorted => p.edits.push(edit),
			_ => self.pending.push(EditPacket {
				edits: vec![edit],
				presorted: false,
			}),
		}
		self.svdag_clean = false;
		let total: usize = self.pending.iter().map(|p| p.edits.len()).sum();
		if total >= self.auto_flush_threshold {
			self.flush_edits();
		}
	}

	fn expand_terminal(&mut self, level_idx: u8, material: u32) -> u32 {
		let depth = self.depth();
		let is_leaf = level_idx == depth - 1;
		let new_node = self.levels[level_idx as usize].occupancy_mask.len() as u32;
		let offset = if is_leaf {
			self.levels[level_idx as usize].materials.len()
		} else {
			self.levels[level_idx as usize].node_children.len()
		};
		if is_leaf {
			for _ in 0..64 {
				self.levels[level_idx as usize].push_leaf_material(material);
			}
		} else {
			for _ in 0..64 {
				self.levels[level_idx as usize].push_terminal(material);
			}
		}
		self.levels[level_idx as usize]
			.occupancy_mask
			.push(u64::MAX);
		self.levels[level_idx as usize].terminal_mask.push(u64::MAX);
		self.levels[level_idx as usize].children_offset.push(offset);
		new_node
	}

	fn alloc_empty_node(&mut self, level_idx: u8) -> u32 {
		let depth = self.depth();
		let is_leaf = level_idx == depth - 1;
		let level = &mut self.levels[level_idx as usize];
		let new_idx = level.occupancy_mask.len() as u32;
		let offset = if is_leaf {
			level.materials.len()
		} else {
			level.node_children.len()
		};
		level.occupancy_mask.push(0);
		level.terminal_mask.push(0);
		level.children_offset.push(offset);
		new_idx
	}

	fn uniform_terminal_material(&self, level_idx: u8, node_idx: u32) -> Option<u32> {
		let level = &self.levels[level_idx as usize];
		let occ = level.occupancy_mask[node_idx as usize];
		let term = level.terminal_mask[node_idx as usize];
		if occ != u64::MAX || term != u64::MAX {
			return None;
		}
		let offset = level.children_offset[node_idx as usize];
		let first_mat = level.materials.get(offset);
		if (1..64).any(|i| level.materials.get(offset + i) != first_mat) {
			return None;
		}
		Some(first_mat)
	}

	fn node_lod(&self, level_idx: u8, node_idx: u32) -> u32 {
		let level = &self.levels[level_idx as usize];
		let occ = level.occupancy_mask[node_idx as usize];
		let offset = level.children_offset[node_idx as usize];
		material::material_mode((0..occ.count_ones()).map(|i| level.materials.get(offset + i)))
	}

	pub(crate) fn tree_order_key(pos: [u32; 3], depth: u8) -> u32 {
		let [x, y, z] = pos;
		let mut key = 0u32;
		for level_idx in 0..depth {
			let shift = 2 * (depth - 1 - level_idx) as u32;
			let slot = ((x >> shift) & 3) | (((y >> shift) & 3) << 2) | (((z >> shift) & 3) << 4);
			key = (key << 6) | slot;
		}
		key
	}

	pub(crate) fn slot_at_level(pos: [u32; 3], level_idx: u32, depth: u8) -> u32 {
		let shift = 2 * (depth as u32 - 1 - level_idx);
		((pos[0] >> shift) & 3) | (((pos[1] >> shift) & 3) << 2) | (((pos[2] >> shift) & 3) << 4)
	}
}
