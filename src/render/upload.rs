use crate::{tree::Tree, world::ChunkPool};

pub fn upload_world_tree(queue: &wgpu::Queue, buf: &wgpu::Buffer, tree: &Tree) { todo!() }

pub fn upload_chunks(
	device: &wgpu::Device,
	queue: &wgpu::Queue,
	data_buf: &wgpu::Buffer,
	offsets_buf: &wgpu::Buffer,
	pool: &ChunkPool,
	dirty: &[u32],
) { todo!() }
