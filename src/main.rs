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
    loop {
        let mut client = Client::new(&args).await?;
        loop {
            let video = match Video::from_directory(&mut rng, args.directory(), client.dimensions())
            {
                Ok(video) => video,
                Err(why) => {
                    error!("error while selecting video: {why}");
                    break;
                }
            };
            // TODO: remove the "play for x seconds" logic entirely
            if let Err(why) = client.send(video, Duration::MAX).await {
                error!("error while sending video: {why}");
                break;
            }
            // let video = match Video::load_static(args.directory(), client.dimensions()) {
            //     Ok(video) => video,
            //     Err(why) => {
            //         error!("error while selecting static: {why}");
            //         break;
            //     }
            // };
            // if let Err(why) = client.send(video, TWO_SECONDS).await {
            //     error!("error while sending video: {why}");
            //     break;
            // }
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
