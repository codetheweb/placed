use colors_transform::Color;
use image::{Rgb, RgbImage};
use mla::{config::ArchiveReaderConfig, ArchiveReader};
use rmp_serde::Deserializer;
use serde::de::Deserialize;
use std::fs::File;
use std::io::{BufReader, Read};
use structures::{ColorMap, Meta, PixelPlacement};

pub struct PlacedArchive {
    pub meta: Meta,
    colors_vec: Vec<Rgb<u8>>,
    archive_path: String,
}

impl PlacedArchive {
    pub fn load(archive_path: String) -> Result<PlacedArchive, std::io::Error> {
        let file = match File::open(archive_path.clone()) {
            Ok(file) => file,
            Err(err) => return Err(err),
        };

        let mut archive = ArchiveReader::from_config(file, ArchiveReaderConfig::new()).unwrap();

        let mut colors: Vec<Rgb<u8>>;
        {
            let colors_archive = archive
                .get_file("colors".to_string())
                .unwrap()
                .expect("Could not find colors file");

            let mut color_map_deserializer = Deserializer::new(colors_archive.data);

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

        let parsed_meta: Meta;
        {
            let meta_archive = archive
                .get_file("meta".to_string())
                .unwrap()
                .expect("Could not find meta file");

            let mut meta_deserializer = Deserializer::new(meta_archive.data);
            parsed_meta = Deserialize::deserialize(&mut meta_deserializer)
                .expect("Could not deserialize meta");
        }

        Ok(PlacedArchive {
            meta: parsed_meta,
            colors_vec: colors,
            archive_path,
        })
    }

    /// Renders the image up to the given number of seconds.
    /// If seconds is 0, renders the entire image.
    pub fn render_up_to(&mut self, seconds: u32) -> RgbImage {
        let mut canvas = RgbImage::new(self.meta.width.into(), self.meta.height.into());

        self.process_data(|data| {
            let mut buffer = [0u8; std::mem::size_of::<alkahest::Packed<PixelPlacement>>()];

            while data.read_exact(&mut buffer).is_ok() {
                let pixel = alkahest::read::<PixelPlacement>(&buffer);

                if pixel.seconds_since_epoch > seconds && seconds != 0 {
                    break;
                }

                canvas.put_pixel(
                    pixel.x as u32,
                    pixel.y as u32,
                    self.colors_vec[pixel.color_index as usize],
                );
            }
        });

        canvas
    }

    fn process_data<C>(&self, process_reader: C)
    where
        C: FnOnce(&mut dyn Read),
    {
        let file = match File::open(&self.archive_path) {
            Ok(file) => file,
            Err(err) => panic!("Could not open archive: {}", err),
        };

        let buffered_file = BufReader::new(file);

        let mut archive =
            ArchiveReader::from_config(buffered_file, ArchiveReaderConfig::default()).unwrap();

        let mut data_file = archive
            .get_file("data".to_string())
            .unwrap()
            .expect("Could not find data file");

        process_reader(&mut data_file.data)
    }
}
