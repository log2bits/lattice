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

1. Each chunk is a sparse voxel 64-tree (not an octree), so nodes encode 64 children instead of 8, cutting per-voxel overhead
2. Per-chunk SVDAG: after each rebuild, identical nodes within a level are deduplicated bottom-up. Interior regions collapse to zero leaf nodes (uniform terminal propagates up). Surface patterns vary: a sphere of radius 8 has ~38 unique leaf patterns; a sphere of radius 32 has ~736. Compression scales with how much structural repetition the geometry has.
3. A `terminal_mask` per node marks subtrees that terminate early (uniform material), avoiding unnecessary descent
4. The `materials` array at every level is fully packed with no gaps: terminal slots store the exact or collapsed material, non-terminal slots store the LOD representative. No separate LOD field needed
5. `node_children` and `materials` share a single `children_offset` with lock-step indexing. Terminal slots in `node_children` hold zero; only non-terminal slots carry child indices
6. Per-chunk material tables deduplicate on the full 32-bit voxel value
7. Bitpacked widths are powers of 2 and scale with content: 4 bits for ≤16 materials, 8 for ≤256, etc.
8. SoA layout per level: GPU warps reading one field across many nodes hit contiguous memory
9. Chunk levels stored top-down (coarsest first) so partial file reads for LOD can stop early

### Editing

1. Edits are queued and flushed in batches each frame
2. Edit-aware partial rebuild: sort edits by tree-order key, then walk top-down. For each node, find which children have at least one edit in their address range. Children with no edits are copied by index unchanged. Children with edits are recursed into, producing new nodes appended to the existing level arrays. Cost is O(depth × edits).
3. Terminal subtrees are always copied by index, never expanded to individual voxels.
4. Dirty-node tracking in a DAG doesn't work cleanly - a single canonical node can be shared by many parents, so any in-place modification would require copy-on-write to un-share it first. The path-walk approach sidesteps this entirely: new nodes are appended and old orphans are swept by the canonicalize pass.
5. Canonicalize (SVDAG dedup + compaction) after each flush - rebuilds level arrays with only reachable canonical nodes, removing the orphaned nodes left by the partial rebuild.

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

`materials`: fully packed, one entry per occupied slot. Unified across all levels - at the leaf level these are the actual voxel colors; at higher levels they are the LOD representative for that subtree. Terminal slots hold the exact uniform material (which is also the LOD). Non-terminal slots hold the mode (most common material) of the children's `materials` entries, computed bottom-up during construction. No zeros, no gaps.

`node_children`: lock-step with `materials`. Non-terminal slots hold child node indices; terminal slots hold zero.

At the leaf level all slots are terminal and `node_children` is empty - it carries no data.

---

## Tree Construction

Built bottom-up from tree-order sorted voxels. Leaf nodes first, then parents from groups of 64 children. If all 64 children share the same material, the group collapses to a single terminal entry in the parent - no node is allocated.

After construction, a canonicalization pass deduplicates identical nodes level-by-level, bottom-up. Two nodes are identical if they have the same `occupancy_mask`, `terminal_mask`, and child values at every occupied slot (with already-remapped indices from deeper levels, so comparison is structurally exact). Duplicates are merged to their canonical copy; parent `node_children` references are remapped. The pass rebuilds each level's arrays from scratch containing only reachable canonical nodes, so it also compacts away orphaned nodes left by partial rebuilds. This is the per-chunk SVDAG reduction.

---

## World

Flat 3D grid of independent chunks. No cross-chunk node sharing - this would break per-chunk LOD streaming. At depth 4 each chunk covers 256³ voxels. The grid expands dynamically as the player explores.

Most chunks are ephemeral: generated procedurally on demand and not stored. When a chunk receives its first voxel edit, the full procedural content is generated and baked together with the edit into a single SVDAG, which is then persisted. From that point the chunk is stored as a pure SVDAG - the procedural recipe is no longer used. The SVDAG is the ground truth, not a separate edit log on top of a procedural base.

---

## Terrain

Procedural terrain is generated chunk by chunk on demand. The solid collapse optimization makes this extremely efficient: below the surface is a large uniform solid region, above is air - both terminate high in the tree. Only the thin surface layer needs full leaf resolution.

Terrain is generated via noise-based heightmaps with erosion and layered geology. The shape edit API drives generation: for each chunk, test columns against the heightmap using the AABB walk, collapsing solid and empty regions immediately without ever expanding them to individual voxels.

Material transitions between geology layers use a dithered boundary: instead of a hard horizontal cut, each voxel samples `hash(world_pos) < blend_factor(height)` to decide which material it belongs to. This produces natural-looking stochastic transitions at zero memory cost, since the hash is computed on the fly during generation.

Terrain features like caves, boulders, and overhangs are driven by the same parameterized shape API used for edits (spheres, capsules, SDFs), making them first-class citizens of the generation pipeline rather than special cases.

---

## LOD

LOD is implemented as a cascade of larger chunks, not as partial uploads of a single tree.

A depth-4 chunk covers 256³ voxels with leaf resolution of 1³. A LOD-1 chunk is 4x larger spatially (1024³ coverage) using the same depth-4 structure, so each leaf covers 4³ original voxels. LOD-2 covers 4096³ with each leaf covering 16³ original voxels, and so on.

The 4x scale factor is intentional: the 64-tree's internal levels are already 4x steps (level 0 = 1 voxel, level 1 = 4³, level 2 = 16³, level 3 = 64³). So a LOD-1 leaf corresponds exactly to a level-2 internal node in the original chunk. The representative material for that node is already computed during flush (`node_lod` walks the children and picks the most common material). Building a LOD chunk from a set of full-res chunks is just lifting their level-2 nodes out and using them as leaves - no fresh downsampling pass needed.

When an edited chunk is re-flushed, the LOD chunk covering that region is rebuilt from the updated full-res SVDAG.

For rendering, the GPU simply traverses the LOD cascade by distance. The existing "no uploaded children → shade a solid cube" fallback applies here too: distant chunks are LOD chunks, which are already coarse SVDAGs with the same structure as any other chunk.

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
