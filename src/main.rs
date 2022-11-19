use chrono::NaiveDateTime;
use gzp::deflate::Gzip;
use gzp::par::compress::ParCompress;
use gzp::par::compress::ParCompressBuilder;
use gzp::ZWriter;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::fs;

#[macro_use]
extern crate serde_derive;
use rmp_serde::Serializer;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct PixelPlacement {
    x: u16,
    y: u16,
    seconds_since_epoch: u32,
    color_index: u8,
}

// This isn't very efficient but only needs to run once :)
fn main() {
    let args: Vec<_> = env::args().collect();

    let filename = &args[1];
    let out_filename = &args[2];

    let file = fs::File::open(filename).expect("Could not open file");
    let out = fs::File::create(out_filename).expect("Could not create file");

    let mut out_compressed_writer: ParCompress<Gzip> = ParCompressBuilder::new().from_writer(out);
    let mut out_serializer = Serializer::new(&mut out_compressed_writer);

    let mut reader = csv::Reader::from_reader(file);

    let mut first_timestamp = None;

    let mut color_map: HashMap<String, u16> = HashMap::new();

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
        if !color_map.contains_key(&color_str) {
            color_map.insert(color_str.clone(), color_map.len() as u16);
        }

        let clean_coords = record.get(3).unwrap().replace('"', "");
        let mut coords = clean_coords.split(',');
        let x_str = coords.next().unwrap();
        let y_str = coords.next().unwrap();
        let x = x_str.parse::<u16>().expect("Could not parse x coordinate");
        let y = y_str.parse::<u16>().expect("Could not parse y coordinate");

        let pixel = PixelPlacement {
            x,
            y,
            seconds_since_epoch: timestamp
                .signed_duration_since(first_timestamp.unwrap())
                .num_seconds() as u32,
            color_index: *color_map.get(&color_str).unwrap() as u8,
        };

        pixel.serialize(&mut out_serializer).unwrap();
    }

    out_compressed_writer.finish().unwrap();
}
