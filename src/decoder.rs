//! The decoder struct responsible for decoding video frames.

use std::io::{self, Write};

use snafu::{ResultExt, Snafu};
use tempfile::NamedTempFile;

use crate::frame::{Frame, Pixel};

/// A wrapper around the `video-rs` decoder.
pub struct Decoder {
    /// The `video-rs` decoder.
    decoder: video_rs::Decoder,
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

impl Decoder {
    /// Create a new decoder from a byte slice.
    ///
    pub fn new(bytes: &[u8]) -> Result<Self, DecodeError> {
        // creating a temporary file is necessary because `video-rs`
        // requires a file path (or a URL) to load a video from
        let tmp_file = NamedTempFile::new().context(CreateTempFileSnafu)?;
        let path = tmp_file.path();
        tmp_file
            .as_file()
            .write_all(bytes)
            .context(WriteTempFileSnafu { path })?;
        let decoder = video_rs::Decoder::new(path).context(CreateDecoderSnafu { path })?;
        Ok(Self { decoder })
    }
}

impl Iterator for Decoder {
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
