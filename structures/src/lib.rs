use bincode::{Decode, Encode};
use std::collections::HashMap;

#[derive(Encode, Decode, PartialEq, Debug)]
pub struct PixelPlacement {
    pub x: u16,
    pub y: u16,
    pub seconds_since_epoch: u32,
    pub color_index: u8,
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub struct Meta {
    pub width: u16,
    pub height: u16,
    pub num_of_pixel_placements: u32,
    pub last_pixel_placed_at_seconds_since_epoch: u32,
    pub colors: HashMap<String, u16>,
}
