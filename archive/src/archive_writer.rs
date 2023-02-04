use std::{
    collections::BTreeMap,
    io::{Read, Seek, SeekFrom, Write},
};

// todo: try different compression (frame grid, encode each second in an array, even if nothing changed)
use chrono::NaiveDateTime;
use image::RgbImage;
use mla::{config::ArchiveWriterConfig, ArchiveWriter};
use tempfile::tempfile;

use crate::{
    constants::BINCODE_CONFIG,
    structures::{CanvasSizeChange, ChunkDescription, Meta, StoredTilePlacement},
};

// todo: make parameter
const NUM_CHUNKS: u32 = 64;

#[derive(Debug, PartialEq, Eq)]
struct IntermediateTilePlacement {
    pub x: u16,
    pub y: u16,
    pub placed_at: NaiveDateTime,
    pub color_index: u8,
}

impl PartialOrd for IntermediateTilePlacement {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.placed_at.cmp(&other.placed_at))
    }
}

impl Ord for IntermediateTilePlacement {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.placed_at.cmp(&other.placed_at)
    }
}

pub struct PlacedArchiveWriter<'a, W: Write> {
    mla: ArchiveWriter<'a, W>,
    color_tuple_to_id: BTreeMap<[u8; 4], u8>,
    tile_placements: Vec<IntermediateTilePlacement>,
}

impl<'a, W: Write> PlacedArchiveWriter<'a, W> {
    pub fn new(dest: W) -> Self {
        let mut config = ArchiveWriterConfig::new();
        config.disable_layer(mla::Layers::ENCRYPT);
        // config.enable_layer(mla::Layers::COMPRESS);
        let mla = ArchiveWriter::from_config(dest, config).unwrap();

        PlacedArchiveWriter {
            mla,
            color_tuple_to_id: BTreeMap::new(),
            tile_placements: Vec::new(),
        }
    }

    pub fn add_tile(&mut self, x: u16, y: u16, color: [u8; 4], placed_at: NaiveDateTime) {
        let color_map_len = self.color_tuple_to_id.len() as u8;
        let color_index = self
            .color_tuple_to_id
            .entry(color)
            .or_insert_with(|| color_map_len);

        self.tile_placements.push(IntermediateTilePlacement {
            x,
            y,
            placed_at,
            color_index: *color_index,
        });
    }

    pub fn finalize(&mut self) {
        self.tile_placements.sort_unstable();

        let first_tile_placed_at = self.tile_placements.first().unwrap().placed_at;
        let num_of_tiles_in_chunk = self.tile_placements.len() as u32 / NUM_CHUNKS;

        let mut chunk_descs: Vec<ChunkDescription> = Vec::new();

        for (i, tiles) in self
            .tile_placements
            .chunks(num_of_tiles_in_chunk as usize)
            .enumerate()
        {
            let mut tile_buf = Vec::new();

            for tile in tiles {
                bincode::encode_into_std_write(
                    StoredTilePlacement {
                        x: tile.x,
                        y: tile.y,
                        ms_since_epoch: tile
                            .placed_at
                            .signed_duration_since(first_tile_placed_at)
                            .num_milliseconds() as u32,
                        color_index: tile.color_index,
                    },
                    &mut tile_buf,
                    BINCODE_CONFIG,
                )
                .unwrap();
            }

            self.mla
                .add_file(
                    format!("tiles/{}", i).as_str(),
                    tile_buf.len() as u64,
                    tile_buf.as_slice(),
                )
                .unwrap();

            chunk_descs.push(ChunkDescription {
                id: i as u32,
                up_to_ms_since_epoch: tiles
                    .last()
                    .unwrap()
                    .placed_at
                    .signed_duration_since(first_tile_placed_at)
                    .num_milliseconds() as u32,
                num_tiles: tiles.len() as u32,
            });
        }

        // todo
        let canvas_size_changes = vec![CanvasSizeChange {
            ms_since_epoch: 0,
            width: 2000,
            height: 2000,
        }];

        let meta = Meta {
            canvas_size_changes,
            chunk_descs,
            // todo
            last_pixel_placed_at_seconds_since_epoch: 0,
            color_id_to_tuple: BTreeMap::from_iter(
                self.color_tuple_to_id.iter().map(|(k, v)| (*v, *k)),
            ),
        };

        let mut meta_buf = Vec::new();
        bincode::encode_into_std_write(meta.clone(), &mut meta_buf, BINCODE_CONFIG).unwrap();
        self.mla
            .add_file("meta", meta_buf.len() as u64, meta_buf.as_slice())
            .unwrap();

        // Generate snapshots
        let largest_canvas_size = meta.get_largest_canvas_size().unwrap();
        let mut canvas = RgbImage::new(
            largest_canvas_size.width as u32,
            largest_canvas_size.height as u32,
        );

        let mut num_of_processed_tiles = 0;
        for chunk in meta.chunk_descs {
            for tile in self.tile_placements[num_of_processed_tiles..]
                .iter()
                .take(chunk.num_tiles as usize)
            {
                canvas.put_pixel(
                    tile.x as u32,
                    tile.y as u32,
                    image::Rgb(
                        meta.color_id_to_tuple[&tile.color_index][0..3]
                            .try_into()
                            .unwrap(),
                    ),
                );
            }

            num_of_processed_tiles += chunk.num_tiles as usize;

            let mut temp_snapshot = tempfile().unwrap();
            let mut buf = Vec::new();
            // needs a seekable writer
            canvas
                .write_to(&mut temp_snapshot, image::ImageOutputFormat::Png)
                .unwrap();
            temp_snapshot.seek(SeekFrom::Start(0)).unwrap();
            temp_snapshot.read_to_end(&mut buf).unwrap();

            self.mla
                .add_file(
                    format!("snapshots/{}", chunk.id).as_str(),
                    buf.len() as u64,
                    buf.as_slice(),
                )
                .unwrap();
        }

        self.mla.finalize().unwrap();
    }
}
