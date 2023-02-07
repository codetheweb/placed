const SIZE_OF_COORDINATE_UPDATE_BYTES = 9u;
const NUM_COORDINATE_UPDATES_PER_TILE = 4u;

// https://gpuweb.github.io/gpuweb/#dom-supported-limits-maxcomputeworkgroupstoragesize
const MAX_COMPUTE_WORKGROUPS_PER_DIMENSION = 65535u;

struct FourTileUpdate {
  // (u32 = 4 bytes)
  data: array<u32, SIZE_OF_COORDINATE_UPDATE_BYTES>
};

// todo: use https://docs.rs/crevice/latest/crevice/ for more ergonomic struct?
struct Locals {
  color_map: array<vec4<u32>, 256>,
  width: u32,
  height: u32,
};

@group(0) @binding(0) var<storage, read> tile_updates : array<FourTileUpdate>;
@group(0) @binding(1) var<uniform> r_locals : Locals;
@group(0) @binding(2) var texture_out : texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<storage, read_write> last_timestamp_for_tile : array<atomic<u32>>;

fn readU8(i: u32, current_offset: u32) -> u32 {
	var ipos : u32 = current_offset / 4u;
	var val_u32 : u32 = tile_updates[i].data[ipos];
	var shift : u32 = 8u * (current_offset % 4u);
	var val_u8 : u32 = (val_u32 >> shift) & 0xFFu;

	return val_u8;
}

fn readU16(i: u32, current_offset: u32) -> u32 {
  var first = readU8(i, current_offset);
  var second = readU8(i, current_offset + 1u);
  var value = first | (second << 8u);

  return value;
}

fn readU32(i: u32, current_offset: u32) -> u32 {
  var first = readU16(i, current_offset);
  var second = readU16(i, current_offset + 2u);
  var value = first | (second << 16u);

  return value;
}

@compute
@workgroup_size(1)
// id.x: 0-MAX_COMPUTE_WORKGROUPS_PER_DIMENSION
// id.z: 0-MAX_COMPUTE_WORKGROUPS_PER_DIMENSION
// id.x * id.z: index of FourTileUpdate within main tile_updates array
// id.y: 0-4, tile index within the FourTileUpdate
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
  let data_index = id.x;// + (id.z * MAX_COMPUTE_WORKGROUPS_PER_DIMENSION);

  if (data_index >= arrayLength(&tile_updates)) {
    return;
  }

  if (id.y > 3u) {
    return;
  }

  var offset_in_struct = id.y * SIZE_OF_COORDINATE_UPDATE_BYTES;

  let x = readU16(data_index, offset_in_struct);
  offset_in_struct += 2u;
  let y = readU16(data_index, offset_in_struct);
  offset_in_struct += 2u;
  let color_index = readU8(data_index, offset_in_struct);
  offset_in_struct += 1u;
  let ms_since_epoch = readU32(data_index, offset_in_struct);
  offset_in_struct += 4u;

  if (color_index == 255u) {
      textureStore(
      texture_out,
      vec2<i32>(i32(ms_since_epoch), i32(ms_since_epoch)),
      vec4<f32>(1.0, 1.0, 1.0, 1.0)
    );
    // This update is just padding
    return;
  }

  let color = r_locals.color_map[color_index];

  // If we don't put a barrier here, we might not have the latest value for the atomic
  storageBarrier();
  let previous_timestamp_value = atomicMax(&last_timestamp_for_tile[x + y * r_locals.width], ms_since_epoch);
  if (previous_timestamp_value > ms_since_epoch) {
    // textureStore(
    //   texture_out,
    //   vec2<i32>(i32(x), i32(y)),
    //   vec4<f32>(0.0, 0.0, 0.0, 1.0)
    // )
    return;
  }

  textureStore(
    texture_out,
    vec2<i32>(i32(x), i32(y)),
    vec4<f32>(f32(color.x) / 255.0, f32(color.y) / 255.0, f32(color.z) / 255.0, f32(color.w) / 255.0)
  );
}
