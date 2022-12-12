// Bug with alkahest?
#![allow(clippy::drop_copy, clippy::drop_non_drop)]

use std::collections::HashMap;

#[derive(alkahest::Schema)]
pub struct PixelPlacement {
    pub x: u16,
    pub y: u16,
    pub seconds_since_epoch: u32,
    pub color_index: u8,
}

#[derive(Debug, PartialEq, Eq, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct ColorMap {
    pub colors: HashMap<String, u16>,
}

#[derive(Debug, PartialEq, Eq, serde_derive::Deserialize, serde_derive::Serialize, Clone)]
pub struct Meta {
    pub width: u16,
    pub height: u16,
    pub num_of_pixel_placements: u32,
    pub last_pixel_placed_at_seconds_since_epoch: u32,
}

#[derive(alkahest::Schema)]
pub struct Snapshot {
    pub pixels: alkahest::Seq<u8>,
    pub num_pixels_in_history: u32,
    pub last_pixel_at_seconds_since_epoch: u32,
}
