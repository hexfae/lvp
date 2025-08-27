//! pixelfLut (pixelpwnr) Video Processor.

use inquire::Text;
use lvp::Client;
use miette::{IntoDiagnostic, Report};
use rfd::FileDialog;
use std::time::Duration;

/// Ten seconds. how long the video plays.
const TEN_SECONDS: Duration = Duration::from_secs(10);

#[allow(clippy::many_single_char_names)]
fn main() -> Result<(), Report> {
    log()?;
    #[allow(clippy::unwrap_used)] // the function this function calls internally can never error
    video_rs::init().unwrap();

    let input = Text::new("address and port:").prompt();
    let file = FileDialog::new().pick_folder();
    if let Some(path) = file
        && let Ok(address) = input
    {
        let mut client = Client::new(&path, &address)?;
        loop {
            client.send(TEN_SECONDS)?;
            client.switch_video()?;
        }
    }
    Ok(())
}

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

/// Start logging with LVP set to DEBUG and everything else to INFO.
fn log() -> Result<(), Report> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .into_diagnostic()?
        .add_directive("lvp=debug".parse().into_diagnostic()?);
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
