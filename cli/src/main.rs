use archive::PlacedArchiveWriter;
use chrono::NaiveDateTime;
use clap::{Parser, Subcommand};
use colors_transform::Color;
use std::fs::File;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Repack data from a CSV into an archive containing color and tile data
    Pack { in_file: String, out_file: String },
    /// Render history to an image
    Render {
        archive_path: String,
        out_file: String,
        #[clap(short, long, default_value = "0")]
        /// if 0, render all history
        up_to_seconds: u32,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Pack { in_file, out_file } => {
            let file = File::open(in_file).expect("Could not open file");
            let mut reader = csv::Reader::from_reader(file);

            let out_file = File::create(out_file).expect("Could not create file");
            let mut archive_writer = PlacedArchiveWriter::new(out_file);

            for result in reader.records() {
                let record = result.expect("Could not read record");

                let placed_at = NaiveDateTime::parse_from_str(
                    record.get(0).unwrap(),
                    "%Y-%m-%d %H:%M:%S%.3f UTC",
                )
                .expect("Could not parse timestamp");

                let color_str = record.get(2).unwrap().to_string();
                let parsed_color = colors_transform::Rgb::from_hex_str(&color_str).unwrap();

                // todo: panic if coords contain more than 1 ,

                let clean_coords = record.get(3).unwrap().replace('"', "");
                let mut coords = clean_coords.split(',');
                let x_str = coords.next().unwrap();
                let y_str = coords.next().unwrap();
                let x = x_str.parse::<u16>().expect("Could not parse x coordinate");
                let y = y_str.parse::<u16>().expect("Could not parse y coordinate");

                archive_writer.add_tile(
                    x,
                    y,
                    [
                        parsed_color.get_red() as u8,
                        parsed_color.get_green() as u8,
                        parsed_color.get_blue() as u8,
                        0xff,
                    ],
                    placed_at,
                );
            }

            archive_writer.finalize();
        }
        Commands::Render {
            archive_path,
            out_file,
            up_to_seconds,
        } => {
            let mut placed_archive = reader::PlacedArchive::load(archive_path).unwrap();
            let image = placed_archive.render_up_to(up_to_seconds);
            image.save(out_file).unwrap();
        }
    }
}
