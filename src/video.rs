//! The video struct containing the bytes of the video.

use aws_sdk_s3::operation::get_object::GetObjectOutput;
use snafu::{ResultExt, Snafu};
use std::{
    env::{VarError, var},
    io,
    num::ParseIntError,
};
use tempfile::NamedTempFile;
use tokio::{fs::File, task::spawn_blocking};
use video_rs::{Decoder, DecoderBuilder, Resize};

use crate::frame::{Dimensions, Frame, Pixel};

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
    /// No width specified.
    #[snafu(display("LVP_MAX_WIDTH environment variable is not set"))]
    NoWidth {
        /// The source of the error.
        source: VarError,
    },
    /// No height specified.
    #[snafu(display("LVP_MAX_HEIGHT environment variable is not set"))]
    NoHeight {
        /// The source of the error.
        source: VarError,
    },
    /// Invalid width specified.
    #[snafu(display("LVP_MAX_WIDTH environment variable is not a valid number"))]
    InvalidWidth {
        /// The source of the error.
        source: ParseIntError,
    },
    /// Invalid height specified.
    #[snafu(display("LVP_MAX_HEIGHT environment variable is not a valid number"))]
    InvalidHeight {
        /// The source of the error.
        source: ParseIntError,
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
    ///
    /// # Errors
    ///
    /// Errors if creating the temporary file fails, writing to it fails, parsing
    /// ``LVP_MAX_WIDTH`` or ``LVP_MAX_HEIGHT`` fails, or building a decoder fails.
    ///
    /// # Panics
    ///
    /// Panics on Tokio join error
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

        let width = var("LVP_MAX_WIDTH")
            .context(NoWidthSnafu)?
            .parse()
            .context(InvalidWidthSnafu)?;
        let height = var("LVP_MAX_HEIGHT")
            .context(NoHeightSnafu)?
            .parse()
            .context(InvalidHeightSnafu)?;

        let decoder = spawn_blocking(move || {
            DecoderBuilder::new(path.clone())
                .with_resize(Resize::FitEven(width, height))
                .build()
                .context(CreateDecoderSnafu { path })
        })
        .await
        .expect("tokio join error")?;

        Ok(Self { decoder })
    }

    /// Returns the dimensions of the video.
    #[must_use]
    pub fn dimensions(&self) -> Dimensions {
        Dimensions::from(self.decoder.size_out())
    }

    #[must_use]
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
                .map(|rgb| Pixel::new(rgb[0], rgb[1], rgb[2]))
                .collect(),
        )
    }
}
