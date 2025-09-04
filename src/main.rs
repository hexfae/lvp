//! pixelfLut (pixelpwnr) Video Processor
#![feature(thread_sleep_until)]
use inquire::Text;
use lvp::Client;
use std::time::Duration;
use tracing::error;

/// Ten seconds. how long the video plays.
const TEN_SECONDS: Duration = Duration::from_secs(10);

/// A catch-all error.
type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    log()?;
    video_rs::init().expect("ffmpeg installed");

    let Ok(address) = Text::new("address and port:").prompt() else {
        println!("no address and port was given :(");
        return Ok(());
    };
    let Ok(path) = Text::new("video storage directory:").prompt() else {
        println!("no video storage directory was given :(");
        return Ok(());
    };

    let mut rng = rand::rng();
    let mut client = Client::new(&mut rng, path, address)?;

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
