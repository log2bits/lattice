# Lattice

A voxel renderer with path tracing, built in Rust + WebGPU. Import a glTF scene, voxelize it into a grid of sparse 64-trees, and render it with GPU ray tracing in the browser.

The offline tool converts glTF to a `.lattice` file. The viewer loads that file, uploads partial tree data to the GPU based on camera distance, and traces rays in a compute shader. Voxels can be edited at runtime (place emissive blocks, see the lighting change live) but edits aren't saved to disk. It's a tech demo, not a game engine.

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
3. Each node stores a 64-bit `occupancy` mask and a 64-bit `solid_mask`, which tracks uniform subtrees that terminate traversal early
4. Per-chunk `MaterialTable` deduplicates on the full 32-bit Voxel value (color + roughness + flags), not just color
5. `node_children` and `leaf_materials` are stored in separate bitpacked arrays so leaf entries don't inflate pointer bit widths
6. All bitpacked widths scale with their contents: a chunk with 16 unique materials uses 4-bit indices everywhere
7. Every `NodePool` is SoA, so GPU warps reading the same field across many nodes hit contiguous memory
8. `lod_material` per node stores a blended subtree material computed bottom-up, enabling LOD without separate LOD trees
9. .lattice file stores each chunk's depths top-down (coarsest first) so partial reads can stop early

### Rendering

1. Partial tree upload: CPU sends only the top N `NodePool` depths per chunk to VRAM based on camera distance, so VRAM cost tracks visible detail, not scene size
2. Upload depth is continuous (1, 2, 3, or 4 depths per chunk), not discrete LOD steps
3. Traversal uses an ancestor stack caching parent node indices, so stepping into neighbor cells doesn't restart from the root
4. Coarse occupancy check groups the 64-bit `occupancy` into 8 regions of 2x2x2, enabling 8-cell skips over empty space
5. Coordinate flipping (`flip_mask`) maps all rays into the negative octant, halving the branch count in the DDA inner loop
6. LOD cutoff: when a node has no uploaded children, the GPU reads `lod_material` and renders one colored cube instead of descending

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
  -> morton-sorted VoxelSample stream per chunk
  -> bottom-up 64-tree construction
  -> .lattice file on disk

.lattice file
  -> full Lattice in RAM
  -> partial tree upload to VRAM (LOD)
  -> GPU ray traversal
```

Import and packing happen offline in a native CLI tool. Loading, uploading, rendering, and editing happen at runtime in the viewer (native or WASM).

---

## Voxel

Every voxel is a 32-bit value. The color is full 24-bit linear RGB, not palette-indexed. The palette only exists during glTF import to keep per-chunk material tables small. Other importers (procedural, .vox, etc.) can skip the palette and use arbitrary colors.

```
Voxel (u32, repr(transparent)):

  bits 31-8   rgb          24-bit linear RGB color
  bits  7-4   roughness    nibble, 0 = mirror, 15 = fully diffuse
  bit   3     emissive     emits light at its albedo color
  bit   2     metallic     conductor, albedo tints specular
  bit   1     transparent  refracts rather than reflects
  bit   0     reserved
```

Zero-cost conversion to/from `u32`.

---

## Sparse Voxel 64-Tree

Each chunk is a sparse tree where every node covers a 4x4x4 block of children (64 slots). Tree depth is configurable and determines chunk resolution: depth 4 gives 4^4 = 256 voxels per side, depth 3 gives 64, etc.

No DAG. Every node is unique and owns its children. This keeps edits trivial: walk down, change a leaf, update `lod_material` on the way back up. No copy-on-write, no rehashing, no reference counting.

### SoA layout

Each tree depth is stored as a `NodePool`, which is a set of parallel arrays, one per field. A warp of 32 GPU threads reading `occupancy` for 32 different nodes hits one contiguous memory region. AoS would scatter those reads across cache lines.

Per `NodePool`:

| Array | Type | Description |
|---|---|---|
| `occupancy` | `Vec<u64>` | Which of 64 slots have something in them |
| `solid_mask` | `Vec<u64>` | Which occupied children are uniform (whole subtree is one material, stop here) |
| `children_offset` | `Vec<u32>` | Where this node's children begin in the two child arrays |
| `lod_material` | `BitpackedArray` | Blended material index per node, used when LOD cuts traversal short |
| `node_children` | `BitpackedArray` | Indices into the next depth's NodePool |
| `leaf_materials` | `BitpackedArray` | MaterialTable indices for solid subtree children |

A child is either an index into the next `NodePool` (descend further) or a `MaterialTable` index meaning the entire subtree is one material (stop traversing). `solid_mask` tells you which is which. `occupancy.count_ones()` gives the child count for a node.

At the deepest level, every child is a voxel. There are no `node_children`, only `leaf_materials`. The `solid_mask` equals `occupancy` because there's nothing further to descend into.

### Bitpacking

`node_children` and `leaf_materials` live in separate arrays to avoid polluting each other's bit width. If they shared an array, any `SOLID_FLAG` entry would force widening to 32 bits.

`node_children` is bitpacked at `ceil(log2(pool_size))` bits, set once per depth after construction. `leaf_materials` is bitpacked at `ceil(log2(table_size))` bits, set per chunk. A chunk with 16 unique voxels stores everything at 4 bits per entry. One with 200 uses 8 bits.

### LOD material

Every node stores a `lod_material`: the dominant material of its subtree, blended bottom-up at pack time. When the GPU hits a node whose children weren't uploaded (too far away for full detail), it reads `lod_material` and renders the node as a solid colored cube.

This also appears at the bottom level for consistency, but there it's just the same as whatever the node's voxels average to. The real per-voxel data is in `leaf_materials`.

---

## MaterialTable

Each chunk has its own `MaterialTable` mapping indices to full 32-bit Voxel values. Two voxels with the same RGB but different roughness are distinct entries.

The table is built during packing by collecting all unique Voxel values in the chunk's subtree. The bit width of all material index fields (`leaf_materials`, `lod_material`) scales with table size, so chunks with few unique materials compress better.

---

## Grid

The world is a flat 3D grid of chunk entries. Dimensions are computed at import time from the scene's bounding box. Each entry is either a chunk index, a proxy (only `lod_material` metadata loaded, flagged with `PROXY_FLAG`), or empty.

At depth 4, each chunk covers 256^3 voxels. At 10cm voxel size, that's a 25.6m cube per chunk. A 500m scene is roughly 20x20x20 chunks.

---

## LOD via Partial Upload

The full Lattice lives in RAM. When uploading to VRAM, the CPU decides how many `NodePool` depths to send per chunk based on camera distance.

A nearby chunk gets all 4 depths (full detail). A far chunk gets 2 (the GPU traverses down, hits a node with no uploaded children, reads `lod_material`, renders a colored cube). A chunk at the horizon might get just depth 0 (the root node, one color for the whole 25.6m cube).

This is continuous, not discrete LOD steps. You can send exactly 1, 2, 3, or 4 depths per chunk. VRAM cost scales with what's actually visible at usable detail, not total scene size.

No separate LOD trees, no LOD construction pipeline, no extra disk storage. Just upload less of the same tree.

### Upload strategy

Each frame the CPU walks the grid, computes a target upload depth per chunk based on distance and screen-space projected size, and diffs against what's currently in the GPU buffer. Changed chunks get re-uploaded. The GPU buffers are structured so each `NodePool` depth is contiguous and can be updated independently.

---

## Import

The importer turns a glTF scene into a stream of `VoxelSample`s (position + voxel value) per chunk.

### Color palette

A 256-entry color palette is spread across OKLab space via sample elimination. A 16MB lookup table maps every possible sRGB triplet to the nearest palette entry. This only exists during glTF import to keep chunk `MaterialTable`s small. The palette is not part of the Lattice or the file format.

### Triangle partitioning

One pass over all meshes builds a flat triangle list and a partition map from chunk grid coordinates to triangle indices. Each triangle goes into every chunk whose AABB it overlaps.

### Voxelization

Per chunk, in parallel across threads: clip triangles to the chunk AABB, rasterize them using barycentric projection with fat voxelization (guarantees 6-connected surfaces for interior culling), sample texture colors, snap to palette, emit morton-sorted VoxelSample runs.

Fat voxelization is important. Without it, thin shells have gaps, and the "all 6 neighbors occupied" interior culling check fails, wasting memory on invisible voxels.

The voxelization approach is adapted from voxquant. Project each triangle onto its dominant axis plane, iterate the 2D bounding box, solve for the depth coordinate via the plane equation, emit voxels. Conservative rasterization via wireframe ensures no gaps from aliasing on thin triangles.

### Interior culling

After voxelization, any voxel with all 6 face-neighbors occupied and opaque is culled. These voxels are never visible from any direction.

---

## Packing

The packer takes per-chunk VoxelSample streams and builds the Lattice.

### Morton sort

Each chunk's samples arrive sorted in morton order from the voxelizer. Chunks are independent so each stream is processed separately.

### Bottom-up construction

The tree is built bottom-up from sorted samples. Leaf nodes are created first, then parent nodes from groups of 64 children. Uniform subtrees (all children are the same material) collapse into a single `leaf_materials` entry in the parent, and the parent's `solid_mask` bit is set. `lod_material` is computed during this pass by blending children bottom-up.

### Finalization

After the tree is built, unique Voxel values are collected into the chunk's `MaterialTable`. All material index fields are bitpacked to their final width based on table size. `node_children` arrays are bitpacked based on pool size at each depth.

### Serialization

The finished Lattice is written to a `.lattice` file. Chunks are stored contiguously and independently so the format is trivially streamable later if needed. Each chunk's data is written top-down (coarsest depth first) so partial reads for LOD can stop early.

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
  table_values      [Voxel; table_size]
  per depth (top-down, coarsest first):
    node_count      u32
    occupancy       [u64; node_count]
    solid_mask      [u64; node_count]
    children_offset [u32; node_count]
    lod_material    bitpacked (table bit width)
    node_children   bitpacked (pool bit width)
    leaf_materials  bitpacked (table bit width)

Grid:
  entries           [u32; grid_dims.x * grid_dims.y * grid_dims.z]
```

Top-down per chunk means you can read just the first 1-2 depths for a far chunk and stop. No seeking.

---

## Rendering

GPU ray tracing in WebGPU compute shaders. One thread per pixel, each thread casts a ray through the scene.

### V1: Primary rays only

Cast one ray per pixel from the camera. Traverse the 64-tree using DDA with the ancestor stack trick from dubiousconst282's guide. When a leaf is hit, read the material and shade with direct sun lighting. When a node has no uploaded children (LOD cutoff), use `lod_material` and render as a solid cube.

The ancestor stack caches parent node indices so stepping into neighbor cells doesn't require restarting from the root. The coarse occupancy check groups the 64-bit `occupancy` into 8 coarse 2x2x2 regions, letting the traversal skip over empty regions in 2^3 steps at once.

### Later: Path tracing

Bidirectional. Rays from the camera bounce off surfaces, rays from light sources (sun, emissive voxels) cast into the scene. Emissive voxels are discovered by camera rays and added to a light list.

Every voxel face gets a unique ID. Lighting values are accumulated per-face across frames, doing spatial and temporal averaging. Mirror surfaces average less, diffuse surfaces average more. Faces that haven't been updated recently get evicted.

### Debug overlays

Normals, depth, LOD depth per chunk, traversal iteration heatmap, voxel grid lines. Toggled at runtime.

---

## Editing

At runtime, the user can place and remove individual voxels. The main use case is dropping emissive voxels into the scene to watch the lighting change.

An edit walks down the tree to the target leaf, modifies it (or inserts/removes a child), then walks back up updating `lod_material` along the path. If the edit adds a new unique voxel value, the chunk's `MaterialTable` is extended and affected bitpacked arrays are recomputed. The modified chunk data is re-uploaded to the GPU.

O(depth) work per edit. No CoW, no DAG overhead.

---

## Module Structure

```
lattice/
  Cargo.toml

  src/
    lib.rs                    crate root, feature flags, re-exports

    voxel.rs                  Voxel newtype, bit layout, From<u32>, field accessors
    bitpacked.rs              BitpackedArray: fixed-width packed storage, encode/decode
    material_table.rs         MaterialTable: per-chunk unique Voxel values, index lookup

    tree/
      mod.rs                  Lattice: grid + depth + node pools + chunks
      pool.rs                 NodePool: SoA arrays (occupancy, solid_mask, children, etc.)
      chunk.rs                Chunk: root node index + MaterialTable
      grid.rs                 Grid: flat 3D array of chunk entries, AABB sizing
      node.rs                 SOLID_FLAG, PROXY_FLAG, slot indexing, coarse occupancy
      walk.rs                 walk down to a position, walk up updating lod_material

    import/                   cfg(feature = "import"), excluded from WASM builds
      mod.rs                  ImportConfig, VoxelSample, import entry point
      gltf.rs                 glTF loading, mesh extraction, scene bounds
      partition.rs            triangle binning into chunk grid cells
      voxelize.rs             triangle rasterization (barycentric projection, fat mode)
      palette.rs              OKLab 256-color palette, sRGB -> index LUT
      pbr.rs                  PBR material properties -> Voxel bit layout
      cull.rs                 interior voxel removal (6-neighbor check)

    pack/                     cfg(feature = "import")
      mod.rs                  PackConfig, entry point
      sort.rs                 morton ordering, per-chunk stream merge
      build.rs                bottom-up tree construction, lod_material blending
      finalize.rs             MaterialTable collection, bitpack width assignment

    format/
      mod.rs                  .lattice header struct, magic bytes, version
      write.rs                Lattice -> .lattice file
      read.rs                 .lattice file -> Lattice in RAM

    render/
      mod.rs                  Renderer: device/queue setup, frame loop
      camera.rs               camera state, projection, mouse/key input
      lod.rs                  per-chunk target depth selection based on distance
      upload.rs               GPU buffer layout, partial NodePool upload, diff/update
      pipeline.rs             compute pipeline creation, bind group layout
      present.rs              output blit, tonemapping, swapchain management
      debug.rs                debug overlay modes and uniforms

    edit.rs                   place/remove voxels, tree walk, MaterialTable extension

  shaders/
    types.wgsl                Voxel struct, node layout, bitpacked decode, grid lookup
    traverse.wgsl             64-tree DDA, ancestor stack, coarse skip, LOD cutoff
    primary.wgsl              primary ray compute shader, one thread per pixel
    output.wgsl               fullscreen blit, basic tonemap
    debug.wgsl                overlay shaders: normals, depth, LOD, heatmap

  src/bin/
    pack.rs                   CLI: gltf -> .lattice
    view.rs                   CLI: .lattice -> window (native) or canvas (wasm)
    inspect.rs                CLI: print .lattice header and stats
```

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