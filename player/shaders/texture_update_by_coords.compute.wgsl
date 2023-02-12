const SIZE_OF_COORDINATE_UPDATE_BYTES = 9u;

struct FourTileUpdate {
  data: array<u32, SIZE_OF_COORDINATE_UPDATE_BYTES>
};

struct DecodedTileUpdate {
  x: u32,
  y: u32,
  color_index: u32,
  ms_since_epoch: u32,
};

// todo: use https://docs.rs/crevice/latest/crevice/ for more ergonomic struct?
struct Locals {
  color_map: array<vec4<u32>, 256>,
  width: u32,
  height: u32,
};

@group(0) @binding(0) var<storage, read> tile_updates : array<FourTileUpdate>;
@group(0) @binding(1) var<uniform> r_locals : Locals;
@group(0) @binding(2) var<storage, read_write> last_index_for_tile : array<atomic<u32>, 4000000>;
@group(0) @binding(3) var texture_out : texture_storage_2d<rgba8unorm, write>;

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

fn readTile(four_tile_offset: u32, offset_in_four_tiles: u32) -> DecodedTileUpdate {
  var current_offset = offset_in_four_tiles * SIZE_OF_COORDINATE_UPDATE_BYTES;

  var tile: DecodedTileUpdate;

  tile.x = readU16(four_tile_offset, current_offset);
  current_offset += 2u;
  tile.y = readU16(four_tile_offset, current_offset);
  current_offset += 2u;
  tile.color_index = readU8(four_tile_offset, current_offset);
  current_offset += 1u;
  tile.ms_since_epoch = readU32(four_tile_offset, current_offset);
  current_offset += 4u;

  return tile;
}

fn getTileIndex(tile: DecodedTileUpdate) -> u32 {
  return tile.x + (tile.y * r_locals.height);
}

@compute
@workgroup_size(1)
fn calculate_final_tiles(@builtin(global_invocation_id) id: vec3<u32>) {
  let current_data_index = (id.x * 4u) + id.y;
  if (current_data_index >= arrayLength(&tile_updates)) {
    return;
  }

  let tile = readTile(id.x, id.y);

  if (tile.color_index == 255u) {
    // This update is just padding
    return;
  }

  atomicMax(&last_index_for_tile[getTileIndex(tile)], current_data_index);
}

@compute
@workgroup_size(1)
fn update_texture(@builtin(global_invocation_id) id: vec3<u32>) {
  let current_data_index = (id.x * 4u) + id.y;
  if (current_data_index >= arrayLength(&tile_updates)) {
    return;
  }

  let tile = readTile(id.x, id.y);

  if (tile.color_index == 255u) {
    // This update is just padding
    return;
  }

  let max_data_index_for_tile = atomicLoad(&last_index_for_tile[getTileIndex(tile)]);

  if (current_data_index != max_data_index_for_tile) {
    return;
  }

  let color = r_locals.color_map[tile.color_index];

  textureStore(
    texture_out,
    vec2<i32>(i32(tile.x), i32(tile.y)),
    vec4<f32>(f32(color.x) / 255.0, f32(color.y) / 255.0, f32(color.z) / 255.0, f32(color.w) / 255.0)
  );
}
