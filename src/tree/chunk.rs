use crate::{tree::Level, voxel::Voxel};

pub struct Chunk {
	pub root: u32,
	pub materials: MaterialTable,
	pub levels: Vec<Level>,
}

pub struct MaterialTable {
	pub values: Vec<Voxel>,
}

impl Chunk {
	pub fn new() -> Self {
		Self {
			root: 0,
			materials: MaterialTable::new(),
			levels: Vec::new(),
		}
	}

	pub fn depth(&self) -> u8 {
		self.levels.len() as u8
	}

	pub fn get_voxel(&self, pos: [u32; 3]) -> Option<Voxel> {
		let mut node_idx = self.root;
		for level_idx in 0..self.depth() as u32 {
			let level = &self.levels[level_idx as usize];
			let slot = Self::slot_at_level(pos, level_idx, self.depth());
			if !level.is_occupied(node_idx, slot) {
				return None;
			}
			let child_idx = level.child_idx(node_idx, slot);
			if level.is_solid(node_idx, slot) || level_idx == self.depth() as u32 - 1 {
				return Some(self.materials.get(level.leaf_material(child_idx)));
			}
			node_idx = level.node_child(child_idx);
		}
		None
	}

	pub fn place_voxel(&mut self, pos: [u32; 3], voxel: Voxel) {
		let mat_idx = self.materials.intern(voxel);
		let depth = self.depth() as u32;
		let mut node_idx = self.root;
		let mut path: Vec<(u32, u32)> = Vec::with_capacity(depth as usize);

		for level_idx in 0..depth {
			let slot = Self::slot_at_level(pos, level_idx, depth as u8);
			let is_leaf = level_idx == depth - 1;
			if !self.levels[level_idx as usize].is_occupied(node_idx, slot) {
				// slot is empty — structural insert not yet supported
				return;
			}
			let child_idx = self.levels[level_idx as usize].child_idx(node_idx, slot);
			let is_solid = self.levels[level_idx as usize].is_solid(node_idx, slot);
			if is_solid || is_leaf {
				self.levels[level_idx as usize].set_leaf_material(child_idx, mat_idx);
				path.push((level_idx, node_idx));
				break;
			}
			let next_node = self.levels[level_idx as usize].node_child(child_idx);
			path.push((level_idx, node_idx));
			node_idx = next_node;
		}

		// Update lod_material up the path.
		// Simple approximation: propagate the new material up rather than recomputing dominance.
		for (level_idx, ancestor_node_idx) in path.into_iter().rev() {
			self.levels[level_idx as usize].set_lod_material(ancestor_node_idx, mat_idx);
		}
	}

	pub fn remove_voxel(&mut self, pos: [u32; 3]) {
		let depth = self.depth() as u32;
		let mut node_idx = self.root;
		let mut path: Vec<(u32, u32)> = Vec::with_capacity(depth as usize);

		for level_idx in 0..depth {
			let slot = Self::slot_at_level(pos, level_idx, depth as u8);
			let is_leaf = level_idx == depth - 1;
			if !self.levels[level_idx as usize].is_occupied(node_idx, slot) {
				return; // already empty
			}
			let child_idx = self.levels[level_idx as usize].child_idx(node_idx, slot);
			let is_solid = self.levels[level_idx as usize].is_solid(node_idx, slot);
			if is_solid || is_leaf {
				let occ = self.levels[level_idx as usize].occupancy(node_idx);
				self.levels[level_idx as usize].set_occupancy(node_idx, occ & !(1u64 << slot));
				let solid = self.levels[level_idx as usize].solid_mask(node_idx);
				self.levels[level_idx as usize].set_solid_mask(node_idx, solid & !(1u64 << slot));
				path.push((level_idx, node_idx));
				break;
			}
			let next_node = self.levels[level_idx as usize].node_child(child_idx);
			path.push((level_idx, node_idx));
			node_idx = next_node;
		}

		// lod_material update on remove would require recomputing dominance from siblings.
		// Left for when we have a proper blending pass.
		let _ = path;
	}

	// Extract the 4x4x4 slot index for a given position at a given level.
	// At level 0 (root), uses the top 2 bits of each axis. At level depth-1, uses the bottom 2 bits.
	fn slot_at_level(pos: [u32; 3], level_idx: u32, depth: u8) -> u32 {
		let shift = 2 * (depth as u32 - 1 - level_idx);
		let sx = (pos[0] >> shift) & 3;
		let sy = (pos[1] >> shift) & 3;
		let sz = (pos[2] >> shift) & 3;
		sx | (sy << 2) | (sz << 4)
	}
}

impl MaterialTable {
	pub fn new() -> Self {
		Self { values: Vec::new() }
	}

	pub fn len(&self) -> u32 {
		self.values.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}

	pub fn get(&self, idx: u32) -> Voxel {
		self.values[idx as usize]
	}

	// Returns the index of the voxel, inserting it if not present.
	pub fn intern(&mut self, voxel: Voxel) -> u32 {
		if let Some(idx) = self.values.iter().position(|&v| v == voxel) {
			return idx as u32;
		}
		self.values.push(voxel);
		self.values.len() as u32 - 1
	}
}


/*

Here's the core issue.                   
                                               
  How children are found right now:              
                     
  For node N, its children start at index        
  children_offset[N] in the flat array. To find  
  the child at slot S, you count how many        
  occupied slots come before S — that's the rank.
   So the child is at children_offset[N] + rank. 

  This works because children are stored densely 
  in slot order within each node's block. Node N
  with 3 occupied children occupies exactly 3    
  consecutive entries.

  What happens when you insert a new child:      
  
  Say node N has children at slots 2, 5, 7 and   
  you want to add slot 4. The new rank of slot 4
  is 1 (slot 2 comes before it). So the new child
   needs to go at children_offset[N] + 1. But
  that position is currently occupied by the
  child at slot 5. You have to shift slot 5 and
  slot 7 forward by one to make room.

  Now every node after N in the array has its    
  children shifted forward by one, but their
  children_offset values still point to the old  
  positions. So you have to increment
  children_offset for every node after N. That's
  O(nodes in level).

  Why the user is right that order doesn't
  matter:

  The children of node N don't actually need to  
  be in slot order. They just need to be
  findable. The rank calculation is one way to   
  find them, but not the only way.

  The fundamental tension:

  Dense packing (good for memory and cache)      
  requires rank-based addressing, which requires
  slot-ordered contiguous storage, which requires
   shifting on insert.

  Any solution that avoids shifting either adds  
  per-child overhead (store slot number alongside
   value), wastes space (fixed 64 slots), or adds
   a level of indirection (freelist, pointers).

  There's no free lunch here. The question is    
  which tradeoff fits your use case best. What
  matters more to you — keeping memory tight,    
  keeping inserts fast, or keeping traversal
  simple?
*/