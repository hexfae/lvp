//! The video struct containing the bytes of the video.

use snafu::{ResultExt, Snafu};
use std::{
    env::{VarError, var},
    io,
    num::ParseIntError,
};
use video_rs::{Decoder, DecoderBuilder, Resize, Url};

use crate::frame::Frame;

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
    /// An error occurred while creating a decoder from a URL.
    #[snafu(display("Failed to create decoder from {url}"))]
    CreateDecoder {
        /// The URL.
        url: String,
        /// The source of the error.
        source: video_rs::Error,
    },
}

impl Video {
    /// Retrieves a video from a url.
    ///
    /// # Errors
    ///
    /// Returns an error if `LVP_MAX_WIDTH` or `LVP_MAX_HEIGHT` failed to
    /// parse, or if the video failed to
    pub fn from_url(url: &Url) -> Result<Self, ReadVideoError> {
        let width = var("LVP_MAX_WIDTH")
            .context(NoWidthSnafu)?
            .parse()
            .context(InvalidWidthSnafu)?;
        let height = var("LVP_MAX_HEIGHT")
            .context(NoHeightSnafu)?
            .parse()
            .context(InvalidHeightSnafu)?;

        let decoder = DecoderBuilder::new(url)
            .with_resize(Resize::FitEven(width, height))
            .build()
            .map_err(|e| ReadVideoError::CreateDecoder {
                url: url.to_string(),
                source: e,
            })?;

        Ok(Self { decoder })
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
        let data = self.decoder.decode().ok()?.1.flatten().to_vec();
        let (width, height) = self.decoder.size_out();

        Some(Frame {
            data,
            width,
            height,
        })
    }
}
