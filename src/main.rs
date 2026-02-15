//! LVP: pixeLflut Video Processor.

mod client;
mod frame;
mod network;
mod video;

use std::time::Duration;

use client::{S3Client, S3Error};
use lvp::CONFIG;
use network::{Network, NetworkError};
use snafu::Snafu;
use tokio::time::sleep;
use tracing::{error, info};

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();
    info!("launching lvp 0.1.0");
    let client = S3Client::new().await;

    loop {
        let mut network = Network::new().await?;
        loop {
            let video = client.random_video().await?;
            if let Err(why) = network.send_video(video).await {
                error!("Error sending video: {why}");
                sleep(Duration::from_secs(1)).await;
                break;
            }
        }
    }
}

/// Errors that can occur when using the LVP client.
#[derive(Debug, Snafu)]
enum Error {
    /// See [`ClientError`].
    #[snafu(transparent)]
    Client {
        /// See [`ClientError`].
        source: S3Error,
    },
    /// See [`NetworkError`].
    #[snafu(transparent)]
    Network {
        /// See [`NetworkError`].
        source: NetworkError,
    },
}
