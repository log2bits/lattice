# Lattice

A voxel renderer with full path tracing. Lattice voxelizes a scene, compresses it into a sparse voxel 64-tree per chunk, and renders via GPU path tracing. The depth of the tree is the only configurable parameter. Everything else about the structure is fixed and uniform, which keeps GPU traversal simple and the file format clean.

---

## Pipeline

```
[import]   scene -> triangle bins -> per-chunk sorted runs -> k-way merged VoxelSample stream
[pack]     sorted VoxelSample stream -> Lattice (grid + levels + chunks) -> .lattice file
[load]     .lattice file -> RAM (full chunks) -> VRAM (partial depth for LOD)
[render]   GPU buffers -> path traced image
```

Each stage is independent. The handoff between stages is a well-defined data structure, so you can test any stage in isolation or swap in a different importer.

---

## Design choices that are fixed

- One tree type: sparse voxel 64-tree (SVO). No DAG, no deduplication across chunks.
- Materials are stored inline with the tree. Each node carries a representative material LUT index for LOD. Uniform subtrees store a single material and terminate early.
- Every tree level uses SoA layout. No AoS.
- The grid is a flat 3D array of chunk pointers. Its dimensions are computed at build time from the scene bounds.
- The only configurable parameter is `depth`. Everything else is determined by it.
- All domain counts and indices are `u32`. `usize` is only used at `Vec` indexing call sites.
- Face normals are derived from the DDA exit face at traversal time. No per-voxel normals stored.
- No editing during rendering. Data is built once and uploaded. Read-only at runtime.
- The renderer is always full path tracing. No rasterization fallback.

---

## Structure

```
Lattice
  grid: Grid                   flat 3D array of chunk pointers
  depth: u8                    number of SVO levels
  levels: Vec<Level>           shared node pools, one per depth level
  chunks: Vec<Chunk>           per-chunk material palette and root node index
```

The grid and levels are shared across all chunks. Each chunk owns its own material palette (LUT). Two grid cells pointing to the same chunk index share all tree and material data.

---

## Sparse Voxel 64-tree

Each tree node covers a 4x4x4 block of children (64 slots). The `child_mask` field (u64) tracks which slots are occupied. Only occupied children are stored.

Each level is SoA:

- `child_mask: Vec<u64>` -- one bit per child slot. A 1 bit means a child exists there.
- `leaf_mask: Vec<u64>` -- which occupied children are uniform (leaf) entries vs node pointers.
- `child_start: Vec<u32>` -- where this node's children begin in the two children arrays.
- `rep_material: BitpackedArray` -- per-chunk LUT index per node, blended bottom-up from children. Bitpacked at `ceil(log2(lut_size))` bits. Used for LOD early termination.
- `ptr_children: BitpackedArray` -- node pointer children, bitpacked at `ceil(log2(node_pool_size))` bits.
- `lut_children: BitpackedArray` -- uniform subtree LUT indices, bitpacked at `ceil(log2(lut_size))` bits.

A child entry is either:
- `LEAF_FLAG | lut_index`: uniform subtree, all voxels have material `lut_index`. Traversal terminates here.
- A node pointer into the next level's pool.

Only occupied children are stored. `child_mask[i].count_ones()` gives the child count for node `i`.

SoA layout matters for GPU traversal. A warp of 32 threads reading `child_mask` for 32 different nodes touches one contiguous memory region. AoS would scatter those reads.

---

## Material data

Materials are stored inline with the tree. Each node has a `rep_material` field: a per-chunk LUT index representing the blended material of its subtree. This is computed bottom-up at pack time and stored on disk as part of each Level chunk.

Uniform subtrees (LEAF_FLAG set) contribute no children and terminate traversal immediately. A 64^3 region of solid stone costs one child entry in the parent node.

### Per-chunk LUT

Each chunk owns a `Lut`:

```
Lut
  values: Vec<Voxel>   unique voxels referenced by this chunk's subtree
```

The bit width of child LUT indices is determined by the LUT size. A chunk with 16 unique voxels uses 4-bit index fields. One with 200 uses 8-bit. Compression is local to each chunk.

### Bitpacking strategy

Mixing node pointers and `LEAF_FLAG | lut_index` entries in the same array breaks compression. `LEAF_FLAG` is bit 31, so any entry with it set looks like a large number and forces the array to widen to 32 bits, regardless of LUT size.

The fix is to never mix them. Each level's children split into two separate bitpacked arrays, selected by a `leaf_mask`:

- `leaf_mask: u64` -- which occupied children are uniform (LEAF_FLAG) entries. Sits alongside `child_mask` in the SoA.
- pointer children: bitpacked at `ceil(log2(node_pool_size))` bits. Width set once per level after construction.
- leaf children: bitpacked at `ceil(log2(lut_size))` bits. Width set once per chunk.

The deepest level has no node pointers at all, only leaf children, so it uses only the leaf array. `rep_material` is also just a LUT index and gets the same treatment: stored as a bitpacked array at `ceil(log2(lut_size))` bits per node rather than a flat `Vec<u32>`.

The result is that palette compression applies everywhere LUT indices appear: leaf children at all levels, rep_material at all levels. A chunk with 16 unique voxels stores all of that data at 4 bits per entry.

---

## Voxel format

Every voxel in every scene uses the same 32-bit layout:

```
Voxel (32 bits):

  bits 31-8   rgb          24-bit linear RGB color
  bits  7-4   roughness    nibble, 0 = mirror, 15 = fully diffuse
  bit   3     emissive     emits light at its albedo color
  bit   2     metallic     conductor, albedo tints specular
  bit   1     transparent  refracts rather than reflects
  bit   0     reserved
```

`Voxel` is a `#[repr(transparent)]` newtype over `u32` with `From<u32>` and `From<Voxel>` impls for zero-cost conversion.

---

## Import-time color palette

During glTF import, surface samples need to be mapped to discrete voxel colors. The importer uses a precomputed 256-entry color palette spread uniformly across OKLab space via sample elimination. Each sampled surface point is snapped to the nearest palette entry before it ever enters the Lattice.

This palette only exists during import. It's not part of the Lattice structure. By the time voxels reach the packer, their RGB values are already set. The per-chunk LUT then further deduplicates on full 32-bit Voxel values (color + roughness + flags), so two voxels with the same palette color but different roughness are still distinct LUT entries.

With 256 palette colors and 16 roughness levels plus material flags, a scene ends up with at most a few thousand unique voxels per chunk. This keeps LUT sizes small and index bit widths narrow.

---

## Import chunking

At depth=8, each chunk covers 4^8 = 65,536 voxels per side. Materializing all voxels for even a moderately dense scene before packing would exhaust RAM. Instead, the importer works in two phases.

**Phase 1 -- triangle binning.** One pass over all meshes builds a flat `Vec<Triangle>` and a bin map from chunk grid coordinates to triangle indices. Each triangle goes into every chunk whose AABB it overlaps. The flat triangle list stays in RAM throughout import; it's the smallest representation of the scene geometry.

**Phase 2 -- per-chunk voxelization.** For each occupied chunk cell, in Morton order: clip triangles to the chunk AABB, voxelize, sort the resulting `VoxelSample`s in Morton order, and emit the sorted run. The run is handed directly to the packer via callback, then dropped. Only one chunk's samples are in memory at a time.

The chunk size (`ImportConfig::chunk_size`, voxels per side, must be a power of 4) is independent of `depth`. It's purely a memory budget knob and has no effect on the output. Chunk boundaries are power-of-4 aligned in voxel space so Morton codes are contiguous within each run.

The packer's k-way merge in `sort.rs` stitches the per-chunk sorted runs back into a globally ordered stream.

---

## Grid

The grid is a flat 3D array of child entries. Its dimensions are determined at build time. When importing a glTF scene, the importer computes the axis-aligned bounding box of the entire scene in voxel space, then sizes the grid to cover that volume exactly.

Each grid entry is either:
- A chunk index (clear LEAF_FLAG): real sub-tree, data resident in VRAM
- `LEAF_FLAG | chunk_index` (LEAF_FLAG set): proxy, only rep_material metadata in VRAM

---

## Streaming and LOD

Disk->RAM and RAM->VRAM are separate concerns with different granularities.

**Disk->RAM:** full chunks always. The streaming manager reads entire chunks from disk into RAM ahead of time. Disk reads are slow and infrequent. They happen far enough in advance that they're never on the critical path.

**RAM->VRAM:** partial-depth uploads based on camera distance. RAM always holds the full tree for every loaded chunk. VRAM gets however many levels are needed given the camera's distance. LOD kicks in when a voxel at the current depth would subtend less than one pixel.

When uploading a chunk at depth N < depth, nodes at level N get `LEAF_FLAG | rep_material_lut_index` child entries instead of real pointers. The GPU traversal hits LEAF_FLAG, reads the rep material from the root's LUT, and terminates. The transition from depth N to depth N+1 is a VRAM upload of one more level plus a rewrite of the leaf entries above it.

The full lifecycle of a grid cell:

```
not loaded:   LEAF_FLAG | chunk_index in grid, rep_material in metadata only
loading:      async disk read into RAM
resident:     real chunk_index in grid, full or partial tree in VRAM
evicting:     write LEAF_FLAG | chunk_index back to grid, free VRAM regions
```

When a chunk transitions from proxy to resident, the grid entry is written last, after all VRAM data is in place. The GPU never sees a partially-uploaded chunk.

---

## Traversal

### Finding the hit

The ray traverses the tree level by level. At each node, the child_mask tells the shader which children exist. Empty children are skipped. When a child entry has LEAF_FLAG set, the shader returns immediately using rep_material (looked up in the chunk's LUT) for both LOD nodes and uniform leaf subtrees.

### Traversal optimizations

**Fractional coordinate encoding.** The tree lives in [1.0, 2.0). IEEE floats in that range have exponent zero, so the mantissa encodes position as fixed-point. Extracting the cell index at any level is two bit operations with no multiply or divide.

**Ancestor stack.** Caches parent node indices. When the ray steps to a neighbor, comparing old and new position bits identifies the highest level that changed. The stack has the right values ready. About 2x speedup over root-to-leaf descent.

**2x2x2 sub-block coalescing.** Before stepping, the traversal checks whether the 2x2x2 sub-block containing the current cell is entirely empty in the 64-bit child_mask. If so, the step size doubles. Adds about 21% more empty space skipped for free.

**Ray-octant mirroring.** The coordinate system is mirrored to the negative ray octant at traversal start. With all direction components negative, finding the exit face is just finding the minimum of three distances with no sign conditionals. About 10% faster.

---

## Lighting

Lighting is full path tracing. Primary rays dispatch from the camera. On each hit, the shader reads the voxel payload, derives the face normal from the DDA exit face, samples the BRDF based on roughness and the metallic flag, and dispatches a secondary ray. Emissive voxels contribute light directly. Shadow rays come out of the path tracing loop naturally.

Each voxel face stores a running weighted average of accumulated indirect light:

```
L_new = (1 - alpha) * L_old + alpha * S
```

`S` is the new path traced sample. `alpha` is tuned per surface type: small for diffuse (stable accumulation), larger for specular (faster response). No sample history buffers, no reservoirs. One color value per face.

---

## Disk format

On disk, sub-node references are removed from the children arrays. Nodes are written depth-first. The first time a unique node appears it gets a label. Every subsequent reference to the same node is a back-reference to that label. LEAF_FLAG entries are written as-is since they're terminal values.

The loader reconstructs explicit sub-node indices by walking the DFS stream. Loading is close to a direct copy.

This encoding achieves 2.8-3.8x smaller files than pointer-based storage.

---

## The .lattice file format

```
Header:
  magic:           [u8; 4]     "LTCE"
  version:         u16
  flags:           u16
  world_min:       [i64; 3]    voxel-space coordinates
  world_max:       [i64; 3]
  depth:           u8
  _pad:            [u8; 3]
  chunk_count:     u32
  chunk_entry_count: u32
  chunk_entries:   [{tag: u32, offset: u64, size: u64}; chunk_entry_count]
```

Chunks:

```
LVL*    one chunk per SVO level (* = level index, 0 = root level)
  node_count:         u32
  child_mask:         [u64; node_count]
  leaf_mask:          [u64; node_count]
  child_start:        [u32; node_count]
  rep_material:       [u8; ...]           bitpacked LUT indices at lut_bits
  ptr_children:       [u8; ...]           bitpacked node pointers at ptr_bits
  lut_children:       [u8; ...]           bitpacked LUT indices at lut_bits
  ptr_bits:           u8                  bit width for node pointers
  lut_bits:           u8                  bit width for LUT indices (ceil(log2(lut_size)))

ROOT*   one chunk per root (* = chunk index)
  root_node_index:  u32
  lut_count:        u32
  lut_entries:      [u32; lut_count]    raw Voxel values

GRID    the spatial grid
  dims:             [u32; 3]
  child_bits:       u8
  child_count:      u32
  children:         [u8; ...]           bitpacked grid entries at child_bits

SPIX    spatial index: maps 3D positions to chunk indices for the streaming manager
  entry_count:      u32
  entries:          [{pos: [i32; 3], chunk_index: u32}; entry_count]
```

---

## Modules

```
lattice/
  Cargo.toml

  src/
    lattice/
      mod.rs          # Lattice, ChildIter
      svo.rs          # Level (SoA), Chunk (materials + root node index)
      grid.rs         # Grid (flat 3D array, auto-sized)
      node.rs         # LEAF_FLAG and child entry helpers
      voxel.rs        # Voxel (repr(transparent) u32 newtype)
      bitpacked.rs    # BitpackedArray, fixed-width packed storage
      lut.rs          # Lut, per-chunk palette

    import/
      mod.rs          # importer entry point, ImportConfig, VoxelSample
      color.rs        # 256-entry OKLab color palette, nearest-entry lookup (import only)
      gltf/
        mod.rs        # glTF scene loading: triangle binning + per-chunk voxelization
        mesh.rs       # triangle extraction, Sutherland-Hodgman chunk clipping
        material.rs   # PBR material -> Voxel mapping
        voxelizer.rs  # SAT intersection test, texture sampling

    pack/
      mod.rs          # packing entry point, PackConfig
      sort.rs         # k-way merge of per-chunk sorted runs into Morton-ordered stream
      tree.rs         # bottom-up streaming SVO construction
      repack.rs       # pool-size bitpacking after construction
      serialize.rs    # .lattice file writing, DFS encoding

    load/
      mod.rs          # load_lattice (disk->RAM), upload_chunk (RAM->VRAM at given depth)
      header.rs       # .lattice header parsing
      stream.rs       # DFS stream decoding, node index reconstruction
      upload.rs       # GPU buffer management, GpuLattice

    render/
      mod.rs          # renderer entry point, pipeline setup
      tracer.rs       # frame loop, compute pass dispatch, output presentation
      camera.rs       # camera state, uniform buffer layout
      traverse.rs     # traversal pipeline and bind group setup
      gi.rs           # GI pipeline, per-face lighting buffer management
      debug.rs        # debug overlay pipeline and mode uniforms

  shaders/
    common.wgsl       # shared math, type definitions, bitpacked decode helpers
    traverse.wgsl     # 64-tree DDA, LOD early termination
    primary.wgsl      # primary ray dispatch, one thread per pixel
    gi.wgsl           # path tracing bounce loop, BRDF sampling, emissive injection
    accumulate.wgsl   # per-face weighted GI accumulation into the lighting buffer
    debug.wgsl        # debug overlays: normals, depth, voxel index, occupancy

  tools/
    pack.rs           # CLI: scene -> .lattice
    render.rs         # CLI: .lattice -> frames
    inspect.rs        # CLI: print .lattice header and stats
```

---

## Key findings from research

- A 64-tree produces 37% fewer total nodes than an octree on the same scene and traverses faster, especially with the 64-bit child_mask enabling sub-block coalescing.
- The ancestor stack is the highest-impact traversal optimization (~2x). Sub-block coalescing adds ~21%, ray-octant mirroring adds ~10%.
- DFS stream encoding achieves 2.8-3.8x smaller files than pointer-based storage.
- Per-chunk LUTs mean index bit widths reflect per-chunk voxel variety, not global variety. A chunk with 16 unique voxels uses 4-bit indices regardless of what the rest of the scene looks like.
- Inline rep_material per node gives LOD with no runtime cost. The GPU hits LEAF_FLAG, reads rep_material, looks up the chunk LUT, done. No separate rep_voxel array needed at load time.
- Streaming keeps disk->RAM and RAM->VRAM separate. RAM holds full chunks, always. VRAM holds partial-depth chunks based on camera distance. Disk reads are infrequent and off the critical path.

---

## Resources

### YouTube Channels

| Channel | Focus |
|---------|-------|
| [Douglas Dwyer](https://www.youtube.com/@DouglasDwyer) | Octo voxel engine in Rust + WebGPU, path-traced GI |
| [John Lin (Voxely)](https://www.youtube.com/@johnlin) | Path-traced voxel sandbox engine, RTX |
| [Gabe Rundlett](https://www.youtube.com/@GabeRundlett) | Open-source C++ voxel engine with Daxa/Vulkan |
| [Ethan Gore](https://www.youtube.com/@EthanGore) | Voxel engine dev, binary greedy meshing contributor |
| [VoxelRifts](https://www.youtube.com/@VoxelRifts) | Programming explainer videos, voxel focus |
| [SimonDev](https://www.youtube.com/@simondev758) | Accessible intro video on Radiance Cascades |

### Projects and Repos

| Project | Description |
|---------|-------------|
| [Voxel Raymarching](github.com/jamescatania1/voxel-raymarching) | Voxel raymarching with Rust and WGPU |
| [VoxelRT](https://github.com/dubiousconst282/VoxelRT) | Voxel rendering experiments: brickmap, Tree64, XBrickMap, MultiDDA benchmarks |
| [HashDAG](https://github.com/Phyronnaz/HashDAG) | Official open-source implementation of the HashDAG paper (Careil et al. 2020) |
| [Voxelis](https://github.com/WildPixelGames/voxelis) | Pure Rust SVO-DAG crate with batching, reference counting, Bevy/Godot bindings |
| [Octo Engine](https://github.com/DouglasDwyer/octo-release) | Rust + WebGPU voxel engine with ray marching and path-traced GI |
| [BrickMap](https://github.com/stijnherfst/BrickMap) | High performance realtime CUDA voxel path tracer using brickmaps |
| [gvox_engine](https://github.com/GabeRundlett/gvox_engine) | Moddable cross-platform voxel engine in C++ with Daxa/Vulkan |
| [gvox](https://github.com/GabeRundlett/gvox) | General voxel format translation library |
| [VoxelHex](https://github.com/Ministry-of-Voxel-Affairs/VoxelHex) | Sparse VoxelBrick Tree with ray tracing support |
| [tree64](https://github.com/expenses/tree64) | Rust sparse 64-tree with hashing, based on dubiousconst282's guide |
| [binary-greedy-meshing](https://github.com/cgerikj/binary-greedy-meshing) | Fast bitwise voxel meshing |

### Blog Posts

| Resource | Description |
|----------|-------------|
| [A guide to fast voxel ray tracing using sparse 64-trees](https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing/) | Comprehensive guide: 64-tree traversal, brickmap comparison, benchmarks |
| [A Rundown on Brickmaps](https://uygarb.dev/posts/0003_brickmap_rundown/) | Clear explanation of the van Wingerden brickmap/brickgrid structure |
| [The Perfect Voxel Engine](https://voxely.net/blog/the-perfect-voxel-engine/) | John Lin's vision post on voxel engine architecture |
| [A Voxel Renderer for Learning C/C++](https://jacco.ompf2.com/2021/02/01/a-voxel-renderer-for-learning-c-c/) | Two-level grid renderer, solid color bricks, OpenCL, 1B rays/sec |
| [Voxel raytracing](https://tenebryo.github.io/posts/2021-01-13-voxel-raymarching.html) | SVDAG path tracer writeup |
| [Voxelisation Algorithms review](https://pmc.ncbi.nlm.nih.gov/articles/PMC8707769/) | Comprehensive survey of voxel data structures |
| [Voxel.Wiki: Raytracing](https://voxel.wiki/wiki/raytracing/) | Community wiki curating voxel raycasting resources and papers |
| [Amanatides & Woo DDA explainer](https://m4xc.dev/articles/amanatides-and-woo/) | Deep dive into the DDA algorithm with visuals |

### ShaderToy

| Shader | Description |
|--------|-------------|
| [Radiance Cascades 3D (surface-based)](https://www.shadertoy.com/view/X3XfRM) | Surface-based 3D RC, 5 cascades, cubemap storage |
| [Radiance Cascades (volumetric voxel)](https://www.shadertoy.com/view/M3ycWt) | True volumetric 3D RC with voxel raycaster |
| [Amanatides & Woo DDA (branchless)](https://www.shadertoy.com/view/XdtcRM) | Clean branchless 3D DDA implementation |

### Papers

#### Foundational Ray Traversal

| Paper | Link |
|-------|------|
| A Fast Voxel Traversal Algorithm for Ray Tracing, Amanatides & Woo 1987 | [PDF](http://www.cse.yorku.ca/~amana/research/grid.pdf) |
| Efficient Sparse Voxel Octrees, Laine & Karras 2010 | [ResearchGate](https://www.researchgate.net/publication/47645140_Efficient_Sparse_Voxel_Octrees) |
| GigaVoxels: Ray-Guided Streaming for Efficient and Detailed Voxel Rendering, Crassin et al. 2009 | [INRIA](http://maverick.inria.fr/Publications/2009/CNLE09/) |
| Real-time Ray Tracing and Editing of Large Voxel Scenes (Brickmap), van Wingerden 2015 | [Utrecht](https://studenttheses.uu.nl/handle/20.500.12932/20460) |

#### SVDAG Family

| Paper | Link |
|-------|------|
| Hybrid Voxel Formats for Efficient Ray Tracing | [ARXIV](https://arxiv.org/html/2410.14128v1) |
| High Resolution Sparse Voxel DAGs, Kampe, Sintorn, Assarsson 2013 | [PDF](https://icg.gwu.edu/sites/g/files/zaxdzs6126/files/downloads/highResolutionSparseVoxelDAGs.pdf) |
| SSVDAGs: Symmetry-aware Sparse Voxel DAGs, Villanueva, Marton, Gobbetti 2016 | [ACM](https://dl.acm.org/doi/10.1145/2856400.2856406) |
| Interactively Modifying Compressed Sparse Voxel Representations (HashDAG), Careil, Billeter, Eisemann 2020 | [Wiley](https://onlinelibrary.wiley.com/doi/abs/10.1111/cgf.13916) |
| Lossy Geometry Compression for High Resolution Voxel Scenes, van der Laan et al. 2020 | [ACM](https://dl.acm.org/doi/10.1145/3384543) |
| Transform-Aware Sparse Voxel Directed Acyclic Graphs (TSVDAG), Molenaar & Eisemann 2025 | [ACM](https://dl.acm.org/doi/10.1145/3728301) |
| Editing Compact Voxel Representations on the GPU, Molenaar & Eisemann 2024 | [TU Delft](https://publications.graphics.tudelft.nl/papers/13) |
| Editing Compressed High-Resolution Voxel Scenes with Attributes, Molenaar & Eisemann 2023 | [Wiley](https://onlinelibrary.wiley.com/doi/full/10.1111/cgf.14757) |
| PSVDAG: Compact Voxelized Representation of 3D Scenes Using Pointerless SVDAGs, Vokorokos, Mados, Bilanova 2020 | [Computing and Informatics](https://doi.org/10.31577/cai_2020_3_587) |
| Evaluation of Pointerless SVO Encoding Schemes Using Huffman Encoding, Mados et al. 2020 | [IEEE](https://doi.org/10.1109/ICETA51985.2020.9379265) |

#### Color and Attribute Compression

| Paper | Link |
|-------|------|
| Geometry and Attribute Compression for Voxel Scenes (Dado), Dado et al. 2016 | [CGF](https://diglib.eg.org/handle/10.1111/cgf.12841) |
| Compressing Color Data for Voxelized Surface Geometry (Dolonius), Dolonius et al. 2017 | [ACM I3D](https://dl.acm.org/doi/10.1145/3023368.3023381) |

#### Surveys and Hybrid Formats

| Paper | Link |
|-------|------|
| Hybrid Voxel Formats for Efficient Ray Tracing, 2024 | [arxiv](https://arxiv.org/abs/2410.14128) |
| Aokana: A GPU-Driven Voxel Rendering Framework for Open World Games, 2025 | [arxiv](https://arxiv.org/abs/2505.02017) |
| Voxelisation Algorithms and Data Structures: A Review, PMC 2021 | [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC8707769/) |

### Misc

| Resource | Description |
|----------|-------------|
| [Voxel.Wiki](https://voxel.wiki) | Community wiki, good starting hub for voxel rendering resources |
| [Voxely.net blog](https://voxely.net/blog/) | John Lin's blog on voxel engine design |
| [Jacco's voxel blog series](https://jacco.ompf2.com) | Practical renderer tutorials with OpenCL |
