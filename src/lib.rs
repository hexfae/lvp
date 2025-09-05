//! LUDD video processor.
pub mod client;
pub mod video;

pub use client::Client;
pub use video::ProcessingError;
pub use video::Video;

use clap::{Parser, arg, command};

/// The arguments provided by the user on startup.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The address of the pixelflut server.
    #[arg(short, long)]
    address: String,

    /// The path to the directory where videos are stored.
    #[arg(short, long)]
    path: String,

    /// Use binary pixel commands (PB). Recommended if connecting to a pixelpwnr server.
    #[arg(short, long)]
    binary: bool,
}

impl Args {
    /// Returns the directory where videos are stored.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}
