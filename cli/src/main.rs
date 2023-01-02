use archiver::{generate_snapshots, pack};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Repack data from a CSV into a zip containing colors and pixels
    Pack { in_file: String, out_file: String },
    /// Add snapshots to an existing zip
    GenerateSnapshots {
        in_file: String,
        out_file: String,
        num_snapshots: u16,
    },
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
            pack(in_file, out_file);
        }
        Commands::GenerateSnapshots {
            in_file,
            out_file,
            num_snapshots,
        } => {
            generate_snapshots(in_file, out_file, num_snapshots);
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
