# Lattice

A voxel renderer with path tracing, built in Rust + WebGPU.

---

# Data Structure
Custom **NBEPSVCDAG**
- N - Nested (chunk trees are nested within one big world tree)
- B - Bitpacked (values and offsets use only as many bits as required)
- E - [Efficient](https://research.nvidia.com/sites/default/files/pubs/2010-02_Efficient-Sparse-Voxel/laine2010i3d_paper.pdf) (allows for leaf nodes anywhere in the tree)
- P - [Pointerless](https://www.cai.sk/ojs/index.php/cai/article/view/2020_3_587) (Implicit offsets are stored instead of explicit pointers)
- S - Sparse (only occupied nodes are stored, empty space is stored efficiently)
- V - Voxel (Volumetric Pixel)
- C - Tetrahexa**contree** (or [64-tree](https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing/))
- DAG - Directed Acyclic Graph

---

# Priorities

1. Extremely fast ray traversal
2. Wonderful compression
3. Moderately fast editability

---

# Optimizations

### Storage

1. Both the world and each chunk use the same sparse 64-tree structure (`Tree`). The world tree (28 levels deep) stores chunk pool handles at its leaves; chunk trees (4 levels deep) store material indices. Same traversal algorithm, same flat SoA layout, same GPU buffers.
2. A `leaf_mask` per node marks subtrees that are leaves (uniform value), avoiding unnecessary descent
3. `values` is fully packed with one entry per occupied slot and does double duty: leaf slots store the exact uniform value, non-leaf slots store the LOD representative. One array, no separate LOD field, no gaps.
4. `node_children` and `values` share a single `children_offset` with lock-step indexing. Leaf slots in `node_children` hold zero; only non-leaf slots carry child indices
5. Per-chunk material tables deduplicate on the full 32-bit voxel value. The world tree has no material table since its values are chunk handles, not materials.
6. Bitpacked widths are powers of 2 and scale with content: 4 bits for ≤16 values, 8 for ≤256, etc.
7. SoA layout per level: GPU warps reading one field across many nodes hit contiguous memory
8. Chunk levels stored top-down (coarsest first) so partial file reads for LOD can stop early

### Editing

1. Each chunk owns an `OrderedEdits`: an ordered list of `EditPacket`s applied sequentially. Later packets overwrite earlier ones. Each packet holds a `Vec<Edit>` and a `sorted` flag.
2. Shape edits (terrain, caves, etc.) produce pre-sorted packets from the coverage walk and are appended via `add_shape_packet`. Player voxel edits are appended to the last unsorted packet, or start a new one if the last packet is sorted. This preserves shape-vs-player ordering without merging the two streams.
3. `Edit` carries a position (`[u64; 3]`), a level (`0` = single voxel, `n` = 4^n voxel cube), and an optional value (`None` = remove). Level > 0 lets shape edits collapse entire uniform subtrees without expanding to individual voxels.
4. `flush_edits` sorts any unsorted packets by Morton key, then applies all packets to the tree in order. Cost is O(depth × edits) per packet.
5. Edit walk is top-down: for each node, find children with edits in their range. Children with no edits are copied unchanged. Children with edits are recursed into, producing new nodes appended to the level arrays.
6. Compact after each flush - rebuilds level arrays with only reachable nodes, removing orphans.

### Shape Editing

Individual voxel edits are just one case. The edit walk generalizes to any shape implementing the `Shape` trait:

```rs
pub trait Shape: Send + Sync {
    fn aabb(&self) -> Aabb;
    fn coverage(&self, node_aabb: Aabb, level: u8) -> Coverage;
}

pub enum Coverage {
    Full(Voxel),  // fill subtree, stop recursing
    Partial,      // recurse into children
    Empty,        // skip entirely
}
```

- **Full coverage** (node AABB entirely inside shape): set subtree to a leaf with the fill material, no recursion
- **No coverage** (node AABB entirely outside shape): copy subtree unchanged, no recursion
- **Partial coverage**: recurse into children

This gives O(surface area) cost for any shape - a 64³ rectangular fill touches a handful of border nodes rather than 262k individual voxels. Works for rectangles, spheres, capsules, or any SDF. Heightmap terrain generation is a column-wise variant of the same walk. At leaf level `coverage` is called per-voxel, so multi-material shapes (terrain geology layers, dithered boundaries) return different `Full(material)` values based on position.

### Rendering

1. Rendered with ray tracing + beam optimization
2. Partial tree upload: CPU sends only the top N levels per chunk to VRAM based on camera distance
3. LOD cutoff: when a node has no uploaded children, the GPU reads `values[child_idx]` on the current node and renders a solid cube instead of descending
4. Traversal uses an ancestor stack caching parent node indices, so stepping into neighbor cells doesn't restart from root
5. Coarse occupancy groups the 64-bit mask into 8 regions of 2×2×2, enabling 8-cell skips over empty space
6. Coordinate flipping maps all rays into the negative octant, halving branch count in the DDA inner loop
7. Camera position is stored as integer chunk coordinates plus a chunk-local `vec3` offset always within `[0, 256]` voxels. All ray traversal happens in chunk-local space: when a ray exits a chunk the integer chunk coords are stepped and the ray origin is re-expressed relative to the new chunk. No large floats ever enter the traversal math, so f32 is sufficient for local coordinates regardless of world size.
8. CPU traversal runs the same DDA algorithm for hit-testing: placing a shape traces a ray from the camera through the world and chunk trees to find the exact hit position in world voxel coordinates.

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

## Tree

The core data structure. Used for both the world tree and every chunk tree.

```rs
pub struct Tree<const DEPTH: usize> {
    pub occupied: bool,  // false = entire tree is empty
    pub is_leaf: bool,   // true = entire tree is one uniform material (value)
    pub value: u32,
    pub levels: [Level; DEPTH],
}

pub struct Level {
    pub occupancy_mask: Vec<u64>,
    pub leaf_mask: Vec<u64>,
    pub children_offset: Vec<u32>,
    pub node_children: BitpackedArray,
    pub values: BitpackedArray,
}
```

`occupied/is_leaf/value` represent the root above all levels. When `is_leaf` is false and `occupied` is true, the tree has structure in `levels`.

`levels[0]` holds the root node (one node after compact). Its 64 slot-children live in `levels[1]`. `levels[d]` holds nodes at tree depth `d`; `levels[DEPTH-1]` is the leaf-node level whose slots are individual voxels.

`occupancy_mask`: which of 64 slots are occupied per node.

`leaf_mask`: which occupied slots are leaves. If set, `values[child_idx]` is the leaf value. If unset, `node_children[child_idx]` is the child node index to descend into.

`children_offset`: start of this node's child block in both arrays. Child index = `children_offset[node] + popcount(occupancy_mask & ((1 << slot) - 1))`.

`values`: fully packed, one entry per occupied slot. Leaf slots hold the leaf value. Non-leaf slots hold the LOD representative (first occupied child's value, bottom-up). No zeros, no gaps.

`node_children`: lock-step with `values`. Non-leaf slots hold child node indices; leaf slots hold zero. Empty at the leaf level.

---

## Chunk Tree

A depth-4 `Tree` covering 256³ voxels. Chunk depth is fixed - depth-4 is the only chunk size used at every LOD level. Positions within a chunk are always `[u8; 3]` (each component 0-255), keeping chunk-local math cheap and free of range checks. World-space positions use `[i64; 3]` only when crossing chunk boundaries.

Terminal `values` are indices into a per-chunk `MaterialTable` (a deduplicated list of 32-bit voxel values). The `MaterialTable` lives alongside the tree, not inside it.

```rs
pub const DEPTH: u8 = 4;
pub const SIDE: u32 = 256; // 4^DEPTH

pub struct Chunk {
    pub tree: Tree,
    pub materials: MaterialTable,
}
```

---

## World Tree

A depth-28 `Tree` covering the full 2^64 voxel world. Terminal `values` are chunk pool handles (u32 index into `ChunkPool`; `u32::MAX` = empty). No material table.

Tree depth encodes LOD: a leaf at depth 28 is a LOD-0 chunk (256³ voxels, 1³ per leaf voxel). A leaf at depth 20 is a LOD-8 chunk (the same depth-4 ESVC structure, but each leaf voxel covers 4^8 original voxels). The world tree is maintained by the CPU LOD system using a points-of-interest walk; see the LOD section.

GPU ray traversal descends the world tree using the same DDA and ancestor stack as per-chunk traversal. On hitting a terminal node, the shader reads the chunk handle, looks up its byte offset in the chunk offset table, and continues traversal in that chunk's tree. Both trees use the same flat SoA buffers, so no structural switch is needed mid-ray.

---

## Tree Construction

Trees start empty and grow entirely through `apply_edits(OrderedEdits)`. There is no separate build path: initial generation, player edits, LOD aggregation, and voxel imports all go through the same edit system. A voxel import is just an empty tree with the imported voxels applied as an unsorted `EditPacket`.

---

## World

The world owns a `WorldTree` (depth-28 `Tree`), a `ChunkPool` (flat pool of loaded chunks indexed by u32 handle), a shape edit list, and a map of persistent chunks.

```rs
pub struct World {
    pub world_tree: Tree,
    pub pool: ChunkPool,
    pub shape_edits: Vec<ShapeEdit>,
    pub persistent_chunks: HashMap<[i64; 3], PersistentChunk>,
}

pub enum PersistentChunk {
    Resident(Chunk), // in CPU memory; out of LOD-0 range, not in pool
    Active(u32),     // handle into pool; chunk lives there at full resolution
}

pub struct ShapeEdit {
    pub aabb: Aabb,   // cached from shape.aabb() for O(1) per-chunk rejection
    pub min_lod: u8,
    pub shape: Box<dyn Shape>,
}
```

No cross-chunk node sharing - this would break per-chunk LOD streaming.

`PersistentChunk::Active` means the chunk is in the pool at full (LOD-0) resolution with no duplication: the pool slot IS the persistent chunk, and the map just holds its handle. When the camera moves away and the area coarsens, the chunk is pulled out of the pool into `Resident`, a new derived LOD chunk is built by aggregating it, and that derived chunk takes the pool slot instead. The derived chunk is ephemeral; the `Resident` data remains the ground truth.

There are two distinct edit types:

**Shape edits** (terrain, caves, boulders, etc.) are stored as a global ordered list of `ShapeEdit` entries. Each entry stores a tight axis-aligned bounding box in world voxel coordinates, a minimum LOD level, and a boxed `Shape`. When generating a chunk, the list is filtered by AABB overlap before invoking any shape logic - for flat terrain whose AABB has a small Y range, sky chunks are rejected with a single comparison. The list is the authoritative world recipe and is always re-runnable. For large edit counts the list can be indexed with a BVH for O(log n) per-chunk queries.

**Voxel edits** are individual voxel writes from player interaction. These are not stored as a log. Instead, the first voxel edit to a chunk triggers creation of a **persistent chunk**: the full shape edit content is baked at that point, the voxel edit is applied on top, and the resulting tree is stored permanently. Subsequent voxel edits to that chunk use the existing partial-rebuild machinery directly on the stored tree. The tree is the ground truth - the shape edit list is not re-run after baking.

Most chunks are **ephemeral**: generated on demand from the shape edit list and discarded when out of range. Only persistent chunks are stored on disk.

When a new shape edit is added whose AABB overlaps an existing persistent chunk, it is applied immediately to that chunk's tree. Player voxels outside the shape's coverage are untouched. The edit is also appended to the shape edit list so future ephemeral chunk generation includes it.

---

## Terrain

Procedural terrain is generated chunk by chunk on demand. The solid collapse optimization makes this extremely efficient: below the surface is a large uniform solid region, above is air - both terminate high in the tree. Only the thin surface layer needs full leaf resolution.

Terrain is generated via noise-based heightmaps with erosion and layered geology. The shape edit API drives generation: for each chunk, test columns against the heightmap using the AABB walk, collapsing solid and empty regions immediately without ever expanding them to individual voxels.

Material transitions between geology layers use a dithered boundary: instead of a hard horizontal cut, each voxel samples `hash(world_pos) < blend_factor(height)` to decide which material it belongs to. This produces natural-looking stochastic transitions at zero memory cost, since the hash is computed on the fly during generation.

Terrain features like caves, boulders, and overhangs are driven by the same `Shape` API, making them entries in the shape edit list rather than special cases. Each shape is LOD-aware: at coarse levels the `coverage` implementation skips noise layers and detail passes whose contribution would be sub-voxel at that scale.

---

## LOD

LOD is implemented as a cascade of chunks where every LOD level uses the same depth-4 ESVC structure. Every chunk in memory is identical in format regardless of LOD level: a depth-4 64-tree with 256³ leaf slots. What changes between LOD levels is only the physical size of each leaf voxel. This mirrors the approach in Aokana (2505.02017), which uses uniform chunk resolution across all LOD levels, but with a 64-tree instead of an octree.

A LOD-0 chunk covers 256³ world voxels, each leaf = 1³ voxels. A LOD-1 chunk covers 1024³ world voxels using the same depth-4 structure, each leaf = 4³ original voxels. LOD-2 covers 4096³, each leaf = 16³ original voxels, and so on. Coverage grows as 256 × 4^k per side at LOD-k, reaching 2^64 at LOD-28.

The 4x scale factor per level (vs Aokana's 2x octree factor) means only 28 LOD levels are needed to span a 2^64 voxel world, instead of 56.

**Construction**: a LOD-k chunk is built by aggregating 64 LOD-(k-1) chunks, exactly as Aokana aggregates 8 octree chunks, but with 64 children instead of 8. Each output leaf samples 64 input leaves from the corresponding LOD-(k-1) chunks. A voxel is filled if the number of non-empty input voxels meets a density threshold, and the output color is the average of non-empty inputs. For persistent chunks there is a shortcut: level-2 nodes (which cover exactly 4³ original voxels) are lifted directly out of the existing tree without recomputation. For ephemeral chunks the shape edit list is re-run at LOD-k resolution, skipping sub-voxel detail. In both cases, any persistent chunks whose AABB intersects the target chunk are aggregated in: their trees are walked and their player-placed voxels influence the coarser LOD output.

When a persistent chunk is re-flushed, the LOD chunk covering that region is rebuilt from the updated ESVC.

**Coarsen and split**: LOD transitions are explicit world-tree operations, not chunk edits. `coarsen_chunk` takes 64 child handles, aggregates them into one new coarser chunk, frees the 64 old pool slots, and returns the new handle. `split_chunk` takes one coarser handle, spawns 64 finer chunks initialized from the parent, frees the parent slot, and returns the 64 new handles. Each new finer chunk is marked for shape resolution to fill in the sub-voxel detail the coarser chunk lacked.

**World tree LOD maintenance**: The CPU walks the world tree top-down each frame to enforce a simple invariant: the only non-leaf paths are those leading directly to an active point of interest. Every other occupied slot must be a leaf chunk. Any non-leaf child that no point of interest passes through is marked for consolidation: its subtree is collapsed via `coarsen_chunk` and replaced with a leaf entry.

A point of interest has a world position and a max depth. The camera is always a point of interest at max depth 28 (full LOD-0 resolution). A spyglass or scoped weapon adds a second point of interest at its target position, also at max depth 28, active only while in use. Any game object can register as a point of interest with whatever max depth gives sufficient detail at that range. When a point of interest is added or moves into a new cell, the leaf chunk on its path is split down to the required depth. When it is removed or moves away, the now-orphaned non-leaf path is consolidated.

For rendering, the GPU traverses the world tree and chunk trees continuously. Distant regions are LOD chunks with the same tree structure as any other chunk, just coarser leaves.

---

## GPU Memory

The GPU needs four things: scene metadata (camera position, sun direction, etc.), the world tree, a chunk offset table, and chunk data.

**Chunk data buffer**: A single large `storage<read>` buffer holding all chunk trees packed end-to-end. Managed as a Vec-style allocator: new chunks append to the end, removed chunks leave holes tracked in a CPU-side free list. The buffer grows with a doubling strategy only when live data exceeds its current size, so a small world uses small VRAM. When fragmentation exceeds a threshold, a GPU-side compaction pass runs: `copy_buffer_to_buffer` packs live chunks together, then a compute shader updates the offset table. Compaction runs at GPU memory bandwidth (200-400 GB/s on mid-range hardware), not PCIe bandwidth, so shifting 500 MB takes ~2 ms.

**Chunk offset table**: A `u32` array, one entry per pool handle. `chunk_offsets[handle]` is the byte offset of that chunk's data in the chunk data buffer. The world tree stores handles; the shader does one extra read to convert handle → offset before descending into the chunk tree.

**Uploads**: Edited chunks are re-uploaded in full (typically 200 KB - 1 MB each). The world tree is ~10-20 KB for a few thousand loaded chunks and is re-uploaded in full whenever the loaded set changes. Both are cheap relative to the per-frame render budget.

**Shader bindings**:
```wgsl
@group(0) @binding(0) var<storage, read> world_tree:    array<u32>;
@group(0) @binding(1) var<storage, read> chunk_offsets: array<u32>;
@group(0) @binding(2) var<storage, read> chunk_data:    array<u32>;
```
When a ray hits a leaf node in the world tree with handle `h`, it reads `chunk_offsets[h]` and jumps to `chunk_data[chunk_offsets[h]]` to continue traversal. One extra memory read per chunk boundary crossing.

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
