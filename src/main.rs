//! pixelfLut (pixelpwnr) Video Processor
#![feature(thread_sleep_until)]
use clap::Parser;
use lvp::{Args, Client, Video};
use std::time::Duration;
use tracing::error;

/// Ten seconds. How long the static plays.
const ONE_SECOND: Duration = Duration::from_secs(1);

/// Ten seconds. How long the video plays.
const TEN_SECONDS: Duration = Duration::from_secs(10);

/// A catch-all error.
type Error = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    log()?;
    video_rs::init().expect("ffmpeg installed");

    let args = Args::parse();
    let mut rng = rand::rng();
    let mut client = Client::new(&args).await?;

    loop {
        let video = Video::from_directory(&mut rng, args.path())?;
        if let Err(why) = client.send(video, TEN_SECONDS).await {
            error!("error while sending video: {why}");
        }
        let video = Video::load_static(args.path())?;
        if let Err(why) = client.send(video, ONE_SECOND).await {
            error!("error while sending video: {why}");
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
