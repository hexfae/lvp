//! Process a video into a [`Video`].

use indicatif::{MultiProgress, ProgressBar};
use miette::Diagnostic;
use snafu::{OptionExt, ResultExt, Snafu};
use std::{num::TryFromIntError, path::PathBuf, time::Instant};
use tracing::debug;
use video_rs::{Time, decode::Decoder};

/// The number of bytes per pixel, assuming RGB24.
const BYTES_PER_PIXEL: u64 = 3;

/// A processed video.
#[derive(Debug)]
pub struct Video {
    /// How long the video is.
    duration: Time,
    /// The pixels that make up this video.
    pixels: Vec<u8>,
    /// The dimensions of the video.
    dimensions: Dimensions,
    /// The frame rate of the video.
    frame_rate: f32,
}

/// The dimensions of a video.
#[derive(Debug)]
pub struct Dimensions {
    /// The width of the video.
    width: u32,
    /// The height of the video.
    height: u32,
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
    /// Construct a new [`Video`].
    const fn new(duration: Time, pixels: Vec<u8>, dimensions: Dimensions, frame_rate: f32) -> Self {
        Self {
            duration,
            pixels,
            dimensions,
            frame_rate,
        }
    }

    /// Process a video file found at `path` into a [`Video`].
    ///
    /// # Errors
    ///
    /// See the documentation for [`ProcessingError`].
    // TODO: convert tv/limited color range (16-231) to full color range (0-255)
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, ProcessingError> {
        let mut decoder = Decoder::new(path.into()).context(CreateDecoderSnafu)?;

        let (width, height) = decoder.size();
        let n_pixels_per_frame = u64::from(width) * u64::from(height);
        let n_frames = decoder.frames().context(MetadataSnafu)?;
        let n_frames_usize = usize::try_from(n_frames).context(TooManyFramesSnafu)?;
        let calculated_n_pixels_per_video =
            usize::try_from(n_frames * n_pixels_per_frame * BYTES_PER_PIXEL)
                .context(TooManyPixelsSnafu)?;
        let duration = decoder.duration().context(MetadataSnafu)?;
        let frame_rate = decoder.frame_rate();

        debug!("duration: {:.2}s", duration.as_secs());
        debug!("frame rate: {frame_rate}");
        debug!("total frames: {n_frames}");
        debug!("total pixels: {calculated_n_pixels_per_video}");
        debug!("dimensions: {width}x{height}");
        debug!("pixels per frame: {n_pixels_per_frame}");

        let mut pixels = Vec::with_capacity(calculated_n_pixels_per_video);

        let now = Instant::now();

        let progress = MultiProgress::new();
        let frame_progress = progress.add(ProgressBar::new(n_frames));

        for result in decoder
            .decode_iter()
            // we get a "stream exhausted" error without this
            .take(n_frames_usize)
        {
            frame_progress.inc(1);

            let (_, frame) = result.context(DecodeStreamSnafu)?;

            let frame_pixels = frame.as_slice().context(ArrayToSliceSnafu)?;
            pixels.extend(frame_pixels);
        }

        frame_progress.finish();

        debug!("finished in: {:.2}s", now.elapsed().as_secs_f32());

        let dimensions = Dimensions::new(width, height);
        Ok(Self::new(duration, pixels, dimensions, frame_rate))
    }

    /// Return the duration of the video.
    #[must_use]
    pub const fn duration(&self) -> Time {
        self.duration
    }

    /// Return the amount of frames in the video.
    #[must_use]
    pub const fn n_frames(&self) -> u64 {
        self.n_pixels() / self.n_pixels_per_frame() as u64
    }

    /// Return the amount of pixels in the video.
    #[must_use]
    pub const fn n_pixels(&self) -> u64 {
        self.pixels.len() as u64 / BYTES_PER_PIXEL
    }

    /// Return the amount of pixels in one frame of the video.
    #[must_use]
    pub const fn n_pixels_per_frame(&self) -> u32 {
        self.dimensions.width * self.dimensions.height
    }
}

impl Dimensions {
    /// Construct a [`Dimensions`] from a width and height.
    const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}
