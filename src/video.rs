//! The video struct containing the bytes of the video.

use aws_sdk_s3::operation::get_object::GetObjectOutput;
use snafu::Snafu;
use std::{fmt::Display, io};
use tokio::io::AsyncReadExt;

/// A wrapper around the bytes of a video.
pub struct Video {
    /// The bytes of the video.
    bytes: Vec<u8>,
}

/// The name of a video.
pub struct VideoName(String);

/// An error occured while reading a video.
#[derive(Debug, Snafu)]
#[snafu(transparent)]
pub struct ReadVideoError {
    /// The source of the error.
    source: io::Error,
}

impl Video {
    /// Creates a new video from an S3 object.
    pub async fn from_object(object: GetObjectOutput) -> Result<Self, ReadVideoError> {
        let mut bytes = vec![];
        object
            .body
            .into_async_read()
            .read_to_end(&mut bytes)
            .await?;
        Ok(Self { bytes })
    }

    /// Returns the bytes of the video.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Display for VideoName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&VideoName> for String {
    fn from(video: &VideoName) -> Self {
        video.0.clone()
    }
}

impl From<String> for VideoName {
    fn from(name: String) -> Self {
        Self(name)
    }
}
