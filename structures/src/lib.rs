// Bug with alkahest?
#![allow(clippy::drop_copy)]

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
