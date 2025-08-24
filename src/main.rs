//! LUDD video processor.

use lvp::Video;
use miette::{IntoDiagnostic, Report};
use rfd::FileDialog;
use tracing::info;

fn main() -> Result<(), Report> {
    log()?;
    #[allow(clippy::unwrap_used)] // the function this function calls internally can never error
    video_rs::init().unwrap();
    let file = FileDialog::new().pick_file();
    if let Some(path) = file {
        let video = Video::from_path(path)?;
        info!(
            "processed {} frames with {} pixels each ({} total)",
            video.n_frames(),
            video.n_pixels_per_frame(),
            video.n_pixels()
        );
    } else {
        info!("yeah whatever, kid");
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
