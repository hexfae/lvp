//! Process a video into a [`Video`].

use miette::Diagnostic;
use snafu::{OptionExt, ResultExt, Snafu};
use std::{num::TryFromIntError, path::PathBuf};
use tracing::debug;
use video_rs::{DecoderBuilder, Options, decode::Decoder};

pub type Frame = Vec<u8>;

pub struct Video {
    /// The video's decoder.
    ///
    /// This is where most of the interesting stuff can be found (metadata).
    decoder: Decoder,
    /// How many frames have been processed.
    ///
    /// This is needed for a workaround.
    frames_processed: u64,
}

/// All errors that can occur while processing a video.
#[derive(Debug, Snafu, Diagnostic)]
pub enum ProcessingError {
    /// Creating the decoder failed.
    ///
    /// Probably something wrong with your system.
    #[snafu(display("error while creating decoder: {source}"))]
    CreateDecoder { source: video_rs::Error },
    /// Reading the metadata failed.
    ///
    /// Probably something wrong with the file.
    #[snafu(display("error while reading metadata: {source}"))]
    MetadataError { source: video_rs::Error },
    /// Decoding the stream failed.
    ///
    /// Probably something wrong with the file.
    #[snafu(display("error while decoding stream: {source}"))]
    DecodeStream { source: video_rs::Error },
    /// Converting the array of pixels to a slice failed.
    ///
    /// Apparently, this can happen if it's non-contiguous and/or in non-standard order.
    #[snafu(display("error while converting array to slice"))]
    ArrayToSlice,
    /// There were too many pixels (for a 32-bit system).
    ///
    /// This can happen if the amount of pixels > [`u32::MAX`] and it tries to cast that as [`usize`].
    #[snafu(display("error while casting u64 to usize: {source}"))]
    TooManyPixels { source: TryFromIntError },
    /// There were too many frames (for a 32-bit system).
    ///
    /// This can happen if the amount of frames > [`u32::MAX`] and it tries to cast that as [`usize`].
    #[snafu(display("error while casting u64 to usize: {source}"))]
    TooManyFrames { source: TryFromIntError },
}

impl Video {
    /// # Errors
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, ProcessingError> {
        let decoder = DecoderBuilder::new(path.into())
            .with_options(&Options::preset_h264_realtime())
            .build()
            .context(CreateDecoderSnafu)?;

        let (width, height) = decoder.size();
        let duration = decoder.duration().context(MetadataSnafu)?;
        let frame_rate = decoder.frame_rate();
        let total_frames = decoder.frames().context(MetadataSnafu)?;

        debug!("duration: {:.2}s", duration.as_secs());
        debug!("frame rate: {frame_rate}");
        debug!("total frames: {total_frames}");
        debug!("dimensions: {width}x{height}");

        Ok(Self {
            decoder,
            frames_processed: 0,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.decoder.size().0
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.decoder.size().1
    }

    #[must_use]
    pub fn frame_rate(&self) -> f32 {
        self.decoder.frame_rate()
    }
}

impl Iterator for Video {
    type Item = Result<Frame, ProcessingError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.frames_processed >= self.decoder.frames().ok()? {
            return None;
        }

        match self.decoder.decode() {
            Ok((_timestamp, frame)) => {
                let frame_pixels = frame
                    .as_slice()
                    .context(ArrayToSliceSnafu)
                    .map(<[u8]>::to_vec);

                self.frames_processed += 1;
                Some(frame_pixels)
            }
            Err(e) => {
                if matches!(e, video_rs::Error::ReadExhausted) {
                    return None;
                }

                Some(Err(ProcessingError::DecodeStream { source: e }))
            }
        }
    }
}

impl ExactSizeIterator for Video {
    fn len(&self) -> usize {
        usize::try_from(self.decoder.frames().unwrap_or_default() - self.frames_processed)
            .unwrap_or_default()
    }
}
