//! LVP: pixeLflut Video Processor.

mod client;
mod frame;
mod network;
mod video;

use client::{S3Client, S3Error};
use lvp::CONFIG;
use network::{Network, NetworkError};
use snafu::Snafu;

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Error> {
    let client = S3Client::new().await;

    loop {
        let mut network = Network::new().await?;
        loop {
            let video = client.random_video().await?;
            if let Err(why) = network.send_video(video).await {
                eprintln!("Error sending video: {why}");
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
