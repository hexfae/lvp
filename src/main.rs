//! LVP: pixeLflut Video Processor.

mod client;
mod frame;
mod network;
mod video;

use client::{Client, ClientError};
use network::{Network, NetworkError};
use snafu::Snafu;

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Error> {
    let client = Client::new().await?;
    let mut network = Network::new().await?;

    let video = client.random_video().await?;

    network.send_video(video).await?;

    Ok(())
}

/// Errors that can occur when using the LVP client.
#[derive(Debug, Snafu)]
enum Error {
    /// See [`ClientError`].
    #[snafu(transparent)]
    Client {
        /// See [`ClientError`].
        source: ClientError,
    },
    /// See [`NetworkError`].
    #[snafu(transparent)]
    Network {
        /// See [`NetworkError`].
        source: NetworkError,
    },
}
