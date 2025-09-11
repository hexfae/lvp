//! Process a video from a path into a [`Video`].

use rand::{Rng, rngs::ThreadRng, seq::IteratorRandom};
use snafu::{OptionExt, ResultExt, Snafu, ensure};
use std::path::PathBuf;
use tracing::debug;
use video_rs::{DecoderBuilder, Options, decode::Decoder};
use walkdir::WalkDir;

use crate::client::Dimensions;

/// The filename of the static played between videos.
const STATIC_VIDEO: &str = "static.mp4";

/// Ten seconds in milliseconds.
///
/// This ensures that videos won't have their random timestamp be right at the end and immediately end.
const TEN_SECONDS_IN_MILLISECONDS: i64 = 10 * 1000;

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

/// The bytes that make up a frame of a video.
type Frame = Vec<u8>;

/// A video that is to be decoded.
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
    /// The directory contained no videos (that aren't `static.mp4`).
    #[snafu(display("no videos found in {}", path.display()))]
    NoVideoFound { path: PathBuf },
    /// The directory did not contain a file called `static.mp4`.
    #[snafu(display("no static.mp4 found in {}", path.display()))]
    NoStaticFound { path: PathBuf },
    /// Seeking in the video failed.
    ///
    /// Probably it tried to go to before or after the video somehow.
    #[snafu(display("error while seeking in video: {source}"))]
    VideoSeekError { source: video_rs::Error },
}

impl Video {
    /// Loads the video of static.
    ///
    /// # Errors
    ///
    /// Returns an error if a `static.mp4` file wasn't found in the given directory.
    pub fn load_static(
        path: impl Into<PathBuf>,
        dimensions: Dimensions,
    ) -> Result<Self, ProcessingError> {
        let path = path.into().join(STATIC_VIDEO);
        let name = "static".to_owned();
        ensure!(path.exists(), NoStaticFoundSnafu { path });
        let decoder = DecoderBuilder::new(path)
            .with_options(&Options::preset_h264_realtime())
            .with_resize(video_rs::Resize::Exact(
                dimensions.video_width(),
                dimensions.video_height(),
            ))
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
        dimensions: Dimensions,
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
            .with_resize(video_rs::Resize::Exact(
                dimensions.video_width(),
                dimensions.video_height(),
            ))
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
        // TODO: fix this +2 workaround
        // let timestamp = rng.random_range(0..millis.saturating_sub(TEN_SECONDS_IN_MILLISECONDS) + 2);
        // decoder.seek(timestamp).context(VideoSeekSnafu)?;

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
                frame
                    .iter_mut()
                    .for_each(|byte| *byte = LUT[*byte as usize]);

                #[expect(deprecated)] // not relevant for our use case
                Some(Ok(frame.into_raw_vec()))
            }
            Err(video_rs::Error::ReadExhausted) => None,
            Err(e) => Some(Err(ProcessingError::DecodeStream { source: e })),
        }
    }
}
