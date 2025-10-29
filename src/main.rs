//! LVP: pixeLflut Video Processor.

mod client;
mod decoder;
mod frame;
mod video;

use client::Client;
use snafu::Snafu;

use crate::{
    client::{BucketNameUnsetError, ListVideosError, SelectVideoError},
    decoder::{DecodeError, Decoder},
};

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Error> {
    let client = Client::new().await?;

    let videos = client.list_videos().await?;

    let Some(first) = videos.first() else {
        return Ok(());
    };

    let video = client.select_video(first).await?;
    let mut decoder = Decoder::new(video.bytes())?;
    let Some(frame) = decoder.next() else {
        return Ok(());
    };

    println!("pixels: {}", frame.len());

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
    /// See [`DecodeError`].
    #[snafu(transparent)]
    Decoder {
        /// See [`DecodeError`].
        source: DecodeError,
    },
}
