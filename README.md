# Lattice

A voxel renderer with path tracing, built in Rust + WebGPU. Import a glTF scene, voxelize it into a grid of sparse 64-trees, and render it with GPU ray tracing in the browser.

The offline tool converts glTF to `.lattice` or `.vox`. The viewer loads a `.lattice` file, uploads partial tree data to the GPU based on camera distance, and traces rays in a compute shader. Voxels can be edited at runtime (place emissive blocks, see the lighting change live) but edits aren't saved to disk. It's a tech demo, not a game engine.

Compiles to native and WASM. Uses rayon for CPU parallelism, WebGPU/WGSL for rendering, and targets wasm_bindgen_rayon for threaded WASM builds.

---

# Optimizations

### Importing from glTF

1. 256-color palette spread across OKLab space via sample elimination, used only at import time
2. 16MB precomputed LUT maps every sRGB triplet to the nearest palette entry in O(1)
3. Scene is spatially partitioned into chunks before voxelization, so only overlapping triangles are sent to each chunk's voxelizer
4. Chunks are independent and parallelize trivially across threads with rayon
5. Fat voxelization guarantees 6-connected (watertight) surfaces
6. Conservative wireframe rasterization catches thin triangles that the area rasterizer would miss due to aliasing
7. Interior voxels (all 6 face-neighbors occupied and opaque) are culled after voxelization since they're never visible

### Storage

1. Each chunk is a sparse voxel 64-tree (not an octree), so nodes encode 64 children instead of 8, cutting per-voxel overhead to ~0.19 bytes vs ~0.57 for SVOs
2. No DAG, so edits are O(depth) with no CoW, rehashing, or refcounting
3. Each node stores a 64-bit occupancy mask and a 64-bit solid mask, which tracks uniform subtrees that terminate traversal early
4. Per-chunk material tables deduplicate on the full 32-bit voxel value (color + roughness + flags), not just color
5. Child node indices and leaf material indices are stored in separate bitpacked arrays so leaf entries don't inflate pointer bit widths
6. All bitpacked widths scale with their contents: a chunk with 16 unique materials uses 4-bit indices everywhere
7. Each tree level is SoA, so GPU warps reading the same field across many nodes hit contiguous memory
8. Each node stores a blended subtree material computed bottom-up, enabling LOD without separate LOD trees
9. The `.lattice` file stores each chunk's levels top-down (coarsest first) so partial reads can stop early

### Rendering

1. Partial tree upload: CPU sends only the top N levels per chunk to VRAM based on camera distance, so VRAM cost tracks visible detail, not scene size
2. Upload depth is continuous (1, 2, 3, or 4 levels per chunk), not discrete LOD steps
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

## Pipeline

```
gltf scene
  -> triangle partitioning per chunk
  -> per-chunk voxelization (parallel)
  -> morton-sorted voxel sample stream per chunk
  -> bottom-up 64-tree construction
  -> .lattice or .vox file on disk

.lattice file
  -> full tree in RAM
  -> partial tree upload to VRAM (LOD)
  -> GPU ray traversal
```

Conversion happens offline in a native CLI tool. Loading, uploading, rendering, and editing happen at runtime in the viewer (native or WASM).

---

## Voxel

Every voxel is a 32-bit value. The color is full 24-bit linear RGB, not palette-indexed. The palette only exists during glTF import to keep per-chunk material tables small. Other importers (procedural, `.vox`, etc.) can skip the palette and use arbitrary colors.

```
Voxel (u32):

  bits 31-8   rgb          24-bit linear RGB color
  bits  7-4   roughness    nibble, 0 = mirror, 15 = fully diffuse
  bit   3     emissive     emits light at its albedo color
  bit   2     metallic     conductor, albedo tints specular
  bit   1     transparent  refracts rather than reflects
  bit   0     reserved
```

Zero-cost conversion to/from u32.

---

## Sparse Voxel 64-Tree

Each chunk is a sparse tree where every node covers a 4x4x4 block of children (64 slots). Tree depth is configurable and determines chunk resolution: depth 4 gives 4^4 = 256 voxels per side, depth 3 gives 64, etc.

No DAG. Every node is unique and owns its children. This keeps edits trivial: walk down, change a leaf, update the blended material on the way back up. No copy-on-write, no rehashing, no reference counting.

### SoA layout

Each tree level is stored as a set of parallel arrays, one per field. A warp of 32 GPU threads reading occupancy for 32 different nodes hits one contiguous memory region. AoS would scatter those reads across cache lines.

Per level:

| Field | Description |
|---|---|
| occupancy (64-bit per node) | Which of 64 slots have something in them |
| solid mask (64-bit per node) | Which occupied children are uniform (whole subtree is one material, stop here) |
| children offset (32-bit per node) | Where this node's children begin in the two child arrays |
| blended material (bitpacked) | Blended material index per node, used when LOD cuts traversal short |
| child node indices (bitpacked) | Indices into the next level |
| leaf material indices (bitpacked) | Material table indices for solid subtree children |

A child is either an index into the next level (descend further) or a material table index meaning the entire subtree is one material (stop traversing). The solid mask tells you which is which. The popcount of occupancy gives the child count for a node.

At the deepest level, every child is a voxel. There are no child node indices, only leaf material indices. The solid mask equals occupancy because there's nothing further to descend into.

### Bitpacking

Child node indices and leaf material indices live in separate arrays to avoid polluting each other's bit width. If they shared an array, flag entries would force widening to 32 bits.

Bit widths are restricted to powers of 2: {1, 2, 4, 8, 16, 32}. This avoids cross-word reads where a single entry spans two u32s, which would require two loads on the GPU.

Child node indices are bitpacked at the next power of 2 above ceil(log2(level size)) bits, set once per level after construction. Leaf material indices are bitpacked at the next power of 2 above ceil(log2(table size)) bits, set per chunk. A chunk with 16 unique voxels stores everything at 4 bits per entry. One with 200 uses 8 bits.

### Blended material

Every node stores a blended subtree material: the dominant material of its subtree, computed bottom-up at pack time. When the GPU hits a node whose children weren't uploaded (too far away for full detail), it reads this value and renders the node as a solid colored cube.

This also appears at the bottom level for consistency, but there it's just the average of the node's voxels. The real per-voxel data is in the leaf material indices.

---

## Material Table

Each chunk has its own material table mapping indices to full 32-bit voxel values. Two voxels with the same RGB but different roughness are distinct entries.

The table is built during packing by collecting all unique voxel values in the chunk's subtree. The bit width of all material index fields scales with table size, so chunks with few unique materials compress better.

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

### Upload strategy

Each frame the CPU walks the grid, computes a target upload depth per chunk based on distance and screen-space projected size, and diffs against what's currently in the GPU buffer. Changed chunks get re-uploaded. The GPU buffers are structured so each level is contiguous and can be updated independently.

---

## Conversion

The converter turns a glTF scene into either a `.lattice` file or a `.vox` file.

### Color palette

A 256-entry color palette is spread across OKLab space via sample elimination. A 16MB lookup table maps every possible sRGB triplet to the nearest palette entry. This only exists during glTF import to keep chunk material tables small. The palette is not part of the tree or the file format.

### Triangle partitioning

One pass over all meshes builds a flat triangle list and a partition map from chunk grid coordinates to triangle indices. Each triangle goes into every chunk whose AABB it overlaps.

### Voxelization

Per chunk, in parallel across threads: clip triangles to the chunk AABB, rasterize them using barycentric projection with fat voxelization (guarantees 6-connected surfaces for interior culling), sample texture colors, snap to palette, emit morton-sorted voxel sample runs.

Fat voxelization is important. Without it, thin shells have gaps, and the "all 6 neighbors occupied" interior culling check fails, wasting memory on invisible voxels.

The voxelization approach is adapted from voxquant. Project each triangle onto its dominant axis plane, iterate the 2D bounding box, solve for the depth coordinate via the plane equation, emit voxels. Conservative rasterization via wireframe ensures no gaps from aliasing on thin triangles.

### Interior culling

After voxelization, any voxel with all 6 face-neighbors occupied and opaque is culled. These voxels are never visible from any direction.

---

## Packing

The packer takes per-chunk voxel sample streams and builds the tree.

### Morton sort

Each chunk's samples arrive sorted in morton order from the voxelizer. Chunks are independent so each stream is processed separately.

### Bottom-up construction

The tree is built bottom-up from sorted samples. Leaf nodes are created first, then parent nodes from groups of 64 children. Uniform subtrees (all children are the same material) collapse into a single leaf material entry in the parent, and the parent's solid mask bit is set. The blended material is computed during this pass by blending children bottom-up.

### Finalization

After the tree is built, unique voxel values are collected into the chunk's material table. All material index fields are bitpacked to their final width based on table size. Child node index arrays are bitpacked based on level size at each depth.

### Serialization

The finished tree is written to a `.lattice` file. Chunks are stored contiguously and independently so the format is trivially streamable later if needed. Each chunk's data is written top-down (coarsest level first) so partial reads for LOD can stop early.

---

## .lattice File Format

```
Header:
  magic             [u8; 8]     "LATTICE\0"
  version           u32
  depth             u8
  grid_dims         [u32; 3]
  chunk_count       u32
  voxel_size_m      f32         meters per voxel (e.g. 0.1)

Per chunk (chunk_count times):
  table_size        u32
  table_values      [u32; table_size]
  per level (top-down, coarsest first):
    node_count      u32
    occupancy       [u64; node_count]
    solid_mask      [u64; node_count]
    children_offset [u32; node_count]
    lod_materials   bitpacked (table bit width)
    child_indices   bitpacked (level bit width)
    leaf_indices    bitpacked (table bit width)

World:
  entries           [u32; grid_dims.x * grid_dims.y * grid_dims.z]
```

Top-down per chunk means you can read just the first 1-2 levels for a far chunk and stop. No seeking.

---

## Rendering

GPU ray tracing in WebGPU compute shaders. One thread per pixel, each thread casts a ray through the scene.

### V1: Primary rays only

Cast one ray per pixel from the camera. Traverse the 64-tree using DDA with the ancestor stack trick. When a leaf is hit, read the material and shade with direct sun lighting. When a node has no uploaded children (LOD cutoff), use the blended material and render as a solid cube.

The ancestor stack caches parent node indices so stepping into neighbor cells doesn't require restarting from the root. The coarse occupancy check groups the 64-bit occupancy into 8 coarse 2x2x2 regions, letting the traversal skip over empty regions in 2^3 steps at once.

### Later: Path tracing

Bidirectional. Rays from the camera bounce off surfaces, rays from light sources (sun, emissive voxels) cast into the scene. Emissive voxels are discovered by camera rays and added to a light list.

Every voxel face gets a unique ID. Lighting values are accumulated per-face across frames, doing spatial and temporal averaging. Mirror surfaces average less, diffuse surfaces average more. Faces that haven't been updated recently get evicted.

### Debug overlays

Normals, depth, LOD depth per chunk, traversal iteration heatmap, voxel grid lines. Toggled at runtime.

---

## Editing

At runtime, the user can place and remove individual voxels. The main use case is dropping emissive voxels into the scene to watch the lighting change.

An edit walks down the tree to the target leaf, modifies it (or inserts/removes a child), then walks back up updating the blended material along the path. If the edit adds a new unique voxel value, the chunk's material table is extended and affected bitpacked arrays are recomputed. The modified chunk data is re-uploaded to the GPU.

O(depth) work per edit. No CoW, no DAG overhead.

---

## Resources

### Key References

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