use bincode::{Decode, Encode};
use std::collections::HashMap;

#[derive(Encode, Decode, PartialEq, Eq, Debug)]
pub struct TilePlacement {
    pub x: u16,
    pub y: u16,
    pub ms_since_epoch: u32,
    pub color_index: u8,
}

#[derive(Encode, Decode, PartialEq, Eq, Debug, Clone)]
pub struct CanvasSizeChange {
    pub width: u16,
    pub height: u16,
    pub ms_since_epoch: u32,
}

#[derive(Encode, Decode, PartialEq, Eq, Debug, Clone)]
pub struct ChunkDescription {
    pub id: u32,
    pub up_to_ms_since_epoch: u32,
    pub num_tiles: u32,
}

#[derive(Encode, Decode, PartialEq, Eq, Debug, Clone)]
pub struct Meta {
    pub canvas_size_changes: Vec<CanvasSizeChange>,
    pub last_pixel_placed_at_seconds_since_epoch: u32,
    pub color_id_to_tuple: HashMap<u8, [u8; 4]>,
    pub chunk_descs: Vec<ChunkDescription>,
}

impl Meta {
    pub fn get_largest_canvas_size(&self) -> Option<CanvasSizeChange> {
        Some(
            self.canvas_size_changes
                .iter()
                .max_by_key(|x| x.width * x.height)?
                .clone(),
        )
    }
}
