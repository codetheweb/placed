use colors_transform::Color;
use image::ImageFormat;
use image::Rgb;
use image::RgbImage;
use rmp_serde::Deserializer;
use serde::de::Deserialize;
use std::env;
use std::fs;
use std::io::Read;
use structures::{ColorMap, PixelPlacement};

fn main() {
    let args: Vec<_> = env::args().collect();
    let filename = &args[1];
    let file = fs::File::open(filename).expect("Could not open file");

    let buffered = std::io::BufReader::new(file);

    let mut archive = zip::ZipArchive::new(buffered).expect("Could not open zip file");

    let mut colors: Vec<Rgb<u8>>;
    {
        let colors_archive = archive
            .by_name("colors")
            .expect("Could not find colors file");
        let mut color_map_deserializer = Deserializer::new(colors_archive);

        let parsed_color_map: ColorMap = Deserialize::deserialize(&mut color_map_deserializer)
            .expect("Could not deserialize color map");

        // Vec lookup by index is faster than HashMap lookup by key
        colors = Vec::new();
        colors.resize(256, Rgb([0, 0, 0]));
        for (color_str, color_id) in parsed_color_map.colors {
            let color =
                colors_transform::Rgb::from_hex_str(&color_str).expect("Could not parse color");

            colors[color_id as usize] = Rgb([
                color.get_red() as u8,
                color.get_green() as u8,
                color.get_blue() as u8,
            ]);
        }
    }

    let mut data = archive.by_name("data").expect("Could not find data file");

    let mut canvas = RgbImage::new(2000, 2000);

    let mut buffer = [0u8; std::mem::size_of::<alkahest::Packed<PixelPlacement>>()];
    while data.read_exact(&mut buffer).is_ok() {
        let pixel = alkahest::read::<PixelPlacement>(&buffer);

        canvas.put_pixel(
            pixel.x as u32,
            pixel.y as u32,
            colors[pixel.color_index as usize],
        );
    }

    canvas
        .save_with_format("./out.png", ImageFormat::Png)
        .expect("Could not save image");
}
