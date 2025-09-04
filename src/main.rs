//! pixelfLut (pixelpwnr) Video Processor
#![feature(thread_sleep_until)]
use clap::Parser;
use lvp::{Args, Client};
use std::time::Duration;
use tracing::error;

/// Ten seconds. how long the video plays.
const TEN_SECONDS: Duration = Duration::from_secs(10);

/// A catch-all error.
type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    log()?;
    video_rs::init().expect("ffmpeg installed");

    let args = Args::parse();
    let mut rng = rand::rng();
    let mut client = Client::new(&mut rng, args)?;

    loop {
        if let Err(why) = client.send(TEN_SECONDS) {
            error!("error while sending video: {why}");
        }
        if let Err(why) = client.switch_video(&mut rng) {
            error!("error while switching video: {why}");
        }
    }
}

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// Start logging with LVP set to DEBUG and everything else to INFO.
fn log() -> Result<(), Error> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?
        .add_directive("lvp=debug".parse()?);
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
