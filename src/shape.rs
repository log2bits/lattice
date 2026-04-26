mod rect;
mod sphere;
mod terrain;

pub use rect::Rect;
pub use sphere::Sphere;
pub use terrain::Terrain;

use crate::{
	tree::{Aabb, EditPacket, TreePath},
	types::Voxel,
};

pub enum Coverage {
	Full(Voxel),
	Partial,
	Empty,
}

pub trait Shape: Send + Sync {
	fn aabb(&self) -> Aabb;
	// Given a node's world-space AABB and its depth level, classify coverage.
	// At leaf level the AABB covers exactly one voxel.
	fn coverage(&self, node_aabb: Aabb, level: u8) -> Coverage;
}

pub fn edit_packet_for_shape<const DEPTH: usize>(
	shape: &dyn Shape,
	root_aabb: Aabb,
) -> EditPacket<DEPTH> {
	assert!(
		DEPTH <= u8::MAX as usize,
		"shape edit packets only support depths up to {}",
		u8::MAX
	);

	let mut packet = EditPacket::new(true);
	let shape_aabb = shape.aabb();

	if !shape_aabb.overlaps(&root_aabb) {
		return packet;
	}

	let mut path = [0u8; DEPTH];
	collect_shape_edits(
		shape,
		shape_aabb,
		root_aabb,
		DEPTH as u8,
		0,
		&mut path,
		&mut packet,
	);

	packet
}

fn collect_shape_edits<const DEPTH: usize>(
	shape: &dyn Shape,
	shape_aabb: Aabb,
	node_aabb: Aabb,
	level: u8,
	depth: usize,
	path: &mut [u8; DEPTH],
	packet: &mut EditPacket<DEPTH>,
) {
	if !shape_aabb.overlaps(&node_aabb) {
		return;
	}

	match shape.coverage(node_aabb, level) {
		Coverage::Empty => {}
		Coverage::Full(voxel) => push_shape_edit(path, depth, voxel, packet),
		Coverage::Partial => {
			if level == 0 {
				debug_assert!(
					false,
					"shape returned partial coverage at leaf level; leaf coverage must resolve to full or empty"
				);
				return;
			}

			for slot in 0u8..64 {
				path[depth] = slot + 1;
				collect_shape_edits(
					shape,
					shape_aabb,
					node_aabb.split_at_slot(slot as u32),
					level - 1,
					depth + 1,
					path,
					packet,
				);
			}
		}
	}
}

fn push_shape_edit<const DEPTH: usize>(
	path: &[u8; DEPTH],
	depth: usize,
	voxel: Voxel,
	packet: &mut EditPacket<DEPTH>,
) {
	let mut buf = [0u8; DEPTH];
	buf[..depth].copy_from_slice(&path[..depth]);
	packet.paths.push(TreePath::from_packed(buf));
	packet.values.push(packet.lut.get_or_add(voxel.into()));
}
