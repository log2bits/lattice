# Lattice

A voxel renderer with full path tracing. Lattice voxelizes a scene, compresses the geometry into a sparse voxel DAG, and renders via GPU path tracing. The tree structure is modular: each level of the hierarchy can be a different layer type, and LUT compression can be applied independently at any level. The header of a `.lattice` file fully describes the structure, so the traversal just follows those instructions at runtime.

---

## Pipeline

```
[import]   scene -> sorted (position, voxel) stream
[pack]     sorted voxel stream -> layered DAG + material data -> .lattice file
[load]     .lattice file -> GPU-ready buffers
[render]   GPU buffers -> path traced image
```

Each stage is independent. The handoff between stages is a well-defined data structure, so you can test any stage in isolation or swap in a different importer.

---

## Design choices that are fixed

- Each level of the tree is one of three layer types: Geometry DAG, Material DAG, or Grid. No other layer types.
- Every level uses SoA layout. No AoS.
- All voxel references resolve through the global voxel LUT to a fixed 32-bit format. Per-section-root LUTs and pool-size bitpacking compress the intermediate indices. The `Lut<T>` + `BitpackedArray` primitives are reused for all of these.
- The renderer is always full path tracing. No rasterization fallback, no hybrid primary visibility.
- Face normals are derived from the DDA exit face at traversal time. No per-voxel normals are stored.
- No LOD. Every ray traverses to the leaf.
- No editing during rendering. The data is built once and uploaded. Read-only at runtime.

---

## The three layer types

Every child entry everywhere in the tree is a u32. Bit 31 (LEAF_FLAG) is always the same: set means this is a leaf, clear means it's a pointer to the next level's node pool. What the remaining bits mean depends on the layer type, which the header tells you.

### Geometry DAG

A geometry DAG level deduplicates on geometry only. Two nodes with the same occupancy pattern but different materials hash the same and share a node. Material data lives in a separate flat array called the materials array, indexed by a running count the ray maintains during traversal. This is the Dolonius method: each descent step adds the `voxel_count` of skipped sibling subtrees to a running offset, so when a leaf is hit, that offset points directly into the materials array.

Uniform subtrees (LEAF_FLAG set) don't contribute to the materials array and don't advance the running count. Their material either sits inline in the child entry or is handled by the level above.

This layer type is right for scenes with lots of geometric repetition and varied materials. The DAG stays tight in cache because it carries no material payload.

### Material DAG

A material DAG level deduplicates on geometry and material together. Two nodes only share a node if both their occupancy and their leaf data match exactly. Material data is inline in the leaf entries, not in a separate flat array.

This is right for the bottom levels of scenes where material variation is low and you want the DAG to capture both geometric and material repetition. A Minecraft chunk is a good example since most blocks are a single material with low variation.

### Grid

A grid level is just a flat 3D array of u32 child entries. No tree structure, no deduplication. Each entry either points to a sub-DAG in the level below or is empty. Grid is right for the top of the hierarchy where deduplication almost never fires, and it avoids tree traversal overhead in regions that are nearly all unique.

---

## SoA layout

Every level stores its fields in Structure of Arrays layout. All fields are stored in separate contiguous arrays, each indexed by node index.

- `occupancy: [u64]` -- one bit per child slot. A 1 bit means a child exists there.
- `voxel_count: [u32]` -- total materials array entries contributed by this subtree. Only present on Geometry DAG levels. Uniform subtrees contribute 0.
- `children_start: [u32]` -- where this node's children begin in the flat children array.
- `children: [u32]` -- packed child entries for all nodes at this level, laid out contiguously.

Only occupied children are stored. `occupancy[i].count_ones()` gives the child count for node `i`.

SoA layout matters for GPU traversal. A warp of 32 threads reading `occupancy` for 32 different nodes touches one contiguous memory region. AoS would scatter those reads across memory, causing cache misses for every field access.

---

## Sections

A section is a consecutive group of levels sharing the same layer type. Sections are the unit of LUT ownership: each section root gets its own LUT covering all leaf child entries within its subtree.

The Minecraft configuration has three sections:

```
Section 1: Grid         (1 level)   -- spatial index into block DAGs
Section 2: Geometry DAG (3 levels)  -- block geometry dedup across the world
Section 3: Material DAG (2 levels)  -- voxel color dedup within each block
```

Expressed in code:

```rust
let lattice = Lattice::new(&[
    SectionConfig::grid(1),
    SectionConfig::geometry_dag(3).with_lut(),
    SectionConfig::material_dag(2).with_lut(),
]);
```

Child pointers within a section (level N to level N+1) use pool-size bitpacking: after construction, the node pool at each level is a dense sequential array, so the minimum power-of-two bit width to address it is computed and all entries are repacked. No LUT table is needed.

Child entries at the bottom of a section are global voxel LUT indices. These get per-section-root LUT compression: each section root builds a small local LUT of the unique global voxel LUT indices its subtree references, stores those as bitpacked local indices, and the local LUT entries are the global indices. Different section roots reference small disjoint subsets of the global voxel LUT, so local bit widths are much narrower than the global index width.

---

## LUT compression

The core primitive is a `BitpackedArray`: a flat array of values stored at a fixed power-of-two bit width (1, 2, 4, 8, 16, or 32). Powers of two mean the GPU extracts any entry with a single shift and mask. `BitpackedArray` supports converting its bit width after construction, which is how both compression strategies finalize their storage: build with full u32 values, determine the minimum bit width once sizes are known, repack.

On top of `BitpackedArray` is a generalized `Lut<T>`: a flat array of unique values of type `T`, plus a `BitpackedArray` of indices into those values. This pattern appears four times in the structure:

1. **DAG node pools** -- each node pool is a set of unique nodes; intra-section child pointers are bitpacked indices into it. The pool IS the value table. No separate index array is needed since the pool indices are already sequential.
2. **Global voxel LUT** -- all unique 32-bit voxels in the scene, deduplicated across all sections. Everything ultimately resolves to an index into this table.
3. **Per-section-root LUT** -- for each section root, a local table of the unique global voxel LUT indices its subtree references. In-tree leaf entries are bitpacked local indices into this table.
4. **Materials array LUT** -- for Geometry DAG sections, the Dolonius materials array is bitpacked global voxel LUT indices. A small LUT of unique global indices maps the compressed stream entries to actual voxel data.

### Pool-size packing

After construction, the node pool for each level is a dense sequential array. The pool size determines the bit width. Every intra-section child pointer at that level is repacked at that width. No LUT table is needed since the pool itself is the value set.

### Global voxel LUT

All unique 32-bit voxels in the scene are collected into a single flat table during packing. The table size determines a single global bit width, `voxel_bits`, that is stored in the file header. All materials array entries and all per-section-root LUT entries use this bit width.

### Per-section-root LUT

At the bottom of a section, each section root gets a local table of the unique global voxel LUT indices within its subtree. In-tree leaf entries are bitpacked local indices into this table. The local table size determines a per-root bit width that is typically much narrower than `voxel_bits`.

The local table is owned by the section root node and shared by all instances through DAG node sharing. LUT memory scales with unique content, not scene size. In the Minecraft geometry section, the global voxel LUT might have ~10,000 entries (14 bits), but each block chunk's local table has ~100 entries (7 bits).

### Materials array

For Geometry DAG sections, the materials array stores one global voxel LUT index per non-uniform leaf in Dolonius DFS order. The entries are bitpacked at `voxel_bits`. A small local LUT of unique indices within the materials array can compress further if the section's voxel variety is low.

### Bit widths

```
Entry count    ->    bit width
1                    1 bit
2-3                  2 bits
4-15                 4 bits
16-255               8 bits
256-65535            16 bits
65536+               32 bits (no benefit, store raw)
```

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

The shader always decodes the same layout. No per-scene variants, no header fields for field offsets. All references to voxel data in the tree are indices into the global voxel LUT, which stores the full 32-bit values.

---

## The color palette

The color palette is a perceptually uniform 256-entry spread across OKLab space, precomputed using sample elimination. It doesn't change between scenes. At import, each sampled surface point maps to its nearest palette entry, which sets the 24-bit RGB field of the voxel.

Using the palette keeps the global voxel LUT small. With 256 possible colors and a handful of material flag combinations, the total unique voxel count is bounded and typically sits in the low thousands. A smaller global voxel LUT means smaller `voxel_bits`, which reduces the cost of every per-section-root LUT entry and every materials array entry across the entire scene.

---

## Example configurations

### Bistro

The Bistro is a dense architectural scene with lots of geometric repetition but varied materials. Colors are quantized to the 256-entry palette at import, bounding the global voxel LUT to a few thousand entries. `voxel_bits` typically lands at 16.

```rust
let lattice = Lattice::new(&[
    SectionConfig::grid(1),
    SectionConfig::geometry_dag(3).with_lut(),
]);
```

- Section 1, Grid (1 level): spatial index into the geometry DAG trees below.
- Section 2, Geometry DAG (3 levels): deduplicates block geometry. Material data lives in the materials array in Dolonius DFS order, bitpacked at `voxel_bits`. Each section root gets a local LUT of the global voxel LUT indices its subtree references. Most trees only touch a small slice of the global table so local indices compress to 8 bits or fewer.

### Minecraft world

A Minecraft world has a two-tier structure. Each block is a small voxel DAG. The world is a large DAG of blocks.

```rust
let lattice = Lattice::new(&[
    SectionConfig::grid(1),
    SectionConfig::geometry_dag(3).with_lut(),
    SectionConfig::material_dag(2).with_lut(),
]);
```

- Section 1, Grid (1 level): spatial index of 64^3 chunk pointers. 64^3 regions are almost never identical so dedup doesn't help here.
- Section 2, Geometry DAG (3 levels): deduplicates block geometry across the world. The global block pool has ~10,000 unique block types, but each chunk only references ~100, so per-section-root LUT compresses geometry section leaf entries to 7 bits. Because block DAGs are deduplicated, the number of LUTs equals the number of unique block types, not block instances. A sandstone block appearing a million times has one LUT.
- Section 3, Material DAG (2 levels): deduplicates voxel data within each block. Each block uses fewer than 256 unique colors, so the global voxel LUT has at most a few thousand entries and `voxel_bits` is 16. Each block's per-section-root local LUT has fewer than 16 entries, so in-tree leaf entries compress to 4 bits. That's a 4x reduction from `voxel_bits` and an 8x reduction from raw 32-bit voxels.

The raw voxel count for a full Minecraft world is astronomical, but after two levels of dedup the actual node count sits in the millions. The whole thing fits in a few GB. u32 indices are fine throughout since dedup means you're counting unique subtrees, not individual voxels.

---

## Traversal

### Finding the hit

The ray traverses the tree level by level. At each node, the occupancy mask tells the shader which children exist. Empty children are skipped immediately. The layer type of the current level tells the shader how to handle a leaf: Geometry DAG levels do a Dolonius materials array lookup at hit time, Material DAG levels decode the child entry through the per-section-root LUT to get a global voxel LUT index, then read the 32-bit voxel from the global table. Grid levels follow the pointer to the next level.

Intra-section child pointers are decoded with a single shift and mask using the level's fixed bit width. At section boundaries, leaf entries are decoded through the section root's LUT before being used. The LUT is small and cache-resident so this adds no meaningful cost.

An ancestor stack caches parent node indices so the ray doesn't re-descend from root on every step. For Geometry DAG levels, the stack also carries the running Dolonius material base offset, updated by one addition per descent step.

When a leaf is hit, the face normal comes from whichever face the DDA ray exited. Nothing about the normal is stored in the voxel payload.

### Traversal optimizations

**Fractional coordinate encoding.** The tree lives in [1.0, 2.0). IEEE floats in that range have exponent zero, so the mantissa encodes position as fixed-point. Extracting the cell index at any level is two bit operations with no multiply or divide. Descending decrements `scale_exp` by 2.

**Ancestor stack.** Caches parent node indices and Dolonius base offsets. When the ray steps to a neighbor, comparing old and new position bits identifies the highest level that changed. The stack has the right values ready. About 2x speedup over root-to-leaf descent.

**2x2x2 sub-block coalescing.** Before stepping, the traversal checks whether the 2x2x2 sub-block containing the current cell is entirely empty in the 64-bit occupancy mask. If so, the step size doubles. One mask and compare on an existing field. Adds about 21% more empty space skipped for free.

**Ray-octant mirroring.** The coordinate system is mirrored to the negative ray octant at traversal start. With all direction components negative, finding the exit face is just finding the minimum of three distances with no sign conditionals. About 10% faster.

---

## Lighting

Lighting is full path tracing. Primary rays dispatch from the camera. On each hit, the shader reads the voxel payload, derives the face normal from the DDA exit face, samples the BRDF based on roughness and the metallic flag, and dispatches a secondary ray. Emissive voxels contribute light directly. Shadow rays aren't a separate pass, they come out of the path tracing loop naturally.

Each voxel face stores a running weighted average of accumulated indirect light:

```
L_new = (1 - alpha) * L_old + alpha * S
```

`S` is the new path traced sample. `alpha` is tuned per surface type: small for diffuse (stable accumulation over many frames) and larger for specular (faster response to view changes). No sample history buffers, no reservoirs. One color value per face.

Transparent voxels refract the ray using the face normal. Metallic voxels tint the specular lobe by albedo. Emissive voxels inject radiance into the path without consuming a bounce.

---

## Disk format: PSVDAG

On disk, sub-node references are removed from the children arrays. Nodes are written depth-first. The first time a unique node appears it gets a label. Every subsequent reference to the same node is a back-reference to that label. LEAF_FLAG entries are written as-is since they're terminal values.

The loader reconstructs explicit sub-node indices by walking the DFS stream. Loading is close to a direct copy.

PSVDAG achieves 2.8-3.8x smaller files than pointer-based SVDAG. Every repeated node appears once in the DFS stream and is referenced cheaply everywhere else.

---

## The .lattice file format

```
Header:
  magic:           [u8; 4]     "LTCE"
  version:         u16
  flags:           u16
  world_min:       [i64; 3]    voxel-space coordinates
  world_max:       [i64; 3]
  num_sections:    u8
  num_levels:      u8          total levels across all sections
  voxel_bits:      u8          bit width for global voxel LUT indices (1/2/4/8/16/32)
  _pad:            u8
  sections:        [SectionDesc; num_sections]
  levels:          [LevelDesc; num_levels]
  chunk_count:     u32
  chunks:          [{tag: u32, offset: u64, size: u64}; chunk_count]

SectionDesc:
  layer_type:      u8          0 = Grid, 1 = Geometry DAG, 2 = Material DAG
  lut_enabled:     u8          0 = no per-section-root LUT, 1 = enabled
  num_levels:      u8          number of levels in this section
  _pad:            u8

LevelDesc:
  child_bits:      u8          bit width for intra-section child pointers (1/2/4/8/16/32)
  _pad:            [u8; 3]
```

Chunks:

```
LVL*    one chunk per tree level (* = level index, 0 = root)
  node_count:       u32
  occupancy:        [u64; node_count]          SoA field
  voxel_count:      [u32; node_count]          SoA field, Geometry DAG levels only
  children_start:   [u32; node_count]          SoA field
  children:         [u8; ...]                  intra-section child pointers, bitpacked
                                               at child_bits, PSVDAG DFS order

SEC*    one chunk per section with lut_enabled (* = section index)
  root_count:       u32
  roots:            [SectionRootDesc; root_count]

SectionRootDesc:
  root_node_index:  u32
  lut_index_bits:   u8          per-root bit width for in-tree leaf entries (1/2/4/8/16/32)
  _pad:             [u8; 3]
  lut_entry_count:  u32
  lut_entries:      [u32; lut_entry_count]     global voxel LUT indices
  leaf_offset:      u64         byte offset into the bottom level's children array
                                where this root's bitpacked leaf entries begin

MATL    one chunk per Geometry DAG section -- the Dolonius materials array
  entry_count:  u32
  entries:      [u8; ...]       global voxel LUT indices, bitpacked at voxel_bits

VOXL    global voxel LUT
  entry_count:  u32
  entries:      [u32; entry_count]   32-bit voxel values

PAL     global color palette
  entry_count:  u32                  always 256
  entries:      [[u8; 3]; 256]       linear RGB, perceptually uniform OKLab spread

SPIX    spatial index
  entry_count:  u32
  entries:      [{pos: [i32; 3], root_node_index: u32}; entry_count]
```

---

## Modules

```
lattice/
  Cargo.toml

  src/
    lattice/
      mod.rs          # Lattice, SectionConfig, LevelConfig, Level (SoA), layer type definitions
      node.rs         # LEAF_FLAG and child entry helpers
      voxel.rs        # Voxel (32-bit format), ColorPalette
      bitpacked.rs    # BitpackedArray -- fixed-width packed storage, width conversion
      lut.rs          # Lut<T> -- unique value table + BitpackedArray of indices

    import/
      mod.rs          # importer entry point, VoxelChunk output type
      palette.rs      # global color palette, nearest-entry lookup
      gltf/
        mod.rs        # glTF scene loading, chunk dispatch
        mesh.rs       # mesh data extraction, triangle clipping to chunks
        material.rs   # PBR material -> voxel payload mapping
        voxelizer.rs  # SAT intersection test, texture sampling

    pack/
      mod.rs          # packing entry point
      sort.rs         # k-way merge of sorted chunk streams (Morton order)
      dag.rs          # bottom-up streaming DAG construction, layer type dispatch
      lut.rs          # global voxel LUT construction, per-section-root LUT construction
      materials.rs    # Dolonius materials array, streamed to temp file, bitpacked at voxel_bits
      serialize.rs    # .lattice file writing, PSVDAG encoding

    load/
      mod.rs          # loader entry point
      header.rs       # .lattice header parsing, level descriptors, section index
      stream.rs       # PSVDAG DFS stream decoding, node index reconstruction
      upload.rs       # CPU -> GPU buffer upload

    render/
      mod.rs          # renderer entry point
      tracer.rs       # render loop, pass orchestration
      camera.rs       # camera state, ray generation
      traverse.rs     # 64-tree DDA, layer type dispatch, LUT decode, Dolonius lookup, face normals
      gi.rs           # path tracing, per-face accumulation
      debug.rs        # debug overlay passes

  shaders/
    common.wgsl       # shared math, type definitions, payload decode helpers
    traverse.wgsl     # 64-tree DDA, per-tree LUT decode, Dolonius index accumulation
    primary.wgsl      # primary ray dispatch
    gi.wgsl           # path tracing bounce loop
    accumulate.wgsl   # per-face weighted GI accumulation
    debug.wgsl        # debug overlays

  tools/
    pack.rs           # CLI: scene -> .lattice
    render.rs         # CLI: .lattice -> frames
    inspect.rs        # CLI: print .lattice header and stats
```

---

## Key findings from research

- A 64-tree produces 37% fewer total nodes than an octree on the same scene and traverses faster, especially with the 64-bit occupancy mask enabling sub-block coalescing.
- Geometry-only DAG deduplication with the Dolonius attribute method adds about 1% to DAG size compared to a pure geometry DAG. Per-pointer attribute methods (Dado et al.) add about 2x. For path tracing where traversal dominates, Dolonius is the right call.
- Leaf-level deduplication is where almost all savings happen. At the 4^3 level, 77% of nodes are duplicates. At the 64^3 level, 0.1%.
- The ancestor stack is the highest-impact traversal optimization (~2x). Sub-block coalescing adds ~21%, ray-octant mirroring adds ~10%.
- PSVDAG-style encoding achieves 2.8-3.8x smaller files than pointer-based SVDAG.
- SVO and DAG use the same traversal algorithm. The DAG's advantage is purely cache efficiency from having fewer unique nodes in memory.
- LUT compression scales with unique content, not scene size. In a Minecraft world, the number of LUTs equals the number of unique block types after dedup, not the number of block instances. A block appearing a million times has one LUT.
- Chaining small LUT lookups at hit time is basically free if the tables stay cache-resident. A 256-entry LUT is 1KB and fits in L1 easily.
- With u32 indices and strong DAG dedup, even a Minecraft-scale world stays within u32 range. The index limit applies to unique nodes, not raw voxels.

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