# TODO

## Tree core

- [x] `Aabb::contains`, `overlaps`, `split_at_slot`
- [x] `Level::push_node`, `Level::set_node`
- [ ] `Tree::build_from_data`
- [ ] `Tree::compact`
- [ ] `Tree::trace`

## Chunk

- [ ] `MaterialTable::get_or_add`, `mode`
- [ ] `Chunk::new`, `get_voxel`
- [ ] `Chunk::build_from_shapes`, `rebuild_region`
- [ ] `Chunk::flush_edits`

## Shapes

- [ ] `Rect::aabb`, `coverage`
- [ ] `Sphere::aabb`, `coverage`
- [ ] `Terrain::aabb`, `coverage`

## Tree edits

- [ ] `Tree::apply_sorted_edits`

## World

- [ ] `World::new`
- [ ] `World::add_shape_edit`
- [ ] `World::queue_voxel_edit`, `flush_edits`
- [ ] `World::tick_lod`
- [ ] `World::trace_ray`

## Render

- [ ] `RenderPipeline::new`
- [ ] `upload_world_tree`, `upload_chunks`
- [ ] `Renderer::new`, `render`, `resize`
- [ ] Shaders (primary traversal, debug, output)
