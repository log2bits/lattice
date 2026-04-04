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

- Each level of the tree is one of three layer types: Geometry DAG, Standard DAG, or RAW. No other layer types.
- Every level uses SoA layout. No AoS.
- LUT compression is optional and independent per level.
- The renderer is always full path tracing. No rasterization fallback, no hybrid primary visibility.
- Face normals are derived from the DDA exit face at traversal time. No per-voxel normals are stored.
- No LOD. Every ray traverses to the leaf.
- No editing during rendering. The data is built once and uploaded. Read-only at runtime.

---

## The three layer types

Every child entry everywhere in the tree is a u32. Bit 31 (LEAF_FLAG) is always the same: set means this is a leaf, clear means it's a pointer to the next level's node pool. What the remaining bits mean depends on the layer type, which the header tells you.

### Geometry DAG

A geometry DAG level deduplicates on geometry only. Two nodes with the same occupancy pattern but different materials hash the same and share a node. Material data lives in a separate flat array, indexed by a running count the ray maintains during traversal. This is the Dolonius method: each descent step adds the `voxel_count` of skipped sibling subtrees to a running offset, so when a leaf is hit, that offset points directly into the flat material array.

Uniform subtrees (LEAF_FLAG set) don't contribute to the material array and don't advance the running count. Their material either sits inline in the child entry or is handled by the level above.

This layer type is right for scenes with lots of geometric repetition and varied materials. The DAG stays tight in cache because it carries no material payload.

### Standard DAG

A standard DAG level deduplicates on geometry and material together. Two nodes only share a node if both their occupancy and their leaf data match exactly. Material data is inline in the leaf entries, not in a separate flat array.

This is right for the bottom levels of scenes where material variation is low and you want the DAG to capture both geometric and material repetition. A Minecraft chunk is a good example since most blocks are a single material with low variation.

### RAW

A RAW level is just a flat 3D grid of u32 child entries. No tree structure, no deduplication. Each entry either points to a sub-DAG in the level below or is empty. RAW is right for the top of the hierarchy where deduplication almost never fires, and it avoids tree traversal overhead in regions that are nearly all unique.

---

## SoA layout

Every level stores its fields in Structure of Arrays layout. All fields are stored in separate contiguous arrays, each indexed by node index.

- `occupancy: [u64]` -- one bit per child slot. A 1 bit means a child exists there.
- `voxel_count: [u32]` -- total Dolonius MAT entries contributed by this subtree. Only present on Geometry DAG levels. Uniform subtrees contribute 0.
- `children_start: [u32]` -- where this node's children begin in the flat children array.
- `children: [u32]` -- packed child entries for all nodes at this level, laid out contiguously.

Only occupied children are stored. `occupancy[i].count_ones()` gives the child count for node `i`.

SoA layout matters for GPU traversal. A warp of 32 threads reading `occupancy` for 32 different nodes touches one contiguous memory region. AoS would scatter those reads across memory, causing cache misses for every field access.

---

## LUT compression

LUT compression reduces the bit width of child entries at the bottom layer of any level by replacing full u32 values with short indices into a lookup table of unique values. It applies per tree root, not per level globally. So if a level contains 64 independent trees, each tree gets its own LUT.

### Why per tree root

The LUT belongs to a tree, not to a level. This matters because of how DAG deduplication interacts with LUT compression. In the Minecraft example, even though the world contains millions of block instances, the block pool only contains a few thousand unique blocks after dedup. Each unique block has exactly one LUT, shared by all instances of that block everywhere in the world via the DAG node sharing. LUT memory scales with unique content, not scene size.

### Construction algorithm

LUT compression is built during tree construction, not as a separate post-process pass. As child entries are added to a tree, a hashmap tracks every unique u32 value seen. When a new unique value is encountered, it gets appended to the LUT. The child entry stored in the tree is always a u32 index into the LUT rather than the raw value directly.

At this stage, all indices are full u32 values. The LUT is just a flat array of unique u32s.

Once the tree is fully constructed, the LUT length is known. The minimum power-of-two bit width needed to address all entries is computed (1, 2, 4, 8, or 16 bits). All the u32 indices in the tree's child entries are then repacked into a custom bitpacked array at that width. Powers of two are required so the GPU can extract an index with a single bit shift and mask.

Because the bit width changes during repacking, a custom bitpacked array type handles the storage. It supports converting its internal width after the fact, which is the operation that makes repacking straightforward.

### LUT size and bit widths

```
LUT entry count    ->    index bit width
1                         1 bit
2 - 3                     2 bits
4 - 15                    4 bits
16 - 255                  8 bits
256 - 65535               16 bits
65536+                    no compression (store raw u32)
```

The bit width for each tree's LUT is stored in the section data alongside the LUT itself. The GPU reads it once per tree root and uses it for all child entry lookups within that tree.

### What gets compressed

LUT compression applies to the bottom layer of the level it's configured on. Upper layers within the same level still use raw u32 node indices, since those are structural pointers and their range is determined by the node pool size, not the number of unique values. Only the leaf values at the bottom of the tree benefit from LUT compression, since that's where the value space is small and enumerable.

---

## Voxel payloads

The voxel payload format varies by scene and is fully described in the `.lattice` header. The header stores the bit layout so the GPU knows how to decode a leaf value: where the color index is, where roughness is, where the flag bits are, and how wide the total payload is. The shader reads this once as uniforms and uses them for every hit. No separate shader variant is needed per scene.

For a scene like the Bistro, the payload is 16 bits:

```
Voxel (16 bits):

  bit  15     reserved
  bit  14     transparent  refracts rather than reflects
  bit  13     metallic     conductor, albedo tints specular
  bit  12     emissive     emits light at its albedo color
  bits 11-8   roughness    nibble, 0 = mirror, 15 = fully diffuse
  bits  7-0   palette      index into the global 256-entry color palette
```

For a scene like a Minecraft world where you want a full 24-bit color, the payload is 32 bits with a different layout. The traversal code doesn't care. It just reads the layout from the header uniforms and extracts fields with bit shifts.

Because payload size varies per scene, the material array entry size varies too. Bistro uses 16-bit entries, Minecraft uses 32-bit. The header stores this so the loader reads the MAT section correctly.

---

## The global color palette

The color palette for model imports is a fixed global 256-entry table, precomputed once as a perceptually uniform spread across OKLab space. It doesn't change between scenes. OKLab is perceptually uniform, so a uniform spread minimizes worst-case visible error for any input color. The palette is 768 bytes and stays in L1 cache permanently once uploaded.

At import time, each sampled surface point maps to its nearest palette entry. Scenes that use the 8-bit palette color field in their voxel payload get this automatically.

---

## Example configurations

### Bistro

The Bistro is a dense architectural scene with lots of geometric repetition but varied materials. Colors are snapped to the global 256-entry color palette at import. There are potentially thousands of unique voxel payloads globally (up to 2^15 in theory given 256 colors and the material flag bits), but any individual tree usually has far fewer. Per-tree LUT compression often brings most trees down to 8-bit or fewer indices.

A typical configuration:

- Top level: RAW flat grid acting as a spatial index into the DAG trees below. No LUT compression since large regions are mostly unique.
- Middle levels: Geometry DAG. Each tree root gets its own LUT. After construction, the LUT is checked and indices are repacked to the minimum bit width. Most trees compress to 8 bits or fewer.
- Material data: flat MAT array of 16-bit voxel payloads in Dolonius DFS order.

### Minecraft world

A Minecraft world has a two-tier structure. Each block is a small voxel DAG. The world is a large DAG of blocks.

A typical configuration:

- Top level: RAW flat grid of 64^3 chunk pointers. 64^3 regions of a Minecraft world are almost never identical so dedup doesn't help here.
- Middle levels: Geometry DAG with LUT compression. Deduplicates block geometry across the world. Most 64^3 regions only use around 100 unique block configurations so LUT indices compress to 8 bits. Because the block DAGs are deduplicated, the number of LUTs equals the number of unique blocks, not the number of block instances. A sandstone block appearing a million times has one LUT shared by all million instances.
- Bottom levels: Standard DAG with LUT compression. Deduplicates voxel geometry and material together within each block. Individual block trees usually have very few unique voxel types so LUT indices often compress to 4 bits.

The raw voxel count for a full Minecraft world is astronomical, but after two levels of dedup the actual node count sits in the millions. The whole thing fits in a few GB. u32 indices are fine throughout since dedup means you're counting unique subtrees, not individual voxels.

---

## Traversal

### Finding the hit

The ray traverses the tree level by level. At each node, the occupancy mask tells the shader which children exist. Empty children are skipped immediately. The layer type of the current level tells the shader how to handle a leaf: Geometry DAG levels do a Dolonius material lookup at hit time, Standard DAG levels read the payload inline from the child entry, RAW levels follow the pointer to the next level.

If a level has LUT compression, the child entry is first decoded through that tree's LUT before being used. The LUT is small and cache-resident so this adds no meaningful cost.

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
  magic:          [u8; 4]     "LTCE"
  version:        u16
  flags:          u16
  world_min:      [i64; 3]    voxel-space coordinates
  world_max:      [i64; 3]
  num_levels:     u8
  payload_bits:   u8          total bit width of a voxel payload (16 or 32)
  payload_layout: u32         packed description of payload field offsets and widths
  mat_entry_size: u8          bytes per MAT array entry (2 or 4)
  _pad:           [u8; 3]
  levels:         [LevelDesc; num_levels]
  section_count:  u32
  sections:       [{tag: u32, offset: u64, size: u64}; section_count]

LevelDesc:
  layer_type:     u8          0 = RAW, 1 = Geometry DAG, 2 = Standard DAG
  lut_enabled:    u8          0 = no LUT compression on this level, 1 = enabled
  _pad:           [u8; 2]
```

Section tags:

```
0x4C564C**  LVL*    one section per tree level (* = level index, 0 = root)
  node_count:       u32
  occupancy:        [u64; node_count]       SoA field
  voxel_count:      [u32; node_count]       SoA field, Geometry DAG levels only
  children_start:   [u32; node_count]       SoA field
  children:         [u32; total_children]   bitpacked if LUT enabled for this level
                    encoded in PSVDAG DFS order

  if lut_enabled:
    tree_count:     u32
    trees:          [TreeLutDesc; tree_count]

TreeLutDesc:
  root_node_index:  u32
  lut_bits:         u8        actual index bit width used (1/2/4/8/16)
  _pad:             [u8; 3]
  lut_entry_count:  u32
  lut_entries:      [u32; lut_entry_count]   full u32 values before compression
  children_offset:  u64       byte offset into this level's children array
                              where this tree's bitpacked indices begin

0x4D415400  MAT     flat material array (Dolonius DFS order, Geometry DAG levels only)
  entry_count:  u32
  entries:      [u16 or u32; entry_count]   size set by mat_entry_size in header

0x50414C00  PAL     global color palette
  entry_count:  u32                         always 256
  entries:      [[u8; 3]; 256]              linear RGB, perceptually uniform OKLab spread

0x53504958  SPIX    spatial index
  entry_count:  u32
  entries:      [{pos: [i32; 3], root_node_index: u32}; entry_count]
```

---

## Modules

```
lattice/
  Cargo.toml

  src/
    dag/
      mod.rs          # Dag, Level (SoA), layer type definitions
      node.rs         # LEAF_FLAG and child entry helpers
      voxel.rs        # voxel payload description, ColorPalette
      bitpacked.rs    # custom bitpacked array type, width conversion

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
      lut.rs          # per-tree LUT construction, hashmap dedup, bitwidth selection, repacking
      materials.rs    # Dolonius material array, streamed to temp file
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