//! LVP: pixeLflut Video Processor.

mod client;

use client::Client;
use snafu::Snafu;

use crate::client::{BucketNameUnsetError, ListVideosError};

#[tokio::main]
#[snafu::report]
async fn main() -> Result<(), Error> {
    let client = Client::new().await?;

    let videos = client.list_videos().await?;

    for video in videos {
        println!("{video}");
    }

    Ok(())
}

/// Errors that can occur when using the LVP client.
#[derive(Debug, Snafu)]
enum Error {
    /// See [`BucketNameUnsetError`].
    #[snafu(transparent)]
    BucketNameUnsetError {
        /// See [`BucketNameUnsetError`].
        source: BucketNameUnsetError,
    },
    /// See [`ListVideosError`].
    #[snafu(transparent)]
    ListVideosError {
        /// See [`ListVideosError`].
        #[snafu(source(from(ListVideosError, Box::new)))]
        source: Box<ListVideosError>,
    },
}
