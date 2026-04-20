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

1. Each chunk is a sparse voxel 64-tree (not an octree), so nodes encode 64 children instead of 8, cutting per-voxel overhead.
2. No DAG, so edits are O(depth) with no CoW, rehashing, or refcounting
3. Each node stores a 64-bit occupancy mask and a 64-bit solid mask, which tracks uniform subtrees that terminate traversal early
4. Per-chunk material tables deduplicate on the full 32-bit voxel value (color + roughness + flags).
5. Child node indices and leaf material indices are stored in separate bitpacked arrays so leaf entries don't inflate pointer bit widths
6. All bitpacked widths scale with their contents: a chunk with 16 unique materials uses 4-bit indices everywhere
7. Each tree level is SoA, so GPU warps reading the same field across many nodes hit contiguous memory
8. Each node stores a blended subtree material computed bottom-up, enabling LOD without separate LOD trees
9. The `.lattice` file stores each chunk's levels top-down (coarsest first) so partial reads can stop early
10. Voxel edits are added to a queue where the affected chunks are rebuilt each frame in bulk.
11. Edits are streamed to the GPU in sections of edits, instead of uploading the entire chunk at once.

### Rendering

1. Rendered with ray tracing + beam optimization. (No rasterization)
2. Partial tree upload: CPU sends only the top N levels per chunk to VRAM based on camera distance, so VRAM cost tracks visible detail, not scene size
3. Traversal uses an ancestor stack caching parent node indices, so stepping into neighbor cells doesn't restart from the root
4. Coarse occupancy check groups the 64-bit occupancy into 8 regions of 2x2x2, enabling 8-cell skips over empty space
5. Coordinate flipping maps all rays into the negative octant, halving the branch count in the DDA inner loop
6. LOD cutoff: when a node has no uploaded children, the GPU reads the blended subtree material and renders one colored cube instead of descending

### Later (path tracing)

1. Bidirectional: rays from camera and rays from light sources (sun, emissive voxels)
2. Emissive voxels discovered by camera rays get added to the light list dynamically
3. Per-face unique ID with lighting averaged across that face, doing spatial and temporal accumulation in one pass
4. Averaging strength tied to roughness: mirror faces accumulate slowly, diffuse faces accumulate aggressively
5. Faces not updated recently get evicted from the lighting buffer

---

## Voxel

Every voxel is a 32-bit value. The color is full 24-bit linear RGB, not palette-indexed. 

```
Voxel (u32):

  bits 31-8   rgb          24-bit linear RGB color
  bits  7-4   roughness    nibble, 0 = mirror, 15 = fully diffuse
  bit   3     emissive     emits light at its albedo color
  bit   2     metallic     conductor, albedo tints specular
  bit   1     transparent  refracts rather than reflects
  bit   0     reserved
```

---

## Sparse Voxel 64-Tree

Each chunk is a sparse tree where every node covers a 4x4x4 block of children (64 slots). Tree depth is configurable and determines chunk resolution: depth 4 gives 4^4 = 256 voxels per side, depth 3 gives 64, etc.

### SoA layout

Each tree level is stored as a set of parallel arrays, one per field. A warp of 32 GPU threads reading occupancy for 32 different nodes hits one contiguous memory region. AoS would scatter those reads across cache lines.

A child is either an index into the next level (descend further) or a material table index meaning the entire subtree is one material (stop traversing). The solid mask tells you which is which. The popcount of occupancy gives the child count for a node.

At the deepest level, every child is a voxel. There are no child node indices, only leaf material indices. The solid mask equals occupancy because there's nothing further to descend into.

### Bitpacking

Child node indices and leaf material indices live in separate arrays to avoid polluting each other's bit width. If they shared an array, flag entries would force widening to 32 bits.

Bit widths are restricted to powers of 2: {1, 2, 4, 8, 16, 32}. This avoids cross-word reads where a single entry spans two u32s, which would require two loads on the GPU.

### Blended material

Every node stores a blended subtree material: the dominant material of its subtree, computed bottom-up at pack time. When the GPU hits a node whose children weren't uploaded (too far away for full detail), it reads this value and renders the node as a solid colored cube.

This also appears at the bottom level for consistency, but there it's just the average of the node's voxels. The real per-voxel data is in the leaf material indices.

---

## Material Table

Each chunk has its own material table mapping indices to full 32-bit voxel values.

The table is built during packing by collecting all unique voxel values in the chunk's subtree. The bit width of all material index fields scales with table size, so chunks with few unique materials compress much better.

---

## World

The world is a flat 3D grid of chunk entries. Dimensions are computed at import time from the scene's bounding box. Each entry is either a chunk index, a proxy (only blended material metadata loaded), or empty.

At depth 4, each chunk covers 256^3 voxels. At 10cm voxel size, that's a 25.6m cube per chunk. A 500m scene is roughly 20x20x20 chunks.

---

## LOD via Partial Upload

The full tree lives in RAM. When uploading to VRAM, the CPU decides how many levels to send per chunk based on camera distance.

A nearby chunk gets all 4 levels (full detail). A far chunk gets 2 (the GPU traverses down, hits a node with no uploaded children, reads the blended material, renders a colored cube). A chunk at the horizon might get just level 0 (the root node, one color for the whole 25.6m cube).

This is continuous, not discrete LOD steps. You can send exactly 1, 2, 3, or 4 levels per chunk. VRAM cost scales with what's actually visible at usable detail, not total scene size.

No separate LOD trees, no LOD construction pipeline, no extra disk storage. Just upload less of the same tree.

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