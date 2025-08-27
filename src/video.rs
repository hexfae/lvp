//! Process a video from a path into a [`Video`].

use miette::Diagnostic;
use rand::seq::{IndexedRandom, IteratorRandom};
use snafu::{OptionExt, ResultExt, Snafu};
use std::{num::TryFromIntError, path::PathBuf};
use tracing::debug;
use video_rs::{DecoderBuilder, Options, decode::Decoder};
use walkdir::WalkDir;

/// The filename of the static played between videos.
pub const STATIC_VIDEO: &str = "static.mp4";

/// Ten seconds as expressed in milliseconds (10 thousand).
const FIFTEEN_SECONDS: i64 = 15 * 1000;

/// What to multiply seconds with to get milliseconds.
const TO_MILLI: i64 = 1000;

pub type Frame = Vec<u8>;

pub struct Video {
    /// The video's decoder.
    ///
    /// This is where most of the interesting stuff can be found (metadata).
    decoder: Decoder,
    /// The video's name.
    name: String,
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
    #[snafu(display("no videos found in the selected folder"))]
    NoVideoFound,
    #[snafu(display("error while seeking in video: {source}"))]
    VideoSeekError { source: video_rs::Error },
}

impl Video {
    /// Loads a random video from the given directory.
    ///
    /// # Errors
    ///
    /// Returns an error if no video was found, a decoder could not be built,
    /// the metadata is corrupted, or it attempted to seek to an invalid point.
    pub fn from_directory(
        path: impl Into<PathBuf>,
        except: Option<String>,
    ) -> Result<Self, ProcessingError> {
        let path = if let Some(name) = except {
            let mut rng = rand::rng();
            WalkDir::new(path.into())
                .max_depth(1)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .filter(|entry| entry.file_name() != STATIC_VIDEO)
                .filter(|entry| entry.file_name().to_string_lossy() != name)
                .choose(&mut rng)
                .context(NoVideoFoundSnafu)?
                .into_path()
        } else {
            path.into().join("static.mp4")
        };
        let mut decoder = DecoderBuilder::new(&*path)
            .with_options(&Options::preset_h264_realtime())
            .build()
            .context(CreateDecoderSnafu)?;

        let mut rng = rand::rng();
        let duration = decoder.duration().context(MetadataSnafu)?;
        #[allow(clippy::cast_possible_truncation)] // i won't upload videos that long
        let duration = (duration.as_secs() as i64 * TO_MILLI) - FIFTEEN_SECONDS;
        let range = (0..=duration).collect::<Vec<_>>();
        let timestamp = range.choose(&mut rng).copied().unwrap_or_default();
        decoder.seek(timestamp).context(VideoSeekSnafu)?;

        let frame_rate = decoder.frame_rate();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        debug!("loaded {name} ({frame_rate}fps)",);

        Ok(Self { decoder, name })
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

    #[must_use]
    pub fn duration(&self) -> f32 {
        self.decoder.frame_rate()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Iterator for Video {
    type Item = Result<Frame, ProcessingError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.decoder.decode() {
            Ok((_timestamp, frame)) => {
                let frame_pixels = frame
                    .as_slice()
                    .context(ArrayToSliceSnafu)
                    .map(<[u8]>::to_vec);
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
