use super::{Chunk, Edit, EditPacket};
use crate::voxel::Voxel;

/// Returned by shape closures to classify how a node's AABB relates to the shape.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Coverage {
	/// AABB is entirely inside the shape.
	Full,
	/// AABB overlaps the shape boundary — recurse into children.
	Partial,
	/// AABB is entirely outside the shape.
	None,
}

fn shape_to_edits<F: Fn([u32; 3], u32) -> Coverage>(
	shape: &F,
	depth: u8,
	fill: Option<Voxel>,
) -> Vec<Edit> {
	let mut edits = Vec::new();
	walk(shape, depth, 0, [0, 0, 0], fill, &mut edits);
	edits
}

fn walk<F: Fn([u32; 3], u32) -> Coverage>(
	shape: &F,
	depth: u8,
	level_idx: u8,
	base: [u32; 3],
	fill: Option<Voxel>,
	edits: &mut Vec<Edit>,
) {
	let slot_side = 4u32.pow((depth - 1 - level_idx) as u32);
	let is_leaf = level_idx == depth - 1;
	for slot in 0..64u32 {
		let offset = slot_offset(slot, slot_side);
		let slot_base = [
			base[0] + offset[0],
			base[1] + offset[1],
			base[2] + offset[2],
		];
		match shape(slot_base, slot_side) {
			Coverage::None => {}
			Coverage::Full => edits.push(Edit {
				pos: slot_base,
				level: level_idx,
				fill,
			}),
			Coverage::Partial => {
				if is_leaf {
					edits.push(Edit {
						pos: slot_base,
						level: level_idx,
						fill,
					});
				} else {
					walk(shape, depth, level_idx + 1, slot_base, fill, edits);
				}
			}
		}
	}
}

fn slot_offset(slot: u32, slot_side: u32) -> [u32; 3] {
	[
		(slot & 3) * slot_side,
		((slot >> 2) & 3) * slot_side,
		((slot >> 4) & 3) * slot_side,
	]
}

impl Chunk {
	pub fn fill_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F, voxel: Voxel) {
		self.push_shape_packet(&shape, Some(voxel));
		self.flush_edits();
	}

	pub fn clear_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F) {
		self.push_shape_packet(&shape, None);
		self.flush_edits();
	}

	pub fn queue_fill_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F, voxel: Voxel) {
		self.push_shape_packet(&shape, Some(voxel));
	}

	pub fn queue_clear_shape<F: Fn([u32; 3], u32) -> Coverage>(&mut self, shape: F) {
		self.push_shape_packet(&shape, None);
	}

	fn push_shape_packet<F: Fn([u32; 3], u32) -> Coverage>(
		&mut self,
		shape: &F,
		fill: Option<Voxel>,
	) {
		let edits = shape_to_edits(shape, self.depth(), fill);
		if !edits.is_empty() {
			self.pending.push(EditPacket {
				edits,
				presorted: true,
			});
			self.svdag_clean = false;
		}
	}
}
