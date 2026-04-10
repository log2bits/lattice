use crate::tree::Lattice;

/// GPU buffers for the full Lattice. NodePool depths are contiguous and independently updatable.
pub struct GpuBuffers {
	// wgpu buffers
}

impl GpuBuffers {
	pub fn new(device: &wgpu::Device, lattice: &Lattice) -> Self {
		todo!()
	}

	/// Diff target_depths against currently uploaded depths and re-upload changed chunks.
	pub fn update(&mut self, lattice: &Lattice, target_depths: &[u8], device: &wgpu::Device, queue: &wgpu::Queue) {
		todo!()
	}
}
