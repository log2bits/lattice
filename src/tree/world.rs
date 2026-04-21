use crate::{
	tree::{Chunk, Coverage},
	voxel::Voxel,
};
use rayon::prelude::*;
use std::time::{Duration, Instant};

pub struct World {
	pub dims: [u32; 3],
	pub chunks: Vec<Chunk>,
	pub entries: Vec<u32>,
	pub depth: u8,
}

impl World {
	pub fn new(dims: [u32; 3], depth: u8) -> Self {
		Self {
			dims,
			entries: vec![0; (dims[0] * dims[1] * dims[2]) as usize],
			chunks: Vec::new(),
			depth,
		}
	}

	pub fn get_voxel(&self, world_pos: [u32; 3]) -> Option<Voxel> {
		let (chunk_idx, local_pos) = self.resolve(world_pos)?;
		self.chunks[chunk_idx].get_voxel(local_pos)
	}

	pub fn set_voxel(&mut self, world_pos: [u32; 3], voxel: Voxel) {
		let (chunk_pos, local_pos) = self.split_world_pos(world_pos);
		if !self.in_bounds(chunk_pos) {
			return;
		}
		let entry_idx = self.entry_idx(chunk_pos);
		if self.entries[entry_idx] == 0 {
			self.insert_chunk(chunk_pos, Chunk::with_depth(self.depth));
		}
		let ci = Self::entry_to_chunk_idx(self.entries[entry_idx]).unwrap();
		self.chunks[ci].queue_set(local_pos, voxel);
	}

	pub fn remove_voxel(&mut self, world_pos: [u32; 3]) {
		let Some((chunk_idx, local_pos)) = self.resolve(world_pos) else {
			return;
		};
		self.chunks[chunk_idx].queue_remove(local_pos);
	}

	/// Fill every voxel inside `shape` with `voxel`.
	pub fn fill_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F, voxel: Voxel) {
		let cs = self.chunk_size();
		let [dx, dy, dz] = self.dims;
		for cz in 0..dz {
			for cy in 0..dy {
				for cx in 0..dx {
					let chunk_pos = [cx, cy, cz];
					let base = [cx * cs, cy * cs, cz * cs];
					if shape(base, cs) == Coverage::None {
						continue;
					}
					let entry_idx = self.entry_idx(chunk_pos);
					if self.entries[entry_idx] == 0 {
						self.insert_chunk(chunk_pos, Chunk::with_depth(self.depth));
					}
					let ci = Self::entry_to_chunk_idx(self.entries[entry_idx]).unwrap();
					let [bx, by, bz] = base;
					self.chunks[ci].fill_shape(
						|lb, s| shape([lb[0] + bx, lb[1] + by, lb[2] + bz], s),
						voxel,
					);
				}
			}
		}
	}

	/// Remove every voxel inside `shape`.
	pub fn clear_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F) {
		let cs = self.chunk_size();
		let [dx, dy, dz] = self.dims;
		for cz in 0..dz {
			for cy in 0..dy {
				for cx in 0..dx {
					let chunk_pos = [cx, cy, cz];
					let base = [cx * cs, cy * cs, cz * cs];
					if shape(base, cs) == Coverage::None {
						continue;
					}
					if let Some(ci) =
						Self::entry_to_chunk_idx(self.entries[self.entry_idx(chunk_pos)])
					{
						let [bx, by, bz] = base;
						self.chunks[ci]
							.clear_shape(|lb, s| shape([lb[0] + bx, lb[1] + by, lb[2] + bz], s));
					}
				}
			}
		}
	}

	/// Queue a shape fill on every chunk the shape touches.
	pub fn queue_fill_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F, voxel: Voxel) {
		let cs = self.chunk_size();
		let [dx, dy, dz] = self.dims;
		for cz in 0..dz {
			for cy in 0..dy {
				for cx in 0..dx {
					let chunk_pos = [cx, cy, cz];
					let base = [cx * cs, cy * cs, cz * cs];
					if shape(base, cs) == Coverage::None {
						continue;
					}
					let entry_idx = self.entry_idx(chunk_pos);
					if self.entries[entry_idx] == 0 {
						self.insert_chunk(chunk_pos, Chunk::with_depth(self.depth));
					}
					let ci = Self::entry_to_chunk_idx(self.entries[entry_idx]).unwrap();
					let [bx, by, bz] = base;
					self.chunks[ci].queue_fill_shape(
						|lb, s| shape([lb[0] + bx, lb[1] + by, lb[2] + bz], s),
						voxel,
					);
				}
			}
		}
	}

	/// Queue a shape clear on every chunk the shape touches.
	pub fn queue_clear_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F) {
		let cs = self.chunk_size();
		let [dx, dy, dz] = self.dims;
		for cz in 0..dz {
			for cy in 0..dy {
				for cx in 0..dx {
					let chunk_pos = [cx, cy, cz];
					let base = [cx * cs, cy * cs, cz * cs];
					if shape(base, cs) == Coverage::None {
						continue;
					}
					if let Some(ci) =
						Self::entry_to_chunk_idx(self.entries[self.entry_idx(chunk_pos)])
					{
						let [bx, by, bz] = base;
						self.chunks[ci].queue_clear_shape(|lb, s| {
							shape([lb[0] + bx, lb[1] + by, lb[2] + bz], s)
						});
					}
				}
			}
		}
	}

	pub fn flush_edits(&mut self) -> bool {
		self.chunks
			.par_iter_mut()
			.map(|c| c.flush_edits())
			.reduce(|| false, |a, b| a | b)
	}

	pub fn has_pending_edits(&self) -> bool {
		self.chunks.iter().any(|c| c.has_pending_edits())
	}

	pub fn flush_edits_budgeted(&mut self, budget: Duration) -> bool {
		let start = Instant::now();
		let mut any = false;
		for chunk in &mut self.chunks {
			if !chunk.has_pending_edits() {
				continue;
			}
			any |= chunk.flush_edits();
			if start.elapsed() >= budget {
				break;
			}
		}
		any
	}

	pub fn iter_chunks(&self) -> impl Iterator<Item = ([u32; 3], &Chunk)> {
		let [dx, dy, _] = self.dims;
		self.entries
			.iter()
			.enumerate()
			.filter_map(move |(i, &raw)| {
				let chunk_idx = Self::entry_to_chunk_idx(raw)?;
				let i = i as u32;
				Some((
					[i % dx, (i / dx) % dy, i / (dx * dy)],
					&self.chunks[chunk_idx],
				))
			})
	}

	fn insert_chunk(&mut self, chunk_pos: [u32; 3], chunk: Chunk) {
		let entry_idx = self.entry_idx(chunk_pos);
		self.chunks.push(chunk);
		self.entries[entry_idx] = self.chunks.len() as u32;
	}

	fn resolve(&self, world_pos: [u32; 3]) -> Option<(usize, [u32; 3])> {
		let (chunk_pos, local_pos) = self.split_world_pos(world_pos);
		if !self.in_bounds(chunk_pos) {
			return None;
		}
		let chunk_idx = Self::entry_to_chunk_idx(self.entries[self.entry_idx(chunk_pos)])?;
		Some((chunk_idx, local_pos))
	}

	fn split_world_pos(&self, world_pos: [u32; 3]) -> ([u32; 3], [u32; 3]) {
		let s = self.chunk_size();
		(world_pos.map(|v| v / s), world_pos.map(|v| v % s))
	}

	fn entry_to_chunk_idx(entry: u32) -> Option<usize> {
		if entry == 0 {
			None
		} else {
			Some((entry - 1) as usize)
		}
	}

	fn chunk_size(&self) -> u32 {
		4u32.pow(self.depth as u32)
	}

	fn in_bounds(&self, chunk_pos: [u32; 3]) -> bool {
		let [x, y, z] = chunk_pos;
		x < self.dims[0] && y < self.dims[1] && z < self.dims[2]
	}

	fn entry_idx(&self, chunk_pos: [u32; 3]) -> usize {
		let [x, y, z] = chunk_pos;
		(x + y * self.dims[0] + z * self.dims[0] * self.dims[1]) as usize
	}
}
