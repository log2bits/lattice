# Lattice

A general-purpose voxel engine with full path tracing as the primary rendering method. Lattice takes voxel data from any source, analyzes it, compresses it into a custom on-disk format, and renders it via GPU path tracing through a 64-tree hierarchy.

It works great for Minecraft-style worlds, but it's not limited to them.

---

## Overview

The pipeline has five stages, each handled by a separate crate:

```
[lattice-import]   external source -> unified internal voxel representation
[lattice-analyze]  voxel data -> compression statistics and strategy recommendation
[lattice-pack]     voxel data + strategy -> compressed .lattice file on disk
[lattice-load]     .lattice file -> runtime working set, streamed with LODs
[lattice-render]   runtime voxel data -> GPU path tracing
```

Each stage can run independently. The handoff between them is a well-defined data structure, so you can slot in a different importer, swap the compressor, or test the renderer against hand-crafted data.

---

## Design choices that are fixed

These aren't up for debate on a per-scene or per-platform basis:

- The runtime tree structure is always a **64-tree**. No octree mode.
- The renderer is always **full path tracing**. No rasterization fallback, no hybrid primary visibility.
- Temporal GI accumulation uses a running weighted formula. No long sample history buffers.

Everything else, including compression strategy, brick size, DAG usage, and LOD depth, is decided by the analysis stage based on the actual scene.

---

## Why the analysis stage exists

Fixed compression strategies work great for Minecraft because they are designed around Minecraft's specific structure, but I want something more general.

Before compressing anything, `lattice-analyze` runs a full pass over the voxel data and measures things like:

- how many unique materials appear at each level
- how often material counts fit under useful thresholds (16, 32, 64, 256 materials)
- how many tree levels are appropriate for the scene's resolution and structure

For a voxelized Minecraft world, it might find that voxel materials stay under 256 unique values. From that, the compressor knows to use u8 palette indices at the leaf level. For a different kind of scene, the answer might be completely different.

The point is that the compression strategy comes from what the scene actually looks like, not from hardcoded assumptions.

---

## Geometry and material as separate DAGs

Geometry and material are stored in two completely independent DAGs. They compress independently, which is the whole point.

The geometry DAG is `Dag<()>`. It stores only occupancy. Its leaf pool has exactly one entry, solid. Every solid voxel in the world points to `LEAF_FLAG | 0`. Because material is stripped out entirely, identical geometry patterns deduplicate regardless of what material they contain. Two rooms with the same shape but different block types share geometry nodes. Two walls with the same structure but different textures share geometry nodes. The dedup potential is dramatically higher than a combined geometry+material tree.

The material DAG is `Dag<Material>`. Air is material 0. Large uniform regions collapse to a LEAF_FLAG at a higher level with one material entry. The leaf pool has one entry per unique material in the scene.

Both use the same `Dag<T>` code. Any improvement to the core structure applies to both automatically.

### Leaves at any level

A leaf can appear at any level, not just the bottom. A LEAF_FLAG child at level 1 means every voxel in that entire subtree has the same data as that one leaf. This is how uniform regions are represented without any special sentinel values or inline encoding.

In the children array, the high bit of each `u32` entry distinguishes interior nodes from leaves:

- Bit 31 = 0: interior node, lower 31 bits are an index into the next level's node pool
- Bit 31 = 1: leaf, lower 31 bits are an index into the leaf pool

For geometry, every leaf is `()`, so the leaf pool has one entry and every solid voxel points to `LEAF_FLAG | 0`. For material, each unique material is one leaf entry. A `LEAF_FLAG` at a higher level in the material DAG means every voxel in that subtree has that material.

### Zero-cost LOD on the material DAG

Each interior node in the material DAG stores the most common (mode) material across its entire subtree. This is computed bottom-up during construction and stored in the node's otherwise-unused value slot using a SoA layout — the same technique as Voxelis. During rendering, stopping traversal early at any level gives you the representative material for that region without descending further. No separate LOD tree, no update passes, always consistent.

### How traversal works

Geometry and material are not traversed simultaneously. The traversal is sequential:

1. Traverse the geometry DAG to find the hit voxel. This is the expensive step: DDA stepping, ray-AABB tests, the full ancestor stack. It terminates when the ray hits a solid leaf or exits the scene.
2. You now have a 3D position. Traverse the material DAG from the root using that position. This is positional descent: at each level, extract 6 bits from the position coordinates to get the child index, check the occupancy mask, descend. No ray math. It terminates as soon as it hits a `LEAF_FLAG`, which for large uniform material regions is 1-2 levels deep.

The geometry traversal dominates the total cost. The material lookup is cheap because it doesn't involve ray intersection math and terminates early for any region with uniform material.

For distant geometry using LOD, the material traversal stops at a shallower level and reads the pre-computed representative material from that node instead of descending to the leaf.

### Subtree pointer reuse

Inserting a subtree into either DAG returns a `u32` root index. Because dedup is always on, any two subtrees with identical content return the same index. You can store that index and reuse it everywhere the same pattern appears without re-inserting anything.

This is what makes bulk construction efficient. For a Minecraft world:

1. Pre-voxelize each unique block and blockstate into a 16^3 voxel brick once. Insert into both DAGs. Store the resulting `(geo_root, mat_root)` pair in a lookup table keyed by block ID. A typical world has a few thousand unique entries.
2. For each Minecraft chunk, read the block IDs, look up each one in the brick table, and assemble the chunk subtree by combining the pre-built roots. Every stone block in every chunk automatically shares the same geometry and material subtrees.
3. Stream chunk by chunk. Each processed chunk contributes one root index to the world-level spatial map. The entire world never needs to be in memory at once.

The actual work is proportional to the number of unique configurations, not total voxel count. A world with a trillion voxels but only a few thousand unique block types builds the leaf level once and assembles everything else from pointers.

### What the analyzer decides

Deduplication is always on at every level — the analyzer doesn't decide that. What it does decide:

For the material tree, it measures the distribution of unique materials at the leaf level. If most voxels fit under 256 unique materials, use u8 palette indices. If they fit in 16, use u4. This palette encoding is the main thing the analyzer tunes.

The number of tree levels is also a variable the analyzer can tune. A world with very coarse structure might benefit from fewer levels. A world with fine-grained variation might want more. The geometry and material trees don't have to have the same depth, though in practice they usually will.

---

## The entire world is one DAG

There are no separate per-chunk DAGs. The geometry DAG and color DAG each span the entire world. A spatial region (what you might call a "chunk") is just a `u32` root index into the top level's node pool. The pools themselves are world-scale flat arrays.

This is a bad idea with octrees. An octree node covers a 2^3 region, so a world-scale octree has an enormous number of nodes, and the overhead of one global dedup table across all of them outweighs the benefit. With a 64-tree, each node covers a 4^3 region, you have 37% fewer total nodes, and each node amortizes its overhead over 64 children instead of 8. The dedup is also more effective because a single 64-tree node captures a much larger spatial footprint than an octree node at the same depth.

The practical effect for a Minecraft world: a stone block pattern that appears in every chunk is stored exactly once in the world-level node pool. The dedup table catches it on the first insert; every subsequent region with the same content gets the same index back. That's true at every level — individual voxel materials, 4^3 voxel regions, 16^3 regions, all the way up.

The one constraint this introduces is streaming. If two spatial regions share a node and one region is "unloaded," you can't free that node without checking whether anything else still references it. For offline rendering this doesn't matter — you load what you need and keep it. For real-time streaming it requires reference counting or a different eviction strategy. That's a problem for `lattice-load` to handle, not `lattice-dag`. The DAG itself is just a flat pool.

---

## The .lattice file format

The file format is self-describing. The header records exactly how the data was compressed, so the runtime doesn't need to guess.

The header stores things like:

- magic number and version
- tree type (always 64-tree for now)
- world bounds and voxel resolution
- number of tree levels
- LOD encoding info
- spatial region root index table (region position -> root index in the top-level node pool)
- byte offsets and sizes for each section (per-level node pools, children arrays, leaf pool)

This means two .lattice files can be compressed completely differently and the runtime will handle both correctly. It also means the format can evolve without breaking old files.

---

## Disk format vs runtime format

These are separate things with different goals.

The disk format is optimized for compression and storage. It uses the PSVDAG-style pointerless linearization plus palette compression at the leaves. You want the file to be small.

The runtime format is optimized for traversal speed and cache locality. The loader reads the disk format and reconstructs the indexed `Dag<T>` structure, then uploads the flat arrays to VRAM. The layouts serve different goals and are genuinely different things.

Don't assume that what's good on disk is good at runtime.

### Disk encoding: PSVDAG-style linearization

The disk format uses a pointerless depth-first linearization inspired by PSVDAG (Vokorokos et al. 2020). Instead of storing explicit child indices in the file, nodes are written out in depth-first order. Shared subtrees (DAG nodes referenced more than once) get a short label the first time they appear, and a caller (a back-reference to that label) every subsequent time. Both labels and callers use variable-length encoding, with shorter codes assigned to more frequently referenced nodes (frequency-based compaction).

This removes all pointers from the file entirely. Compared to a pointer-based SVDAG, PSVDAG achieves 2.8-3.8x smaller file sizes. Compared to a plain pointerless SVO, it's usually smaller too once the scene has meaningful repetition.

The tradeoff is that the file can't be traversed directly. Random reads are impossible — to find any node you'd have to scan linearly from the start of the stream.

**PSVDAG is a file format, not an in-memory struct.** There is no `PackedDag` type anywhere in the codebase. `lattice-pack` does a depth-first traversal of the `RuntimeDag<T>` it just built, and streams the PSVDAG encoding directly to disk as it goes. The full pointerless structure only ever exists as bytes in the `.lattice` file. `lattice-load` reads that byte stream and reconstructs a `RuntimeDag<T>` in CPU memory, wiring up all the indices from the label/caller relationships.

### CPU and VRAM use the same layout

The `RuntimeDag<T>` that lives in CPU memory and the data uploaded to VRAM are the same structure. Each tree level is a flat array of interior nodes plus a flat packed children array. The leaf level is a flat array of `T`. Uploading to VRAM is just copying those flat arrays into GPU storage buffers. No transformation, no repacking.

```
lattice-pack    walks RuntimeDag<T> depth-first, streams PSVDAG bytes to disk

lattice-load    reads PSVDAG bytes, reconstructs RuntimeDag<T> in CPU memory,
                uploads flat arrays to GPU storage buffers

lattice-render  shader accesses GPU buffers by index — same layout as CPU
```

If cache optimization ever requires a different node ordering in VRAM (for example, sorting nodes by traversal frequency), `lattice-load` can repack during the upload step. But the logical structure is identical.

---

## Temporal GI accumulation

Visible face lighting is stored per-face as a current color value. When a new sample arrives, it updates using:

```
L_new = (1 - alpha) * L_old + alpha * S
```

where `L_old` is the stored value, `S` is the new sample, and `alpha` depends on the surface. More diffuse surfaces use a smaller alpha for stable accumulation. More view-dependent surfaces use a larger alpha to respond faster.

No sample history. No reservoir buffers. Just one color per face and a weight.

---

## Modules

### lattice-import

Converts external formats into the unified internal voxel representation. Every importer produces the same output regardless of the source.

Planned importers:

- **Minecraft world importer.** Reads `.mca` region files, decodes NBT, maps block state strings to numeric IDs, assembles 64^3 chunks.
- **General 3D model importer.** Takes triangle meshes or other standard 3D formats and voxelizes them.
- **Other voxel format importers.** Common voxel file types where possible.

The internal voxel representation is what `lattice-analyze` and `lattice-pack` consume. None of those stages should know or care which importer produced it.

### lattice-analyze

Runs offline over a complete voxel dataset and produces a compression strategy recommendation. This stage does no compression itself. It only measures.

Output is a structured analysis report that `lattice-pack` reads to decide what to do.

### lattice-pack

Takes the voxel data and the analysis report and builds a `RuntimeDag<T>` in CPU memory, then streams it to disk as a PSVDAG-encoded `.lattice` file. The compression choices come from the report, not from hardcoded defaults.

There is no intermediate `PackedDag` struct. Packing is a depth-first traversal of the `RuntimeDag<T>` that writes label/caller-encoded bytes directly to disk as each node is visited. The PSVDAG structure only ever exists on disk.

This runs offline and can be as slow as it needs to be. It's where all the heavy CPU work happens.

### lattice-load

Reads `.lattice` files and reconstructs a `RuntimeDag<T>` in CPU memory by following the PSVDAG label/caller stream and wiring up node indices. Then uploads the flat node and leaf arrays to GPU storage buffers. Handles streaming and LODs.

- Nearby regions load at full detail.
- Mid-distance regions load at reduced detail.
- Far regions load as coarse representations only.

The loader decides how much of the hierarchy to bring in based on camera distance. It doesn't just mirror the disk structure into memory. A good rule of thumb is that if a single voxel face is far enough that it is smaller than a single pixel on screen, you switch to using the coarser LOD instead.

### lattice-render

GPU path tracer built around 64-tree traversal. Primary visibility, indirect lighting, emissives, reflections, refraction, all through the same traversal path.

No rasterization. No hybrid primary visibility. Rays go through the hierarchy directly.

The traversal shader uses several optimizations that compound on each other:

**Fractional coordinate encoding.** The tree lives in the coordinate range [1.0, 2.0). This isn't arbitrary. IEEE floats in that range have exponent zero, so the mantissa encodes position directly as a fixed-point value. Extracting the cell index at any tree level is just `(bits(pos) >> scale_exp) & 3` -- two bit operations, no multiply or divide. Descending a level decrements `scale_exp` by 2. The full traversal loop runs in float space and never needs to convert to integers for addressing.

**Ancestor stack.** Rather than re-descending from the root every iteration, the traversal maintains a small stack of ancestor node indices. When the ray steps to a neighboring cell, it checks whether that neighbor is a sibling (same parent), or whether it crosses a node boundary and needs to ascend. The common ancestor is identified by comparing the bits of the old and new positions: the highest bit that changed tells you which level the boundary is at, and the stack has the right node index ready. This alone is about a 2x speedup over root-to-leaf descent every step.

**2x2x2 sub-block coalescing.** Before stepping, the traversal checks whether the 2x2x2 sub-block containing the current cell is entirely empty in the 64-bit child mask. If it is, the step size doubles for that iteration. The check is just a mask and compare on the existing 64-bit field, and it gets ~21% more iterations for free.

**Ray-octant mirroring.** The coordinate system is mirrored to the negative ray octant at the start of traversal. With all ray direction components negative, finding the exit face of a cell simplifies to finding the minimum of three distances -- no conditionals to select which face based on ray sign. This is about 10% faster and simplifies the intersection code considerably.

---

## How the Minecraft block voxelizer works

This is the algorithm for turning Minecraft block models and textures into 16x16x16 voxel bricks. Every block state gets voxelized once, offline, by reading directly from the Minecraft client JAR. The output is a simple 16^3 grid of voxels per block state, pallete compressed to 256 different materials per block. Each voxel gets it's own material data including color, transparency, emmisivness, and diffuse.

### Block states

Minecraft represents blocks as block states. A block state is a block type plus a set of properties. `oak_stairs[facing=east,half=bottom,shape=straight]` is one block state. `oak_stairs[facing=west,half=top,shape=inner_left]` is a different one. Every unique combination gets its own numeric ID. The voxelizer runs over all of them in parallel, with each worker thread opening the JAR independently and keeping its own texture cache.

### Finding the right model

The JAR has a blockstate JSON for each block. There are two kinds.

**Variant blocks** have a `variants` map. Keys are property filters like `"facing=east,half=bottom"`. You find the entry that matches the most properties and use that model.

**Multipart blocks** (fences, walls, glass panes) have a `multipart` array. The difference here is that *all* matching entries apply, not just the best one. A fence post is one model, each connected arm is a separate model, and you composite them all together. The `when` conditions support pipe-separated alternatives like `"north": "low|tall"` meaning north=low OR north=tall, plus `OR` arrays for more complex cases.

### Resolving the model

Models can have a parent, so you walk the chain until you hit a builtin or run out. Texture references are merged from the whole chain, with child values winning over parent values. Texture names starting with `#` are variables, like `#all` or `#side`. You resolve them by looking them up in the merged texture map repeatedly until none start with `#` anymore.

### Generating quads

Each model is a list of elements. An element is a cuboid defined by `from` and `to` in [0,16]^3 space, with a face on each of its six sides.

There are two model parts to worry about. Volumetric model parts and thin model parts.

For a volumetric shape, you split it into a quad for each face, and shift the face inward toward by 0.5 voxels the center of the volume. You also clip off 0.5 voxels from all edges of each quad. This puts it inside the voxels it belongs to rather than sitting exactly on their boundary, which would cause the SAT test to miss it.

Zero-thickness quads, like cross models for flowers or chains, get nudged 0.5 units instead, for the same reason. If the thin quad is at the edge of the block, you shift in inwards by 0.5 voxels. If the thin quad is inside the block, you shift it by 0.5 units away from the center of the block.

Then element rotation is applied if the element JSON has a `rotation` block, followed by blockstate variant rotation: Y axis first, then X, both around the block center (8,8,8).

UV conventions follow Blockbench's `CubeFace.UVToLocal()` exactly, which is the reference for how Minecraft renders models.

### SAT intersection test

For each quad, the voxelizer iterates over every voxel in the quad's bounding box and runs a Separating Axis Theorem test to see if they actually intersect. The test checks: the three world axes (X, Y, Z), the quad's own normal, and the 12 cross products of the four quad edges with the three world axes. If any axis separates the quad from the voxel AABB, they don't intersect.

This is a strict test and it needs to be. For something like a chain model or a cross-shaped plant, the bounding box is much bigger than the actual geometry. Without SAT, you'd get a lot of solid voxels that shouldn't be there.

### Texture sampling

For voxels that pass SAT, the voxelizer samples the texture at the voxel center. To do that it undoes the blockstate rotation, undoes any element rotation, then computes the UV at that local position. UV rotation from the face JSON is applied too.

There are special cases for water, foliage, grass, and leaves among other blocks that need to be tinted based on the biome. This is done identically to how minecraft hardcodes the tining, and assumes 15x15 biome blend is turned on.

### Special cases

Air returns an empty grid immediately.

Fluids (water and lava) have no usable model in the JAR, so they're generated procedurally. We need to do the same thing.

Waterlogged blocks get voxelized normally first, then every empty voxel gets filled with water.

Animated textures are stored as vertically stacked frames in the PNG (height > width). Only the first frame is used.

Block material data is hardcoded. Certain blocks are hardcoded to have certain roughness material values or emissive material values. For blocks that need a mirror surface, like glass, the whole block and all voxels in it (except for when waterlogged) are encoded to have that hardcoded roughness value. For blocks that need to be emissive, and emissive value is picked, and a brightness threshold. Only voxels in the block that are brighter than the threshold are set to be emissive.

---

## Suggested project structure

This is a guideline, not a specification. The actual layout will evolve as the project grows.

```
lattice/
  Cargo.toml                     # workspace root

  crates/
    lattice-dag/
      Cargo.toml
      src/
        lib.rs                   # Dag<T>, Level, LeafPool — world-scale arena
        node.rs                  # node layout: interior, leaf, uniform
        intern.rs                # hash-table node interning, global dedup tables
        palette.rs               # palette encoding (u4/u8 indices, uniform flag)

    lattice-import/
      Cargo.toml
      src/
        lib.rs                   # shared VoxelBrick type, importer trait
        minecraft/
          mod.rs                 # minecraft importer entry point
          region.rs              # .mca parsing, sector table, decompression
          chunk.rs               # NBT decoding, block section extraction
          block_states.rs        # block state string -> numeric ID mapping
          bake/
            mod.rs               # block model voxelizer entry point
            jar.rs               # client JAR reader with decompression cache
            blockstate.rs        # blockstate JSON parsing, variant/multipart resolution
            model.rs             # model JSON parsing, parent chain resolution, quad gen
            texture.rs           # PNG loading, animated texture handling, tint
            voxelizer.rs         # SAT intersection, texture sampling, fluid generation
          legacy.rs              # pre-1.13 numeric ID + metadata (TODO)
        mesh/
          mod.rs                 # triangle mesh voxelizer entry point
          voxelize.rs            # SAT-based mesh-to-voxel conversion
        voxel/
          mod.rs                 # other voxel format converters

    lattice-analyze/
      Cargo.toml
      src/
        lib.rs                   # analysis entry point, AnalysisReport type
        repetition.rs            # unique node counts per level, tree depth tuning
        palette.rs               # palette size distributions per level
        bricks.rs                # brick size / reuse tradeoff measurements
        strategy.rs              # translates measurements into a TreeConfig per channel

    lattice-pack/
      Cargo.toml
      src/
        lib.rs                   # packing entry point, reads AnalysisReport
        geometry.rs              # builds geometry DAG from VoxelBrick occupancy
        color.rs                 # builds color DAG from VoxelBrick material data
        lod.rs                   # LOD representation generation
        serialize.rs             # .lattice file writing, header construction

    lattice-load/
      Cargo.toml
      src/
        lib.rs                   # loader entry point
        header.rs                # .lattice header parsing, TreeConfig reconstruction
        stream.rs                # streaming and demand-load logic
        lod.rs                   # LOD selection based on camera distance
        upload.rs                # CPU -> GPU buffer upload for both DAGs
        buffers.rs               # GPU buffer layout for geometry and material channels

    lattice-render/
      Cargo.toml
      src/
        lib.rs                   # renderer entry point
        tracer.rs                # render loop, pass orchestration
        camera.rs                # camera state, ray generation
        traverse.rs              # geometry DAG traversal, positional material lookup
        gi.rs                    # path traced indirect lighting, per-face accumulation
        lod.rs                   # LOD-aware traversal decisions
        debug.rs                 # debug overlay passes

  shaders/
    common.wgsl                  # shared math, type definitions
    traverse.wgsl                # geometry DAG ray traversal, positional material lookup
    primary.wgsl                 # primary ray dispatch
    gi.wgsl                      # indirect lighting passes
    accumulate.wgsl              # per-face GI accumulation, weighted update
    debug.wgsl                   # debug overlays

  tools/
    Cargo.toml
    src/
      pack.rs                    # CLI: source -> .lattice
      render.rs                  # CLI: .lattice -> frames (native)
      inspect.rs                 # CLI: print .lattice header and stats
```

---

## Key findings from research:
- A 64-tree (otherwise known as a "contree") produces 37% fewer total nodes than an octree on Minecraft terrain. It's also faster to ray traverse through, especially when storing a coarse 8 bit occupancy mask along with the 64 bit mask.
- Leaf-level deduplication is where almost all the savings happen. At the 4^3 leaf level, 77% of nodes are duplicates. At the 64^3 root level, it's 0.1%.
- Culling (removing voxels surrounded on all 6 sides by opaque blocks) eliminates 88% of solid voxels before any other compression runs. This has a bigger impact than any encoding choice.
- Stacked palettes (chunk-level, region-level, tile-level) compress attribute data well. The biggest win is the inline tile palette at 4^3 scope. A 1024^3 chunk palette for pointers worked well for minecraft worlds, since each 64^3 chunk of minecraft blocks usually contains only around 100 unique blocks, which is 100 unique leaf brick pointers.
- Geometry and material are stored in two completely separate DAGs. Stripping material from the geometry DAG dramatically increases dedup — identical shapes with different materials share geometry nodes. After geometry traversal finds the hit point, the material DAG is traversed by 3D position: cheap positional descent, no ray math, terminates early on LEAF_FLAG for uniform regions. Interior nodes in the material DAG store a representative (mode) material for free via SoA layout, enabling zero-cost LOD at any traversal depth.
- PSVDAG-style depth-first linearization removes all pointers from the disk format, achieving 2.8-3.8x compression over pointer-based SVDAG. The loader reconstructs pointer indices for runtime use. This is the right split: compact on disk, fast at runtime.
- The ancestor stack is the single highest-impact traversal optimization. Benchmarks on a 4K Bistro scene show 16903 cycles/ray without it, 8896 with it (~2x). The 2x2x2 sub-block coalescing adds another ~21%, and ray-octant mirroring adds ~10% on top of that. These stack and should all be in from the start.
- PSVDAG compresses pointer-based SVDAG by 2.8-3.8x. For Lattice this applies to the disk format. The runtime format keeps indices for fast GPU access.

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


Most of the time, the scene isnt going to be uniform material, but instead will be a uniform complex material tiled across the entire area. For example, underground there's lots of stone blocks. Each stone block has a bunch of colors in it, but across a big underground patch, there's certainly many different colors, but if we store the material in some sort of pointer type thing, we can have each stone block be a material pointer, and then the entire underground patch is also a pointer and