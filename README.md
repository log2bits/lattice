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

- how many unique node geometries exist at each tree level
- how many leaf bricks repeat exactly (including material data, not just occupancy)
- how many unique materials appear at each level
- how often palettes fit under useful thresholds (16, 32, 64, 256 materials)
- how effective DAG deduplication would be at each level

For a voxelized Minecraft world, it might find that 16^3 bricks repeat heavily and stay under 256 materials. From that, the compressor knows to use palette compression at the brick level and DAG deduplication for those bricks. For a different kind of scene, the answer might be completely different.

The point is that the compression strategy comes from what the scene actually looks like, not from hardcoded assumptions.

---

## The .lattice file format

The file format is self-describing. The header records exactly how the data was compressed, so the runtime doesn't need to guess.

The header stores things like:

- magic number and version
- tree type (always 64-tree for now)
- chunk dimensions and brick size
- number of tree layers per chunk
- which levels use palette compression
- which levels use DAG deduplication
- whether leaf deduplication includes material payloads or just occupancy
- LOD encoding info
- byte offsets and sizes for each section

This means two .lattice files can be compressed completely differently and the runtime will handle both correctly. It also means the format can evolve without breaking old files.

---

## Disk format vs runtime format

These are separate things with different goals.

The disk format is optimized for compression and storage. It uses DAG deduplication, palette compression, and whatever else `lattice-analyze` recommended. You want the file to be small.

The runtime format is optimized for traversal speed and cache locality. The loader reads the disk format and constructs a working set that's good for the GPU to traverse, which isn't always the same thing as what's good for storage.

For example, DAG deduplication is preserved in VRAM at the leaf level *only* if the analysis says it helps runtime behavior. If the scene has heavy repetition in leaf bricks and good cache reuse, keep the deduplication. If it doesn't, expand those leaves into a flat structure that's faster to traverse even though it uses more memory.

Don't assume that what's good on disk is good at runtime.

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

Takes the voxel data and the analysis report and writes the compressed `.lattice` file. The compression choices come from the report, not from hardcoded defaults.

This runs offline and can be as slow as it needs to be. It's where all the heavy CPU work happens.

### lattice-load

Reads `.lattice` files and constructs the runtime working set. Handles streaming and LODs.

- Nearby regions load at full detail.
- Mid-distance regions load at reduced detail.
- Far regions load as coarse representations only.

The loader decides how much of the hierarchy to bring in based on camera distance. It doesn't just mirror the disk structure into memory. A good rule of thumb, is that if a single voxel face is far enough that it is smaller than a single pixel on screen, you switch to using the coarser LOD instead.

### lattice-render

GPU path tracer built around 64-tree traversal. Primary visibility, indirect lighting, emissives, reflections, refraction, all through the same traversal path.

No rasterization. No hybrid primary visibility. Rays go through the hierarchy directly.

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
    lattice-import/
      Cargo.toml
      src/
        lib.rs                   # unified voxel representation, shared types
        minecraft/
          mod.rs                 # minecraft importer entry point
          region.rs              # .mca parsing, sector table, decompression
          chunk.rs               # NBT decoding, block section extraction
          block_states.rs        # block state string -> numeric ID mapping
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
        repetition.rs            # DAG dedup estimates at each tree level
        materials.rs             # palette size distributions, material counts
        bricks.rs                # brick size / reuse tradeoff measurements
        strategy.rs              # translates measurements into a CompressionStrategy

    lattice-pack/
      Cargo.toml
      src/
        lib.rs                   # packing entry point, reads AnalysisReport
        tree.rs                  # 64-tree construction from voxel data
        dag.rs                   # DAG deduplication, leaf and interior node interning
        palette.rs               # palette encoding at each level
        lod.rs                   # LOD representation generation
        serialize.rs             # .lattice file writing, header construction

    lattice-load/
      Cargo.toml
      src/
        lib.rs                   # loader entry point
        header.rs                # .lattice header parsing
        stream.rs                # streaming and demand-load logic
        lod.rs                   # LOD selection based on camera distance
        upload.rs                # CPU -> GPU buffer upload
        buffers.rs               # GPU buffer layout definitions

    lattice-render/
      Cargo.toml
      src/
        lib.rs                   # renderer entry point
        tracer.rs                # render loop, pass orchestration
        camera.rs                # camera state, ray generation
        traverse.rs              # 64-tree traversal pass
        gi.rs                    # path traced indirect lighting, per-face accumulation
        lod.rs                   # LOD-aware traversal decisions
        debug.rs                 # debug overlay passes

  shaders/
    common.wgsl                  # shared math, type definitions
    traverse.wgsl                # 64-tree traversal
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
- Geometry and attribute encoding can either be done seperately or together. Usually, you want the attributes (material voxel data) to be encoded seperately into a different data structure. Then when you traverse the tree, you count up how many voxels each layer you pass though has (each node in the tree has a voxel_count) and when you finally reach the bottom, you have a unique index for that voxel specificallty to lookup into the seperate material data structure.
