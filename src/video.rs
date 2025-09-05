//! Process a video from a path into a [`Video`].

use rand::{Rng, rngs::ThreadRng, seq::IteratorRandom};
use snafu::{OptionExt, ResultExt, Snafu, ensure};
use std::{num::TryFromIntError, path::PathBuf};
use tracing::debug;
use video_rs::{DecoderBuilder, Options, decode::Decoder};
use walkdir::WalkDir;

/// The filename of the static played between videos.
pub const STATIC_VIDEO: &str = "static.mp4";

/// What to multiply seconds with to get milliseconds.
const TO_MILLI: f32 = 1000.0;

/// A look-up-table for the reduced color "resolution" of videos.
const LUT: [u8; 256] = {
    let mut table = [0; 256];
    let mut i = 0;
    #[expect(clippy::cast_possible_truncation)]
    while i < 256 {
        table[i] = ((i as u8) / 10) * 10;
        i += 1;
    }
    table
};

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
#[derive(Debug, Snafu)]
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
    NoVideoFound { path: PathBuf },
    #[snafu(display("error while seeking in video: {source}"))]
    VideoSeekError { source: video_rs::Error },
}

impl Video {
    /// Loads the video of static.
    ///
    /// # Errors
    ///
    /// Returns an error if a `static.mp4` file wasn't found in the given directory.
    pub fn load_static(path: impl Into<PathBuf>) -> Result<Self, ProcessingError> {
        let path = path.into().join(STATIC_VIDEO);
        let name = "static".to_owned();
        ensure!(path.exists(), NoVideoFoundSnafu { path });
        let decoder = DecoderBuilder::new(path)
            .with_options(&Options::preset_h264_realtime())
            .build()
            .context(CreateDecoderSnafu)?;
        Ok(Self { decoder, name })
    }

    /// Loads a random video from the given directory.
    ///
    /// # Errors
    ///
    /// Returns an error if no video was found, a decoder could not be built,
    /// the metadata is corrupted, or it attempted to seek to an invalid point.
    pub fn from_directory(
        rng: &mut ThreadRng,
        path: impl Into<PathBuf>,
    ) -> Result<Self, ProcessingError> {
        let path = path.into();
        let path = WalkDir::new(&path)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.file_name() != STATIC_VIDEO)
            .choose(rng)
            .context(NoVideoFoundSnafu { path })?
            .into_path();
        let mut decoder = DecoderBuilder::new(&*path)
            .with_options(&Options::preset_h264_realtime())
            .build()
            .context(CreateDecoderSnafu)?;

        let frame_rate = decoder.frame_rate();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        debug!("loaded {name} ({frame_rate}fps)",);
        let duration = decoder.duration().context(MetadataSnafu)?;
        #[expect(clippy::cast_possible_truncation)] // this does not matter
        let millis = (duration.as_secs() * TO_MILLI) as i64;
        let timestamp = rng.random_range(0..millis);
        decoder.seek(timestamp).context(VideoSeekSnafu)?;

        Ok(Self { decoder, name })
    }

    #[must_use]
    pub fn frame_rate(&self) -> f32 {
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
            Ok((_, mut frame)) => {
                let Some(slice) = frame.as_slice_mut() else {
                    return Some(Err(ProcessingError::ArrayToSlice));
                };

                for byte in slice.iter_mut() {
                    *byte = LUT[*byte as usize];
                }

                Some(Ok(slice.to_vec()))
            }
            Err(video_rs::Error::ReadExhausted) => None,
            Err(e) => Some(Err(ProcessingError::DecodeStream { source: e })),
        }
    }
}
