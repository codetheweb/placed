use colors_transform::Color;
use image::ImageFormat;
use image::Rgb;
use image::RgbImage;
use rmp_serde::Deserializer;
use serde::de::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use structures::{ColorMap, PixelPlacement};

fn main() {
    let args: Vec<_> = env::args().collect();
    let filename = &args[1];
    let file = fs::File::open(filename).expect("Could not open file");

    let mut archive = zip::ZipArchive::new(file).expect("Could not open zip file");

    let mut color_map: HashMap<u8, Rgb<u8>>;
    {
        let colors_archive = archive
            .by_name("colors")
            .expect("Could not find colors file");
        let mut color_map_deserializer = Deserializer::new(colors_archive);

        let parsed_color_map: ColorMap = Deserialize::deserialize(&mut color_map_deserializer)
            .expect("Could not deserialize color map");

        // Convert to RGB structs
        color_map = HashMap::new();
        for (color_str, color_id) in parsed_color_map.colors {
            let color =
                colors_transform::Rgb::from_hex_str(&color_str).expect("Could not parse color");
            color_map.insert(
                color_id as u8,
                Rgb([
                    color.get_red() as u8,
                    color.get_green() as u8,
                    color.get_blue() as u8,
                ]),
            );
        }
    }

    let data = archive.by_name("data").expect("Could not find data file");
    let mut data_deserializer = Deserializer::new(data);

    let mut canvas = RgbImage::new(2000, 2000);
    let mut i = 0;
    while let Ok(pixel) = PixelPlacement::deserialize(&mut data_deserializer) {
        canvas.put_pixel(
            pixel.x as u32,
            pixel.y as u32,
            *color_map.get(&pixel.color_index).unwrap(),
        );

        i += 1;

        if i % 100000 == 0 {
            println!("Processed {} pixels", i);
        }
    }

    canvas
        .save_with_format("./out.png", ImageFormat::Png)
        .expect("Could not save image");
}
