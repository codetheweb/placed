use chrono::NaiveDateTime;
use mla::config::ArchiveWriterConfig;
use mla::ArchiveWriter;
use reader::PlacedArchive;
use rmp_serde::Serializer;
use serde::ser::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::BufWriter;
use structures::{ColorMap, Meta, PixelPlacement, PixelPlacementPack};

// This isn't very efficient but only needs to run once :)
pub fn pack(in_file: String, out_file: String) {
    let file = fs::File::open(in_file).expect("Could not open file");
    let mut reader = csv::Reader::from_reader(file);

    // Create archive stream
    let out_file = fs::File::create(out_file).expect("Could not create file");
    let mut buffered_out_file = BufWriter::new(out_file);

    let mut archive_config = ArchiveWriterConfig::default();
    archive_config.disable_layer(mla::Layers::ENCRYPT);

    let mut mla = ArchiveWriter::from_config(&mut buffered_out_file, archive_config).unwrap();
    let data_file_id = mla.start_file("data").unwrap();

    let mut first_timestamp = None;
    let mut color_map = ColorMap {
        colors: HashMap::new(),
    };

    let mut meta = Meta {
        width: 2_000,
        height: 2_000,
        num_of_pixel_placements: 0,
        last_pixel_placed_at_seconds_since_epoch: 0,
    };

    for result in reader.records() {
        let record = result.expect("Could not read record");

        let timestamp =
            NaiveDateTime::parse_from_str(record.get(0).unwrap(), "%Y-%m-%d %H:%M:%S%.3f UTC")
                .expect("Could not parse timestamp");

        first_timestamp = match first_timestamp {
            Some(first_timestamp) => Some(first_timestamp),
            None => Some(timestamp),
        };

        let color_str = record.get(2).unwrap().to_string();
        if !color_map.colors.contains_key(&color_str) {
            color_map
                .colors
                .insert(color_str.clone(), color_map.colors.len() as u16);
        }

        let clean_coords = record.get(3).unwrap().replace('"', "");
        let mut coords = clean_coords.split(',');
        let x_str = coords.next().unwrap();
        let y_str = coords.next().unwrap();
        let x = x_str.parse::<u16>().expect("Could not parse x coordinate");
        let y = y_str.parse::<u16>().expect("Could not parse y coordinate");
        let seconds_since_epoch = timestamp
            .signed_duration_since(first_timestamp.unwrap())
            .num_seconds() as u32;

        let mut data = [0u8; std::mem::size_of::<alkahest::Packed<PixelPlacement>>()];
        alkahest::write(
            &mut data,
            PixelPlacementPack {
                x,
                y,
                seconds_since_epoch,
                color_index: *color_map.colors.get(&color_str).unwrap() as u8,
            },
        );

        mla.append_file_content(data_file_id, data.len() as u64, data.as_slice())
            .unwrap();

        meta.num_of_pixel_placements += 1;
        meta.last_pixel_placed_at_seconds_since_epoch = seconds_since_epoch;
    }

    mla.end_file(data_file_id).unwrap();

    let mut color_buffer = Vec::new();
    let mut color_serializer = Serializer::new(&mut color_buffer);

    color_map.serialize(&mut color_serializer).unwrap();
    mla.add_file("colors", color_buffer.len() as u64, color_buffer.as_slice())
        .unwrap();

    let mut meta_buffer = Vec::new();
    let mut meta_serializer = Serializer::new(&mut meta_buffer);

    meta.serialize(&mut meta_serializer).unwrap();
    mla.add_file("meta", meta_buffer.len() as u64, meta_buffer.as_slice())
        .unwrap();

    mla.finalize().unwrap();
}

pub fn generate_snapshots(in_file: String, out_file: String, num_snapshots: u16) {
    let archive = PlacedArchive::load(in_file).expect("Could not load archive");

    let mut snapshot_points_in_seconds: Vec<u32> = Vec::new();
    let seconds_between_snapshots =
        archive.meta.last_pixel_placed_at_seconds_since_epoch / num_snapshots as u32;
    for i in 0..num_snapshots {
        snapshot_points_in_seconds.push((i as u32) * seconds_between_snapshots);
    }
}
