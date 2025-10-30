//! LVP: pixeLflut Video Processor.

mod client;
mod frame;
mod network;
mod video;

use client::{BucketNameUnsetError, Client, ListVideosError, SelectVideoError};
use network::{Network, NetworkError};
use snafu::Snafu;

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Error> {
    let client = Client::new().await?;
    let mut network = Network::new().await?;

    let videos = client.list_videos().await?;

    let Some(first) = videos.first() else {
        return Ok(());
    };

    let video = client.select_video(first).await?;
    let width = video.width();

    for frame in video {
        network.send_pixels(frame, width).await?;
    }

    Ok(())
}

/// Errors that can occur when using the LVP client.
#[derive(Debug, Snafu)]
enum Error {
    /// See [`BucketNameUnsetError`].
    #[snafu(transparent)]
    BucketNameUnset {
        /// See [`BucketNameUnsetError`].
        source: BucketNameUnsetError,
    },
    /// See [`ListVideosError`].
    #[snafu(transparent)]
    ListVideos {
        /// See [`ListVideosError`].
        #[snafu(source(from(ListVideosError, Box::new)))]
        source: Box<ListVideosError>,
    },
    /// See [`SelectVideoError`].
    #[snafu(transparent)]
    SelectVideo {
        /// See [`SelectVideoError`].
        #[snafu(source(from(SelectVideoError, Box::new)))]
        source: Box<SelectVideoError>,
    },
    /// See [`NetworkError`].
    #[snafu(transparent)]
    Network {
        /// See [`NetworkError`].
        source: NetworkError,
    },
}
