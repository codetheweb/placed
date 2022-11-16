use std::env;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use chrono::{NaiveDateTime};
use std::collections::HashMap;

fn main() {
    let args: Vec<_> = env::args().collect();

    let filename = &args[1];
    let out_filename = &args[2];

    let file = fs::File::open(filename).expect("Could not open file");
    let out = fs::File::create(out_filename).expect("Could not create file");

    let reader = BufReader::new(file);

    let mut first_timestamp = None;

    let mut color_map: HashMap<String, u16> = HashMap::new();

    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let columns = line.split(",").collect::<Vec<_>>();

        let timestamp = NaiveDateTime::parse_from_str(columns.get(0).unwrap(), "%Y-%m-%d %H:%M:%S%.3f UTC").expect("Could not parse timestamp");

        first_timestamp = match first_timestamp {
            Some(first_timestamp) => Some(first_timestamp),
            None => Some(timestamp),
        };

        let color_str = columns.get(2).unwrap().to_string();
        if !color_map.contains_key(&color_str) {
            color_map.insert(color_str.clone(), color_map.len() as u16);
        }

        let x_str = columns.get(3).unwrap().replace('"', "");
        let y_str = columns.get(4).unwrap().replace('"', "");
        let x = x_str.parse::<u32>().expect("Could not parse x coordinate");
        let y = y_str.parse::<u32>().expect("Could not parse y coordinate");

        println!("{} x: {} y: {} color: {}", timestamp, x, y, color_str.clone());
    }
}
