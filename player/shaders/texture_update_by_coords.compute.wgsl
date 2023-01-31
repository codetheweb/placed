let SIZE_OF_COORDINATE_UPDATE = 9u;

struct CoordinateUpdate {
  data: array<u32, SIZE_OF_COORDINATE_UPDATE>
};

// todo: use https://docs.rs/crevice/latest/crevice/ for more ergonomic struct?
struct Locals {
  color_map: array<vec4<u32>, 256>,
  width: u32,
  height: u32,
};

@group(0) @binding(0) var<storage, read> tile_updates : array<CoordinateUpdate>;
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
// todo: what's the optimal workgroup size?
// I think it's 1?
@workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= arrayLength(&tile_updates)) {
    return;
  }

  if (id.y > 3u) {
    return;
  }

  var current_offset = id.y * SIZE_OF_COORDINATE_UPDATE;

  let x = readU16(id.x, current_offset);
  current_offset += 2u;
  let y = readU16(id.x, current_offset);
  current_offset += 2u;
  let color_index = readU8(id.x, current_offset);
  current_offset += 1u;
  let ms_since_epoch = readU32(id.x, current_offset);
  current_offset += 4u;

  let color = r_locals.color_map[color_index];

  let previous_timestamp_value = atomicMax(&last_timestamp_for_tile[x + y * r_locals.width], ms_since_epoch);
  if (previous_timestamp_value > ms_since_epoch) {
    return;
  }

  textureStore(
    texture_out,
    vec2<i32>(i32(x), i32(y)),
    vec4<f32>(f32(color.x) / 255.0, f32(color.y) / 255.0, f32(color.z) / 255.0, f32(color.w) / 255.0)
  );
}
