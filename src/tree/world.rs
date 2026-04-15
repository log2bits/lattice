use crate::tree::Chunk;
use crate::voxel::Voxel;

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

	/// Insert a chunk at a chunk position.
	fn set(&mut self, chunk_pos: [u32; 3], chunk: Chunk) {
		if !self.in_bounds(chunk_pos) {
			eprintln!(
				"set: chunk_pos {:?} out of bounds {:?}",
				chunk_pos, self.dims
			);
			return;
		}
		let entry_idx = self.entry_idx(chunk_pos);
		self.chunks.push(chunk);
		self.entries[entry_idx] = self.chunks.len() as u32;
	}

	/// Get a chunk by chunk position.
	fn get(&self, chunk_pos: [u32; 3]) -> Option<&Chunk> {
		if !self.in_bounds(chunk_pos) {
			eprintln!(
				"get: chunk_pos {:?} out of bounds {:?}",
				chunk_pos, self.dims
			);
			return None;
		}
		let chunk_idx = Self::entry_to_chunk_idx(self.entries[self.entry_idx(chunk_pos)])?;
		self.chunks.get(chunk_idx)
	}

	/// Returns true if a chunk exists at the given chunk position.
	fn contains(&self, chunk_pos: [u32; 3]) -> bool {
		self.in_bounds(chunk_pos)
			&& Self::entry_to_chunk_idx(self.entries[self.entry_idx(chunk_pos)]).is_some()
	}

	/// Read the voxel at a world-space voxel coordinate.
	pub fn get_voxel(&self, world_pos: [u32; 3]) -> Option<Voxel> {
		let (chunk_idx, local_pos) = self.resolve(world_pos)?;
		self.chunks[chunk_idx].get_voxel(local_pos)
	}

	/// Place a voxel at a world-space voxel coordinate, creating the chunk if needed.
	pub fn place_voxel(&mut self, world_pos: [u32; 3], voxel: Voxel) {
		let (chunk_pos, local_pos) = self.split_world_pos(world_pos);
		if !self.in_bounds(chunk_pos) {
			eprintln!("place_voxel: world_pos {:?} out of bounds", world_pos);
			return;
		}
		let entry_idx = self.entry_idx(chunk_pos);
		if self.entries[entry_idx] == 0 {
			self.set(chunk_pos, Chunk::new());
		}
		let chunk_idx =
			Self::entry_to_chunk_idx(self.entries[entry_idx]).expect("chunk was just inserted");
		self.chunks[chunk_idx].place_voxel(local_pos, voxel);
	}

	/// Remove a voxel at a world-space voxel coordinate.
	pub fn remove_voxel(&mut self, world_pos: [u32; 3]) {
		let Some((chunk_idx, local_pos)) = self.resolve(world_pos) else {
			return;
		};
		self.chunks[chunk_idx].remove_voxel(local_pos);
	}

	/// Iterate all occupied chunks with their chunk positions.
	pub fn iter_chunks(&self) -> impl Iterator<Item = ([u32; 3], &Chunk)> {
		let [dx, dy, _] = self.dims;
		self.entries
			.iter()
			.enumerate()
			.filter_map(move |(i, &raw)| {
				let chunk_idx = Self::entry_to_chunk_idx(raw)?;
				let i = i as u32;
				let chunk_pos = [i % dx, (i / dx) % dy, i / (dx * dy)];
				Some((chunk_pos, &self.chunks[chunk_idx]))
			})
	}

	/// Resolve a world-space voxel coordinate to a chunk index and local position.
	fn resolve(&self, world_pos: [u32; 3]) -> Option<(usize, [u32; 3])> {
		let (chunk_pos, local_pos) = self.split_world_pos(world_pos);
		if !self.in_bounds(chunk_pos) {
			return None;
		}
		let chunk_idx = Self::entry_to_chunk_idx(self.entries[self.entry_idx(chunk_pos)])?;
		Some((chunk_idx, local_pos))
	}

	/// Split a world-space coordinate into chunk position and local position within the chunk.
	fn split_world_pos(&self, world_pos: [u32; 3]) -> ([u32; 3], [u32; 3]) {
		let chunk_size = self.chunk_size();
		(
			world_pos.map(|v| v / chunk_size),
			world_pos.map(|v| v % chunk_size),
		)
	}

	fn entry_to_chunk_idx(entry: u32) -> Option<usize> {
		if entry == 0 {
			None
		} else {
			Some((entry - 1) as usize)
		}
	}

	fn chunk_size(&self) -> u32 {
		4_u32.pow(self.depth as u32)
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
