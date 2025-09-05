//! pixelfLut (pixelpwnr) Video Processor
use clap::Parser;
use lvp::client::ClientError;
use lvp::{Args, Client, ProcessingError, Video};
use snafu::ResultExt;
use std::time::Duration;
use tracing::{error, level_filters::LevelFilter};
use tracing_subscriber::filter::{EnvFilter, FromEnvError, ParseError};

#[tokio::main]
async fn main() -> Result<(), Error> {
    log()?;
    video_rs::init().context(VideoRsSnafu)?;

    let args = Args::parse();
    let mut rng = rand::rng();
    let mut client = Client::new(&args).await?;

    loop {
        let video = Video::from_directory(&mut rng, args.directory())?;
        if let Err(why) = client.send(video, TEN_SECONDS).await {
            error!("error while sending video: {why}");
        }
        let video = Video::load_static(args.directory())?;
        if let Err(why) = client.send(video, ONE_SECOND).await {
            error!("error while sending video: {why}");
        }
    }
}

/// Start logging with LVP set to DEBUG and everything else to INFO.
fn log() -> Result<(), Error> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .context(LogFromEnvSnafu)?
        .add_directive("lvp=debug".parse().context(ParseLogFilterSnafu)?);
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}

/// How long the static plays.
const ONE_SECOND: Duration = Duration::from_secs(1);

/// How long the video plays.
const TEN_SECONDS: Duration = Duration::from_secs(10);

/// All of the errors that can occur during video playback.
#[derive(Debug, snafu::Snafu)]
enum Error {
    /// Initializing ffmpeg during startup failed.
    #[snafu(display("initializing ffmpeg failed: {source}"))]
    VideoRs {
        /// The source of the error.
        source: Box<dyn std::error::Error>,
    },
    /// Sending the video to the server failed.
    #[snafu(transparent)]
    Client {
        /// The source of the error.
        source: ClientError,
    },
    /// Processing the video failed.
    #[snafu(transparent)]
    Processing {
        /// The source of the error.
        source: ProcessingError,
    },
    #[snafu(display("parsing log directives from environment failed: {source}"))]
    /// Parsing log directives from environment failed.
    LogFromEnv {
        /// The source of the error.
        source: FromEnvError,
    },
    #[snafu(display("parsing log directives from binary failed: {source}"))]
    /// Parsing given log filter directives failed.
    ParseLogFilter {
        /// The source of the error.
        source: ParseError,
    },
}
