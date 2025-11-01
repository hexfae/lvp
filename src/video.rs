//! The video struct containing the bytes of the video.

use aws_sdk_s3::operation::get_object::GetObjectOutput;
use snafu::{ResultExt, Snafu};
use std::io;
use tempfile::NamedTempFile;
use tokio::{fs::File, task::spawn_blocking};
use video_rs::Decoder;

use crate::frame::{Frame, Pixel};

/// A wrapper around a video.
pub struct Video {
    /// The `video-rs` decoder.
    decoder: Decoder,
}

/// An error occured while reading a video.
#[derive(Debug, Snafu)]
pub enum ReadVideoError {
    /// Reading the S3 object body failed.
    #[snafu(transparent)]
    Read {
        /// The source of the error.
        source: io::Error,
    },
    /// See [`DecodeError`].
    #[snafu(transparent)]
    Decode {
        /// See [`DecodeError`].
        source: DecodeError,
    },
}

/// An error occurred while decoding a video.
#[derive(Debug, Snafu)]
pub enum DecodeError {
    /// An error occurred while creating a temporary file.
    #[snafu(display("Failed to create temporary file"))]
    CreateTempFile {
        /// The source of the error.
        source: io::Error,
    },
    /// An error occurred while writing to a temporary file.
    #[snafu(display("Failed to write to temporary file at {}", path.display()))]
    WriteTempFile {
        /// The path of the temporary file.
        path: std::path::PathBuf,
        /// The source of the error.
        source: io::Error,
    },
    /// An error occurred while creating a decoder from a file.
    #[snafu(display("Failed to create decoder from {}", path.display()))]
    CreateDecoder {
        /// The path of the video file.
        path: std::path::PathBuf,
        /// The source of the error.
        source: video_rs::Error,
    },
}

impl Video {
    /// Creates a new video from an S3 object.
    pub async fn from_object(object: GetObjectOutput) -> Result<Self, ReadVideoError> {
        // video-rs requires a path (or url) to decode from
        let tmp_file = NamedTempFile::new().context(CreateTempFileSnafu)?;
        let path = tmp_file.path().to_owned();

        let mut file = File::create(&path)
            .await
            .context(WriteTempFileSnafu { path: path.clone() })?;
        let mut body = object.body.into_async_read();
        tokio::io::copy(&mut body, &mut file)
            .await
            .context(WriteTempFileSnafu { path: path.clone() })?;

        drop(file);

        let decoder =
            spawn_blocking(move || Decoder::new(path.clone()).context(CreateDecoderSnafu { path }))
                .await
                .expect("tokio join error")?;

        Ok(Self { decoder })
    }

    /// Returns the width of the video.
    pub fn width(&self) -> u32 {
        self.decoder.size().0
    }

    /// Returns the frame rate of the video.
    pub fn frame_rate(&self) -> f32 {
        self.decoder.frame_rate()
    }
}

impl Iterator for Video {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.decoder.decode().ok()?.1;

        Some(
            frame
                // reduce color "resolution" to cache more pixels
                .mapv_into(|pixel| (pixel / 10) * 10)
                .into_flat()
                .exact_chunks(3)
                .into_iter()
                .map(Pixel::from)
                .collect(),
        )
    }
}
