# Lattice

A voxel renderer with full path tracing. Lattice voxelizes a glTF scene, compresses the geometry into a sparse voxel DAG, stores material data in a separate flat array using the Dolonius method, and renders via GPU path tracing.

---

## Pipeline

```
[import]   glTF scene -> sorted (position, voxel) stream
[pack]     sorted voxel stream -> geometry DAG + material array -> .lattice file
[load]     .lattice file -> GPU-ready buffers
[render]   GPU buffers -> path traced image
```

Each stage is independent. The handoff between stages is a well-defined data structure, so you can test any stage in isolation or swap in a different importer.

---

## Design choices that are fixed

- The runtime tree structure is always a **64-tree** (4x4x4 branching per node). No octree mode.
- The renderer is always **full path tracing**. No rasterization fallback, no hybrid primary visibility.
- The entire scene is one single DAG. No per-region trees, no separate geometry table.
- Geometry and material data are stored separately. The DAG is geometry-only; materials live in a flat array indexed by the Dolonius voxel-count method.
- No LOD. Every ray traverses to the leaf.
- No editing during rendering. The data is built once and uploaded. Read-only at runtime.

---

## Voxelization pipeline

Voxelization is split from DAG construction. The voxelizer produces a sorted stream of `(position, voxel)` pairs; the packer consumes that stream and builds the DAG. This separation keeps memory bounded and makes the two stages independently testable.

### Color palette first

Before voxelizing any geometry, the importer samples all textures in the glTF and runs k-means++ clustering in OKLab space to produce a 256-color palette. OKLab is perceptually uniform, so k-means minimizes visible banding. The full palette is 768 bytes. It stays in L1 cache permanently once uploaded.

Sampling textures to build the palette is fast (a few thousand pixels per texture is plenty) and means voxelization runs in a single pass -- every sampled surface point gets a palette index immediately, no second pass needed.

### Chunked parallel voxelization

The scene bounding box is divided into 256^3 voxel chunks. Each chunk is processed by a thread pool worker that takes all triangles intersecting that chunk and voxelizes them. Triangles are clipped to the chunk boundary before voxelization so nothing is processed twice and no holes appear at edges.

Each worker outputs a sorted-by-Morton-code list of `(position, voxel)` pairs. Only surface voxels are emitted -- interior voxels that are completely surrounded by opaque neighbors on all six faces are culled at this stage.

In practice, occupancy in any given chunk is low (1-3% for typical glTF scenes), so each chunk's voxel list is a few MB at most even though the worst-case bound is 67MB (256^3 * 4 bytes).

### Streaming sort and DAG construction

Once all chunks are voxelized, a k-way merge of the sorted chunk outputs produces a globally sorted voxel stream in Morton order. This merge is streaming -- only one chunk's data is in memory per thread at a time.

The DAG builder consumes this sorted stream bottom-up. Consecutive voxels that share a parent node in the 64-tree form a group; the builder processes each group, hashes the resulting node, deduplicates it, and moves up a level. Identical geometric subtrees naturally share nodes regardless of their materials.

DAG deduplication is global and happens in memory. The geometry DAG is small enough that this is fine. The material array is built in parallel and streamed directly to a temp file, since it can be large and doesn't need to be in memory all at once. At the end, the packer writes the .lattice file: geometry sections first, then the material section appended from the temp file, then the header is finalized with the correct section offsets.

---

## The DAG

The DAG is geometry-only. It stores solid/empty occupancy, nothing else. Because material data is absent, any two subtrees with the same geometric shape but different materials deduplicate to the same node. A wall with one texture and the same wall with a different texture share a node in the DAG.

This maximizes the dedup ratio. For a scene like the Bistro, many surfaces are geometrically repetitive even when their materials vary. The geometry DAG also stays tight in cache during traversal since its nodes carry no material payload.

The tradeoff is that material lookup requires computing a running index. During traversal, each ray accumulates a count of voxels in preceding sibling subtrees. When it hits a leaf, that count is the offset into the flat material array. This is the Dolonius (2017) method. The overhead is one addition per traversal step.

---

## How the 64-tree is structured

Each level stores its nodes in SoA layout. All fields are indexed by node index.

- `occupancy: Vec<u64>` -- one bit per child slot. A 1 bit means a child exists there.
- `voxel_count: Vec<u32>` -- total number of leaf voxels in this subtree. Used by Dolonius traversal to compute the material array index.
- `children_start: Vec<u32>` -- where this node's children begin in the flat children array.
- `children: Vec<u32>` -- packed child entries for all nodes in this level.

Only occupied children are stored. `occupancy[i].count_ones()` gives the child count.

The SoA layout matters for GPU traversal. A warp of 32 threads reading occupancy for 32 different nodes touches one contiguous memory region. AoS would scatter those reads.

Each child entry is a raw u32. Bit 31 (LEAF_FLAG) distinguishes two cases:

- LEAF_FLAG set: the lower bits are a material LUT index. Every voxel in that subtree has the same material. A completely uniform region collapses to one entry at any level.
- LEAF_FLAG clear: the lower bits are a node index into the next level's pool.

---

## Material data: the Dolonius method

The flat material array stores LUT indices in the same order as a depth-first traversal of the geometry DAG would visit leaves. Each node stores `voxel_count`, the total number of leaf voxels in its subtree.

During traversal, when the ray descends into a child, it adds up the `voxel_count` of every preceding sibling. That running sum is the material base index for the subtree the ray is entering. When the ray hits a leaf, `material_base + leaf_position` gives the exact offset into the flat material array.

This never reads material data during the traversal phase, only at hit time. The geometry traversal stays tight in cache.

Dolonius adds about 1% overhead to the DAG compared to pure geometry-only. Per-pointer approaches (Dado et al. 2016) add about 2x. For path tracing where traversal dominates, the 1% overhead is well worth the cache benefit.

---

## Voxel format

The material LUT is a flat array of deduplicated `Voxel` entries. Multiple positions in the flat material array can point to the same LUT entry, but `Voxel` values are stored once. The LUT is small enough to stay warm in cache.

```
Voxel (4 bytes, packed u32):

  bits 31-15  normal      17-bit John White signed octahedral encoding
  bit  14     transparent refracts rather than reflects
  bit  13     metallic    conductor, albedo tints specular
  bit  12     emissive    emits light at its albedo color
  bits 11-8   roughness   0 = perfect mirror, 15 = fully diffuse
  bits  7-0   palette     index into the scene's 256-entry color palette
```

**Normal encoding.** Normals use John White's signed octahedral encoding. Project the normal onto the L1 unit octahedron, rotate 45 degrees to redistribute quantization error uniformly, then store X and Y as u8 each plus a sign bit for Z. Average error is about 0.3 degrees. Decoding is one normalize in the shader.

**Color palette.** 256 linear RGB entries, 768 bytes. Built at import time by sampling glTF textures and running k-means++ in OKLab space. Every voxel stores an 8-bit index into it.

The number of unique voxel types determines `mat_index_bits`, the bit width needed to index the LUT:

```
log2(lut_entry_count) -> round up to next power of two

examples:
  2 unique voxels     -> 1 bit
  200 unique voxels   -> 8 bits
  50000 unique voxels -> 16 bits
```

`mat_index_bits` is stored in the .lattice header. The GPU reads it once as a uniform and extracts the index with `child & ((1 << mat_index_bits) - 1)`.

---

## Traversal

### Finding the hit

The ray traverses the 64-tree using DDA stepping. At each node, the occupancy mask tells the shader which children exist. Empty children are skipped immediately. Occupied children either descend (LEAF_FLAG clear) or resolve (LEAF_FLAG set). An ancestor stack caches parent node indices so the ray doesn't re-descend from root on every step.

While descending, the ray accumulates the material base index by summing `voxel_count` for preceding siblings. This is maintained on the ancestor stack alongside the node index, so the cost is one addition per descent step.

When LEAF_FLAG is hit, `materials[material_base + leaf_position]` gives the LUT index, and `lut[lut_index]` gives the Voxel.

### Traversal optimizations

**Fractional coordinate encoding.** The tree lives in [1.0, 2.0). IEEE floats in that range have exponent zero, so the mantissa encodes position directly as fixed-point. Extracting the cell index at any level is `(bits(pos) >> scale_exp) & 3` -- two bit operations, no multiply or divide. Descending decrements `scale_exp` by 2.

**Ancestor stack.** Caches parent node indices and material base offsets. When the ray steps to a neighbor, comparing old and new position bits identifies the highest level that changed. The stack has the right values ready. About 2x speedup over root-to-leaf descent.

**2x2x2 sub-block coalescing.** Before stepping, the traversal checks whether the 2x2x2 sub-block containing the current cell is entirely empty in the 64-bit occupancy mask. If so, the step size doubles. The check is one mask and compare on the existing field. Adds about 21% more iterations for free.

**Ray-octant mirroring.** The coordinate system is mirrored to the negative ray octant at traversal start. With all direction components negative, finding the exit face simplifies to finding the minimum of three distances with no sign conditionals. About 10% faster.

---

## Lighting

Lighting is full path tracing. Primary rays are dispatched from the camera. On each hit, the shader reads the Voxel, samples the BRDF (diffuse or specular depending on roughness/metallic flags), and dispatches a secondary ray. Emissive voxels contribute light directly. Shadow rays are not a separate pass -- they emerge naturally from the path tracing loop.

Each voxel face stores a running weighted average of accumulated indirect light:

```
L_new = (1 - alpha) * L_old + alpha * S
```

`S` is the new path traced sample. `alpha` is tuned per surface type: small for diffuse (stable accumulation over many frames) and larger for specular (faster response to view changes). No sample history buffers, no reservoirs -- one color value per face.

Transparent voxels refract the ray using the surface normal. Metallic voxels tint the specular lobe by albedo. Emissive voxels inject radiance into the path without consuming a bounce.

---

## Disk format: PSVDAG

On disk, sub-node references are removed from the children arrays. Nodes are written depth-first. The first time a unique node appears it gets a label. Every subsequent reference to the same node is a caller, a back-reference to that label. LEAF_FLAG entries are written as-is since they're terminal values.

The loader reconstructs explicit sub-node indices by walking the DFS stream. Loading is close to a direct copy.

PSVDAG achieves 2.8-3.8x smaller files than pointer-based SVDAG. For a scene with many repeated geometric structures, the sharing factor is large and PSVDAG captures most of it. Every repeated node appears once in the DFS stream and is referenced cheaply everywhere else.

---

## The .lattice file format

```
Header:
  magic:           [u8; 4]    "LTCE"
  version:         u16
  flags:           u16
  num_levels:      u8
  mat_index_bits:  u8         1, 2, 4, 8, 16, or 32
  world_min:       [i64; 3]   voxel-space coordinates
  world_max:       [i64; 3]
  section_count:   u32
  sections:        [{tag: u32, offset: u64, size: u64}; section_count]
```

Sections are seekable independently via the offset table. Geometry sections are written first (they're small and needed to set up the traversal), then the material array (which is large and streamed in from a temp file during build).

Section tags:

```
0x4C564C**  LVL*    one section per tree level (* = level index, 0 = root)
  node_count:   u32
  occupancy:    [u64; node_count]
  voxel_count:  [u32; node_count]
  children:     [u32; sum of popcount(occupancy[i]) for all nodes]
                encoded in DFS order (PSVDAG: node refs replaced by caller labels)

0x4C555400  LUT     material LUT (deduplicated Voxel entries)
  entry_count:  u32
  entries:      [Voxel; entry_count]   4 bytes each

0x4D415400  MAT     flat material array (Dolonius DFS order)
  entry_count:  u32
  entries:      [u32; entry_count]   LUT indices

0x50414C00  PAL     color palette
  entry_count:  u32
  entries:      [[u8; 3]; entry_count]   linear RGB

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
      mod.rs          # Dag, Level (SoA), MaterialLut
      node.rs         # LEAF_FLAG and child entry helpers
      voxel.rs        # Voxel struct and ColorPalette

    import/
      mod.rs          # importer entry point, VoxelChunk output type
      palette.rs      # k-means++ in OKLab space, texture sampling
      gltf/
        mod.rs        # glTF scene loading, chunk dispatch
        mesh.rs       # mesh data extraction, triangle clipping to chunks
        material.rs   # PBR material -> Voxel mapping
        voxelizer.rs  # SAT intersection test, texture sampling, normal baking

    pack/
      mod.rs          # packing entry point
      sort.rs         # k-way merge of sorted chunk streams (Morton order)
      dag.rs          # bottom-up streaming DAG construction
      materials.rs    # Dolonius material array, streamed to temp file
      serialize.rs    # .lattice file writing, PSVDAG encoding

    load/
      mod.rs          # loader entry point
      header.rs       # .lattice header parsing, section index
      stream.rs       # PSVDAG DFS stream decoding, node index reconstruction
      upload.rs       # CPU -> GPU buffer upload

    render/
      mod.rs          # renderer entry point
      tracer.rs       # render loop, pass orchestration
      camera.rs       # camera state, ray generation
      traverse.rs     # 64-tree traversal, Dolonius material lookup
      gi.rs           # path tracing, per-face accumulation
      debug.rs        # debug overlay passes

  shaders/
    common.wgsl       # shared math, type definitions
    traverse.wgsl     # 64-tree DDA, Dolonius material index accumulation
    primary.wgsl      # primary ray dispatch
    gi.wgsl           # path tracing bounce loop
    accumulate.wgsl   # per-face weighted GI accumulation
    debug.wgsl        # debug overlays

  tools/
    pack.rs           # CLI: glTF scene -> .lattice
    render.rs         # CLI: .lattice -> frames
    inspect.rs        # CLI: print .lattice header and stats
```

---

## Key findings from research

- A 64-tree produces 37% fewer total nodes than an octree on the same scene and traverses faster, especially with the 64-bit occupancy mask enabling sub-block coalescing.
- Geometry-only DAG deduplication with the Dolonius attribute method adds about 1% to DAG size compared to a pure geometry DAG. Per-pointer attribute methods (Dado et al.) add about 2x. For path tracing where traversal dominates, the Dolonius approach is the right call.
- Leaf-level deduplication is where almost all savings happen. At the 4^3 level, 77% of nodes are duplicates. At the 64^3 level, 0.1%.
- The ancestor stack is the highest-impact traversal optimization (~2x). Sub-block coalescing adds ~21%, ray-octant mirroring adds ~10%.
- PSVDAG-style encoding achieves 2.8-3.8x smaller files than pointer-based SVDAG.

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

#### Surveys

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