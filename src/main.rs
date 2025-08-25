//! LUDD video processor.
#![feature(thread_sleep_until)]

use inquire::Text;
use lvp::{Client, Video};
use miette::{IntoDiagnostic, Report};
use rfd::FileDialog;

#[allow(clippy::many_single_char_names)]
fn main() -> Result<(), Report> {
    log()?;
    #[allow(clippy::unwrap_used)] // the function this function calls internally can never error
    video_rs::init().unwrap();

    let input = Text::new("address and port:").prompt();
    let file = FileDialog::new().pick_file();
    if let Some(path) = file
        && let Ok(address) = input
    {
        loop {
            let video = Video::from_path(&path)?;
            let mut client = Client::new(video, &address);
            client.send()?;
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
