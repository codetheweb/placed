use std::collections::HashMap;

#[macro_use]
extern crate serde_derive;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PixelPlacement {
    pub x: u16,
    pub y: u16,
    pub seconds_since_epoch: u32,
    pub color_index: u8,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ColorMap {
    pub colors: HashMap<String, u16>,
}
