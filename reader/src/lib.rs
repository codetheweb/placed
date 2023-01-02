use colors_transform::Color;
use image::{Rgb, RgbImage};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use structures::{Meta, PixelPlacement};

struct LastRenderedCanvas {
    canvas: RgbImage,
    rendered_up_to_seconds: u32,
    rendered_up_to_offset: u64,
}

pub struct PlacedArchive {
    pub meta: Meta,
    colors: Vec<Rgb<u8>>,
    archive_path: String,
    last_rendered_canvas: Option<LastRenderedCanvas>,
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
            last_rendered_canvas: None,
        })
    }

    /// Renders the image up to the given number of seconds.
    /// If seconds is 0, renders the entire image.
    pub fn render_up_to(&mut self, seconds: u32) -> RgbImage {
        let mut canvas: RgbImage;

        if let Some(last_rendered_canvas) = &self.last_rendered_canvas {
            canvas = last_rendered_canvas.canvas.clone();
        } else {
            canvas = RgbImage::new(self.meta.width.into(), self.meta.height.into());
            canvas.fill(0xff);
        }

        let mut rendered_up_to_offset = 0;
        self.process_pixel_data(|data| {
            if let Some(last_rendered_canvas) = &self.last_rendered_canvas {
                if last_rendered_canvas.rendered_up_to_seconds < seconds {
                    data.seek(std::io::SeekFrom::Start(
                        last_rendered_canvas.rendered_up_to_offset,
                    ))
                    .unwrap();
                }
            }

            while let Ok(pixel) = bincode::decode_from_std_read::<
                PixelPlacement,
                bincode::config::Configuration,
                BufReader<&mut File>,
            >(data, bincode::config::standard())
            {
                if (pixel.ms_since_epoch / 1000) > seconds && seconds != 0 {
                    break;
                }

                canvas.put_pixel(
                    pixel.x as u32,
                    pixel.y as u32,
                    self.colors[pixel.color_index as usize],
                );
            }

            rendered_up_to_offset = data.seek(std::io::SeekFrom::Current(0)).unwrap();
        });

        self.last_rendered_canvas = Some(LastRenderedCanvas {
            canvas: canvas.clone(),
            rendered_up_to_seconds: seconds,
            rendered_up_to_offset,
        });

        canvas
    }

    pub fn process_pixel_data<C>(&self, process_reader: C)
    where
        C: FnOnce(&mut BufReader<&mut File>),
    {
        let mut file = match File::open(&self.archive_path) {
            Ok(file) => file,
            Err(err) => panic!("Could not open archive: {}", err),
        };

        PlacedArchive::seek_to_pixel_data(&mut file);

        let mut buffered_data = BufReader::new(&mut file);

        process_reader(&mut buffered_data)
    }

    pub fn seek_to_pixel_data<R: Read + Seek>(r: &mut R) {
        r.seek(SeekFrom::Start(0)).unwrap();

        let meta: Meta = bincode::decode_from_std_read(r, bincode::config::standard()).unwrap();

        let meta_end_offfset = r.seek(SeekFrom::Current(0)).unwrap();

        if let Some(last_snapshot) = meta.snapshots.last() {
            r.seek(std::io::SeekFrom::Start(
                last_snapshot.start_offset + last_snapshot.length + meta_end_offfset,
            ))
            .unwrap();
        }
    }
}
