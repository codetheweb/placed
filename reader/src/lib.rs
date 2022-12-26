use colors_transform::Color;
use image::{Rgb, RgbImage};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use structures::{Meta, PixelPlacement};

pub struct PlacedArchive {
    pub meta: Meta,
    colors: Vec<Rgb<u8>>,
    archive_path: String,
}

impl PlacedArchive {
    pub fn load(archive_path: String) -> Result<PlacedArchive, std::io::Error> {
        let mut file = match File::open(archive_path.clone()) {
            Ok(file) => file,
            Err(err) => return Err(err),
        };

        let meta: Meta = bincode::decode_from_std_read(&mut file, bincode::config::standard())
            .expect("Could not deserialize meta");

        // Vec lookup by index is slightly faster than HashMap lookup by key
        let mut colors: Vec<Rgb<u8>> = Vec::new();
        {
            colors.resize(256, Rgb([0, 0, 0]));
            for (color_str, color_id) in meta.colors.clone() {
                let color =
                    colors_transform::Rgb::from_hex_str(&color_str).expect("Could not parse color");

                colors[color_id as usize] = Rgb([
                    color.get_red() as u8,
                    color.get_green() as u8,
                    color.get_blue() as u8,
                ]);
            }
        }

        Ok(PlacedArchive {
            meta,
            colors,
            archive_path,
        })
    }

    /// Renders the image up to the given number of seconds.
    /// If seconds is 0, renders the entire image.
    pub fn render_up_to(&mut self, seconds: u32) -> RgbImage {
        let mut canvas = RgbImage::new(self.meta.width.into(), self.meta.height.into());

        self.process_data(|data| {
            while let Ok(pixel) = bincode::decode_from_std_read::<
                PixelPlacement,
                bincode::config::Configuration,
                Box<&mut dyn Read>,
            >(&mut Box::new(data), bincode::config::standard())
            {
                if pixel.seconds_since_epoch > seconds && seconds != 0 {
                    break;
                }

                canvas.put_pixel(
                    pixel.x as u32,
                    pixel.y as u32,
                    self.colors[pixel.color_index as usize],
                );
            }
        });

        canvas
    }

    fn process_data<C>(&self, process_reader: C)
    where
        C: FnOnce(&mut dyn Read),
    {
        let mut file = match File::open(&self.archive_path) {
            Ok(file) => file,
            Err(err) => panic!("Could not open archive: {}", err),
        };

        // Skip metadata
        let _: Meta =
            bincode::decode_from_std_read(&mut file, bincode::config::standard()).unwrap();

        let mut buffered_data = BufReader::new(&mut file);

        process_reader(&mut buffered_data)
    }
}
