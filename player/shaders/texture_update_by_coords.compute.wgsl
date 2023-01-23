// todo: use https://docs.rs/crevice/latest/crevice/ for more ergonomic struct?
struct CoordinateUpdate {
  x: u32,
  y: u32,
  r: u32,
  g: u32,
  b: u32,
};

@group(0) @binding(0) var<storage, read> tile_updates : array<CoordinateUpdate>;
@group(0) @binding(1) var texture_out : texture_storage_2d<rgba8unorm, write>;

@compute
// todo: what's the optimal workgroup size?
// I think it's 1?
@workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= arrayLength(&tile_updates)) {
    return;
  }

  let tile_update = tile_updates[id.x];

  textureStore(
    texture_out,
    vec2<i32>(i32(tile_update.x), i32(tile_update.y)),
    vec4<f32>(f32(tile_update.r) / 255.0, f32(tile_update.g) / 255.0, f32(tile_update.b) / 255.0, 1.0)
  );
}
