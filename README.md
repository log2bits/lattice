# Lattice

A voxel renderer with path tracing, built in Rust + WebGPU.

---

# Priorities

1. Extremely fast ray traversal
2. Wonderful compression
3. Moderately fast editability

---

# Optimizations

### Storage

1. Each chunk is an ESVC (Efficient Sparse Voxel Contree): a sparse 64-tree (not an octree) where nodes encode 64 children instead of 8, cutting per-voxel overhead
2. A `terminal_mask` per node marks subtrees that terminate early (uniform material), avoiding unnecessary descent
3. `materials` is fully packed with one entry per occupied slot and does double duty: terminal slots store the exact uniform material, non-terminal slots store the LOD representative. One array, no separate LOD field, no gaps.
4. `node_children` and `materials` share a single `children_offset` with lock-step indexing. Terminal slots in `node_children` hold zero; only non-terminal slots carry child indices
5. Per-chunk material tables deduplicate on the full 32-bit voxel value
6. Bitpacked widths are powers of 2 and scale with content: 4 bits for ≤16 materials, 8 for ≤256, etc.
7. SoA layout per level: GPU warps reading one field across many nodes hit contiguous memory
8. Chunk levels stored top-down (coarsest first) so partial file reads for LOD can stop early

### Editing

1. Edits are queued and flushed in batches each frame
2. Edit-aware partial rebuild: sort edits by tree-order key, then walk top-down. For each node, find which children have at least one edit in their address range. Children with no edits are copied by index unchanged. Children with edits are recursed into, producing new nodes appended to the existing level arrays. Cost is O(depth × edits).
3. Terminal subtrees are always copied by index, never expanded to individual voxels.
4. New nodes are appended and old orphans are left in place during the walk, then swept in the compaction pass.
5. Compact after each flush - rebuilds level arrays with only reachable nodes, removing orphans left by partial rebuilds.

### Shape Editing

Individual voxel edits are just one case. The edit walk generalizes to any shape with an AABB test:

- **Full coverage** (node AABB entirely inside shape): set subtree to terminal with the fill material, no recursion
- **No coverage** (node AABB entirely outside shape): copy subtree unchanged, no recursion
- **Partial coverage**: recurse into children

This gives O(surface area) cost for any shape - a 64³ rectangular fill touches a handful of border nodes rather than 262k individual voxels. Works for rectangles, spheres, capsules, or any SDF. Heightmap terrain generation is a column-wise variant of the same walk.

### Rendering

1. Rendered with ray tracing + beam optimization
2. Partial tree upload: CPU sends only the top N levels per chunk to VRAM based on camera distance
3. LOD cutoff: when a node has no uploaded children, the GPU reads `materials[child_idx]` on the current node and renders a solid cube instead of descending
4. Traversal uses an ancestor stack caching parent node indices, so stepping into neighbor cells doesn't restart from root
5. Coarse occupancy groups the 64-bit mask into 8 regions of 2×2×2, enabling 8-cell skips over empty space
6. Coordinate flipping maps all rays into the negative octant, halving branch count in the DDA inner loop
7. Camera position is stored as integer chunk coordinates plus a chunk-local `vec3` offset always within `[0, 256]` voxels. All ray traversal happens in chunk-local space: when a ray exits a chunk the integer chunk coords are stepped and the ray origin is re-expressed relative to the new chunk. No large floats ever enter the traversal math, so f32 is sufficient for local coordinates regardless of world size.

### Later (path tracing)

1. Bidirectional: rays from camera and rays from light sources (sun, emissive voxels)
2. Emissive voxels discovered by camera rays get added to the light list dynamically
3. Per-face unique ID with lighting averaged across that face, doing spatial and temporal accumulation in one pass
4. Averaging strength tied to roughness: mirror faces accumulate slowly, diffuse faces accumulate aggressively
5. Faces not updated recently get evicted from the lighting buffer

---

## Voxel

```
bits 31-8   rgb          24-bit linear RGB color
bits  7-4   roughness    0 = mirror, 15 = fully diffuse
bit   3     emissive     emits light at its albedo color
bit   2     metallic     albedo tints specular
bit   1     transparent  refracts rather than reflects
bit   0     reserved
```

---

## Level

```rs
pub struct Level {
    pub occupancy_mask: Vec<u64>,
    pub terminal_mask: Vec<u64>,
    pub children_offset: Vec<u32>,
    pub node_children: BitpackedArray,
    pub materials: BitpackedArray,
}
```

`occupancy_mask`: which of 64 slots are occupied per node.

`terminal_mask`: which occupied slots terminate here. If set, `materials[child_idx]` is the material. If unset, `node_children[child_idx]` is the child node index to descend into.

`children_offset`: start of this node's child block in both arrays. Child index = `children_offset[node] + popcount(occupancy_mask & ((1 << slot) - 1))`.

`materials`: fully packed, one entry per occupied slot. Unified across all levels - at the leaf level these are the actual voxel colors; at higher levels they are the LOD representative for that subtree. Terminal slots hold the exact uniform material (which is also the LOD). Non-terminal slots hold the most common material among the children's `materials` entries, computed bottom-up during construction. Ties go to the first material. This means edits that don't change the dominant material in a subtree don't propagate LOD changes all the way up the tree. No zeros, no gaps.

`node_children`: lock-step with `materials`. Non-terminal slots hold child node indices; terminal slots hold zero.

At the leaf level all slots are terminal and `node_children` is empty - it carries no data.

---

## Tree Construction

Built bottom-up from tree-order sorted voxels. Leaf nodes first, then parents from groups of 64 children. If all 64 children share the same material, the group collapses to a single terminal entry in the parent - no node is allocated.

After construction, a compaction pass rebuilds each level's arrays from scratch containing only reachable nodes, removing orphans left by partial rebuilds.

---

## World

Chunks are stored in a hashmap keyed on integer (x, y, z) chunk coordinates, expanding lazily as the player explores. No cross-chunk node sharing - this would break per-chunk LOD streaming. At depth 4 each chunk covers 256³ voxels.

There are two distinct edit types:

**Procedural edits** (terrain, caves, boulders, etc.) are stored as a global ordered list of parameterized operations. Each operation has a bounding volume and a resolution level parameter. When any chunk is generated, the list is spatially queried for overlapping ops and they are applied in order using the shape edit API. At coarser LOD levels, operations skip sub-voxel detail: a high-frequency noise layer that produces centimeter-scale variation contributes nothing to a LOD chunk whose voxels are meters wide, so it is culled by level. The list is the authoritative world recipe and is always re-runnable.

**Voxel edits** are individual voxel writes from player interaction. These are not stored as a log. Instead, the first voxel edit to a chunk triggers creation of a **persistent chunk**: the full procedural content is baked at that point, the voxel edit is applied on top, and the resulting ESVC is stored permanently. Subsequent voxel edits to that chunk use the existing partial-rebuild machinery directly on the stored ESVC. The ESVC is the ground truth - the procedural recipe is not re-run after baking.

Most chunks are **ephemeral**: they are generated on demand from the procedural list and discarded when out of range. Only persistent chunks are stored on disk.

When a new procedural edit is added whose bounding volume overlaps an existing persistent chunk, it is applied immediately to that chunk's ESVC using the shape edit API. Player voxels outside the op's coverage are untouched. The op is also appended to the procedural list so future ephemeral chunk generation includes it. Order semantics are append-only: retroactive ops apply on top of current chunk state rather than re-running the full list, so they should logically follow everything already in the list (e.g., a cave op added after terrain generation cuts into whatever is already there).

---

## Terrain

Procedural terrain is generated chunk by chunk on demand. The solid collapse optimization makes this extremely efficient: below the surface is a large uniform solid region, above is air - both terminate high in the tree. Only the thin surface layer needs full leaf resolution.

Terrain is generated via noise-based heightmaps with erosion and layered geology. The shape edit API drives generation: for each chunk, test columns against the heightmap using the AABB walk, collapsing solid and empty regions immediately without ever expanding them to individual voxels.

Material transitions between geology layers use a dithered boundary: instead of a hard horizontal cut, each voxel samples `hash(world_pos) < blend_factor(height)` to decide which material it belongs to. This produces natural-looking stochastic transitions at zero memory cost, since the hash is computed on the fly during generation.

Terrain features like caves, boulders, and overhangs are driven by the same parameterized shape API, making them entries in the procedural edit list rather than special cases. Each operation is LOD-aware: at coarse levels the operation's resolution parameter skips noise layers and detail passes whose contribution would be sub-voxel at that scale.

---

## LOD

LOD is implemented as a cascade of larger chunks, not as partial uploads of a single tree.

A depth-4 chunk covers 256³ voxels with leaf resolution of 1³. A LOD-1 chunk is 4x larger spatially (1024³ coverage) using the same depth-4 structure, so each leaf covers 4³ original voxels. LOD-2 covers 4096³ with each leaf covering 16³ original voxels, and so on.

The 4x scale factor is intentional: the 64-tree's internal levels are already 4x steps (level 0 = 1 voxel, level 1 = 4³, level 2 = 16³, level 3 = 64³). So a LOD-1 leaf corresponds exactly to a level-2 internal node in the original chunk. The representative material for that node is already computed during flush (`node_lod` walks the children and picks the most common material). Building a LOD chunk from its base chunks has two paths depending on whether each base chunk is persistent or ephemeral. For persistent chunks, level-2 nodes are lifted out directly - no recomputation needed. For ephemeral chunks, the procedural edit list is run at LOD-1 resolution: operations apply their coarse-level logic and sub-voxel detail is skipped. The two paths produce compatible node data and are merged into a single LOD ESVC.

When a persistent chunk is re-flushed, the LOD chunk covering that region is rebuilt from the updated ESVC.

For rendering, the GPU simply traverses the LOD cascade by distance. The existing "no uploaded children → shade a solid cube" fallback applies here too: distant chunks are LOD chunks, which are already coarse ESVCs with the same structure as any other chunk.

---

## Resources

### References

| Reference | Why it matters |
|---|---|
| [Guide to sparse 64-trees](https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing/) | The traversal algorithm. Ancestor stack, coarse occupancy, flipped coordinates. |
| [Aokana (2505.02017)](https://arxiv.org/abs/2505.02017) | Chunked SVDAG with LOD streaming. Validates the shallow-tree-per-chunk approach. |
| [Hybrid Voxel Formats (2410.14128)](https://arxiv.org/abs/2410.14128) | Systematic comparison of voxel storage formats and their tradeoffs. |
| [High Resolution SVDAGs](https://icg.gwu.edu/sites/g/files/zaxdzs6126/files/downloads/highResolutionSparseVoxelDAGs.pdf) | Original SVDAG paper. Bottom-up DAG reduction, GPU traversal. |
| [Efficient Sparse Voxel Octrees](https://www.researchgate.net/publication/47645140_Efficient_Sparse_Voxel_Octrees) | Laine & Karras. Foundation for SVO traversal and beam optimization. |
| [Voxelis Bible](https://github.com/WildPixelGames/voxelis) | SVO-DAG deep dive: batching, CoW, SoA, LOD, hash consing. |
| [Amanatides & Woo DDA](http://www.cse.yorku.ca/~amana/research/grid.pdf) | The DDA algorithm for voxel ray traversal. |
| [Compressing color data for voxels (Dolonius 2017)](https://dl.acm.org/doi/10.1145/3023368.3023381) | DFS-order color arrays, block compression for SVDAG attributes. |
| [Fast and Gorgeous Erosion Filter](https://blog.runevision.com/2026/03/fast-and-gorgeous-erosion-filter.html) | Per-point erosion filter (no simulation). Evaluates in isolation, LOD-friendly, outputs height + derivatives + ridge map. |

### Channels

| Channel | Focus |
|---|---|
| [Douglas Dwyer](https://www.youtube.com/@DouglasDwyer) | Octo voxel engine, Rust + WebGPU, path-traced GI |
| [John Lin (Voxely)](https://www.youtube.com/@johnlin) | Path-traced voxel sandbox, RTX |
| [Gabe Rundlett](https://www.youtube.com/@GabeRundlett) | C++ voxel engine, Daxa/Vulkan |
| [Ethan Gore](https://www.youtube.com/@EthanGore) | Voxel engine dev, binary greedy meshing |
| [VoxelRifts](https://www.youtube.com/@VoxelRifts) | Voxel programming explainers |
| [SimonDev](https://www.youtube.com/@simondev758) | Radiance Cascades intro |

### Projects

| Project | Description |
|---|---|
| [voxquant](https://github.com/) | glTF voxelizer, source of rasterization algorithms |
| [VoxelRT](https://github.com/dubiousconst282/VoxelRT) | Tree64, brickmap, XBrickMap benchmarks |
| [Voxelis](https://github.com/WildPixelGames/voxelis) | Rust SVO-DAG with batching, CoW, LOD |
| [Octo Engine](https://github.com/DouglasDwyer/octo-release) | Rust + WebGPU voxel engine |
| [tree64](https://github.com/expenses/tree64) | Rust sparse 64-tree with hashing |
| [HashDAG](https://github.com/Phyronnaz/HashDAG) | HashDAG reference implementation |
| [gvox](https://github.com/GabeRundlett/gvox) | Voxel format translation library |

### More

| Resource | Description |
|---|---|
| [Voxel.Wiki](https://voxel.wiki) | Community hub for voxel rendering resources |
| [Voxely.net blog](https://voxely.net/blog/) | John Lin's voxel engine design posts |
| [A Rundown on Brickmaps](https://uygarb.dev/posts/0003_brickmap_rundown/) | Brickmap/brickgrid explanation |
| [Radiance Cascades 3D (ShaderToy)](https://www.shadertoy.com/view/X3XfRM) | Surface-based 3D radiance cascades |
| [Branchless DDA (ShaderToy)](https://www.shadertoy.com/view/XdtcRM) | Clean branchless 3D DDA reference |
