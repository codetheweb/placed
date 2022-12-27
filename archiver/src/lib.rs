use chrono::NaiveDateTime;
use reader::PlacedArchive;
use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Seek};
use structures::{Meta, PixelPlacement, Snapshot};
use tempfile::tempfile;

// This isn't very efficient but only needs to run once :)
/// Creates an archive from a CSV file.
/// If `pack_up_to_seconds` is set to 0, the entire history will be packed.
pub fn pack(
    in_file: String,
    out_file: String,
    block_size: usize,
    pack_up_to_seconds: u32,
    compressed: bool,
) {
    let file = fs::File::open(in_file).expect("Could not open file");
    let mut reader = csv::Reader::from_reader(file);

    // Create archive stream
    let mut out_file = fs::File::create(out_file).expect("Could not create file");

    let mut temp_pixel_data_file = tempfile().unwrap();

    let mut colors = HashMap::new();

    let mut num_of_pixel_placements = 0;
    let mut last_pixel_placed_at_seconds_since_epoch = 0;

    {
        let mut data_writer_buffered = BufWriter::new(&mut temp_pixel_data_file);

        let mut first_timestamp = None;

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
            if !colors.contains_key(&color_str) {
                colors.insert(color_str.clone(), colors.len() as u16);
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

            if pack_up_to_seconds != 0 && seconds_since_epoch > pack_up_to_seconds {
                break;
            }

            bincode::encode_into_std_write(
                PixelPlacement {
                    x,
                    y,
                    seconds_since_epoch,
                    color_index: *colors.get(&color_str).unwrap() as u8,
                },
                &mut data_writer_buffered,
                bincode::config::standard(),
            )
            .unwrap();

            num_of_pixel_placements += 1;
            last_pixel_placed_at_seconds_since_epoch = seconds_since_epoch;
        }
    }

    let meta = Meta {
        width: 2000,
        height: 2000,
        num_of_pixel_placements,
        last_pixel_placed_at_seconds_since_epoch,
        colors,
        snapshots: Vec::new(),
    };

    bincode::encode_into_std_write(meta, &mut out_file, bincode::config::standard()).unwrap();

    // Copy from temp file
    temp_pixel_data_file
        .seek(std::io::SeekFrom::Start(0))
        .unwrap();
    std::io::copy(&mut temp_pixel_data_file, &mut out_file).unwrap();
}

pub fn generate_snapshots(in_file_path: String, out_file_path: String, num_snapshots: u16) {
    let mut archive = PlacedArchive::load(in_file_path.clone()).expect("Could not load archive");

    let mut snapshot_points_in_seconds: Vec<u32> = Vec::new();
    let seconds_between_snapshots =
        archive.meta.last_pixel_placed_at_seconds_since_epoch / num_snapshots as u32;
    for i in 0..num_snapshots {
        snapshot_points_in_seconds.push((i as u32) * seconds_between_snapshots);
    }

    let mut temp_snapshot_file = tempfile().unwrap();

    let mut snapshots: Vec<Snapshot> = Vec::new();

    for snapshot_point in snapshot_points_in_seconds {
        let snapshot = archive.render_up_to(snapshot_point);
        let start_offset = temp_snapshot_file
            .seek(std::io::SeekFrom::Current(0))
            .unwrap();
        snapshot
            .write_to(&mut temp_snapshot_file, image::ImageOutputFormat::Png)
            .unwrap();
        let end_offset = temp_snapshot_file
            .seek(std::io::SeekFrom::Current(0))
            .unwrap();
        let length = end_offset - start_offset;

        snapshots.push(Snapshot {
            up_to_seconds_since_epoch: snapshot_point,
            start_offset,
            length,
        });
    }

    let mut out_file = fs::File::create(out_file_path).expect("Could not create file");

    let mut meta = archive.meta.clone();
    meta.snapshots = snapshots;

    bincode::encode_into_std_write(meta, &mut out_file, bincode::config::standard()).unwrap();

    // Copy snapshots from temp file
    temp_snapshot_file
        .seek(std::io::SeekFrom::Start(0))
        .unwrap();
    std::io::copy(&mut temp_snapshot_file, &mut out_file).unwrap();

    // Copy pixel data
    let mut in_file = fs::File::open(in_file_path).expect("Could not open file");

    PlacedArchive::seek_to_pixel_data(&mut in_file);
    std::io::copy(&mut in_file, &mut out_file).unwrap();
}
