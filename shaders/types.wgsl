// Shared type definitions imported by all other shaders.
// Mirrors the Rust types: NodePool SoA layout, Voxel bit layout, grid entry flags.

struct Voxel {
	// bits 31-8: rgb, bits 7-4: roughness, bit 3: emissive, bit 2: metallic, bit 1: transparent
	value: u32,
}

struct NodePool {
	// occupancy, solid_mask, children_offset, lod_material, node_children, leaf_materials
	// bound as separate storage buffers per depth level
}

// Grid entry flags
const PROXY_FLAG: u32 = 0x80000000u;
const SOLID_FLAG: u32 = 0x80000000u;

// Decode packed rgb from a voxel value.
fn voxel_rgb(v: u32) -> vec3<f32> {
	let r = f32((v >> 24u) & 0xffu) / 255.0;
	let g = f32((v >> 16u) & 0xffu) / 255.0;
	let b = f32((v >> 8u)  & 0xffu) / 255.0;
	return vec3<f32>(r, g, b);
}

fn voxel_roughness(v: u32) -> f32 {
	return f32((v >> 4u) & 0xfu) / 15.0;
}

fn voxel_emissive(v: u32) -> bool {
	return (v & (1u << 3u)) != 0u;
}

// Decode a slot index to 3D coords within a 4x4x4 block.
fn slot_coords(slot: u32) -> vec3<u32> {
	return vec3<u32>(slot & 3u, (slot >> 2u) & 3u, slot >> 4u);
}

// Compress 64 occupancy bits into 8 coarse 2x2x2 region bits.
fn coarse_occupancy(occ: u64) -> u32 {
	// todo
	return 0u;
}
