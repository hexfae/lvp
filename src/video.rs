//! Process a video into a [`Video`].

use bon::Builder;
use indicatif::{MultiProgress, ProgressBar};
use miette::Diagnostic;
use snafu::{OptionExt, ResultExt, Snafu};
use std::{path::PathBuf, time::Instant};
use tracing::debug;
use video_rs::{Time, decode::Decoder};

/// How many bytes per pixel.
const BYTES_PER_PIXEL: [usize; 3] = [1, 1, 3];

/// A processed video.
#[derive(Debug, Builder)]
pub struct Video {
    /// How long the video is.
    duration: Time,
    /// The frames that make up this video.
    frames: Vec<Frame>,
    /// The dimensions of the video.
    dimensions: Dimensions,
}

/// A single frame of a video.
#[derive(Debug, Builder)]
pub struct Frame {
    /// How long this frame is displayed for.
    time: Time,
    /// The pixels that make up this frame.
    pixels: Vec<Pixel>,
}

/// The dimensions of a video.
#[derive(Debug, Builder)]
pub struct Dimensions {
    /// The width of the video.
    width: usize,
    /// The height of the video.
    height: usize,
}

/// A single pixel of a frame.
#[derive(Debug, Builder)]
pub struct Pixel {
    /// How red this pixel is.
    red: u8,
    /// How green this pixel is.
    green: u8,
    /// How blue this pixel is.
    blue: u8,
}

/// The metadata of a video.
pub struct Metadata {
    /// How long the video is.
    duration: Time,
    /// The framerate of the video.
    frame_rate: f32,
    /// The amount of frames in the video.
    n_frames: u64,
    /// The width of the video.
    width: usize,
    /// The height of the video.
    height: usize,
    /// The amount of pixels in a frame.
    n_pixels: usize,
    /// The amount of pixels in the video.
    n_pixels_per_video: u64,
}

/// All errors that can occur while processing a video.
#[derive(Debug, Snafu, Diagnostic)]
pub enum ProcessingError {
    /// Creating the decoder failed.
    ///
    /// Probably something wrong with your system.
    #[snafu(display("error while creating decoder: {source}"))]
    CreateDecoder { source: video_rs::Error },
    #[snafu(transparent)]
    MetadataError { source: MetadataError },
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
}

/// Reading the metadata failed.
///
/// Probably something wrong with the file.
#[derive(Debug, Snafu, Diagnostic)]
#[snafu(display("error while reading metadata: {source}"))]
pub struct MetadataError {
    /// The original Error.
    source: video_rs::Error,
}

impl Video {
    /// Process a video file found at `path` into a [`Video`].
    ///
    /// # Errors
    ///
    /// See the documentation for [`ProcessingError`].
    // TODO: convert tv/limited color range (16-231) to full color range (0-255)
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, ProcessingError> {
        let mut decoder = Decoder::new(path.into()).context(CreateDecoderSnafu)?;

        let metadata = get_metadata(&decoder)?;

        #[allow(clippy::cast_possible_truncation)] // this is fine
        #[allow(clippy::cast_sign_loss)] // this should never happen
        let size = (metadata.frame_rate * metadata.duration.as_secs()) as usize;
        let mut frames = Vec::with_capacity(size);

        let now = Instant::now();

        let progress = MultiProgress::new();
        let frame_progress = progress.add(ProgressBar::new(metadata.n_frames));
        let pixel_progress = progress.add(ProgressBar::new(metadata.n_pixels_per_video));

        for (index, result) in decoder.decode_iter().enumerate() {
            // for some reason, the loop will simply stall when it reaches the last frame,
            // even if you `Iterator::fuse` it, so this makes it end at... the end
            if index as u64 >= metadata.n_frames {
                break;
            }

            // printing often can bottleneck, this is for weaker cpus
            if index % 9 == 0 {
                frame_progress.inc(9);
            }

            let (time, frame) = result.context(DecodeStreamSnafu)?;
            let mut pixels = Vec::with_capacity(metadata.n_pixels);

            for (pixel_index, pixel) in frame.exact_chunks(BYTES_PER_PIXEL).into_iter().enumerate()
            {
                // printing very often will bottleneck, e.g. triple the ttc for a 240p video (5->15)
                if pixel_index % 999 == 0 {
                    pixel_progress.inc(999);
                }

                let pixel = pixel.as_slice().context(ArrayToSliceSnafu)?;
                // indexing is safe because `exact_chunks` gives us exactly 3
                // elements and will skip over any remainder that doesn't fit
                pixels.push(
                    Pixel::builder()
                        .red(pixel[0])
                        .green(pixel[1])
                        .blue(pixel[2])
                        .build(),
                );
            }

            let frame = Frame::builder().time(time).pixels(pixels).build();
            frames.push(frame);
        }

        frame_progress.finish();
        pixel_progress.finish();

        debug!("finished in: {:.2}s", now.elapsed().as_secs_f32());

        let dimensions = Dimensions::builder()
            .width(metadata.width)
            .height(metadata.height)
            .build();
        let video = Self::builder()
            .duration(metadata.duration)
            .frames(frames)
            .dimensions(dimensions)
            .build();
        Ok(video)
    }

    /// Return a slice of all frames that the video consists of.
    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Return the amount of frames in the video.
    #[must_use]
    pub const fn n_frames(&self) -> usize {
        self.frames.len()
    }

    /// Return the amount of pixels in one frame of the video.
    #[must_use]
    pub const fn n_pixels_per_frame(&self) -> usize {
        self.dimensions.width * self.dimensions.height
    }

    /// Return the amount of pixels in the video.
    #[must_use]
    pub const fn n_pixels(&self) -> usize {
        self.n_frames() * self.dimensions.width * self.dimensions.height
    }

    /// Return the duration of the video.
    #[must_use]
    pub const fn duration(&self) -> Time {
        self.duration
    }
}

impl Frame {
    /// Return a slice of all pixels that the frame consists of.
    #[must_use]
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }

    /// Return the amount of pixels in the frame.
    #[must_use]
    pub const fn n_pixels(&self) -> usize {
        self.pixels.len()
    }

    /// Return the time the frame is displayed for.
    #[must_use]
    pub const fn time(&self) -> Time {
        self.time
    }
}

impl Pixel {
    #[must_use]
    pub const fn red(&self) -> u8 {
        self.red
    }

    #[must_use]
    pub const fn green(&self) -> u8 {
        self.green
    }

    #[must_use]
    pub const fn blue(&self) -> u8 {
        self.blue
    }
}

/// Get and return some useful metadata from a video's decoder.
fn get_metadata(decoder: &Decoder) -> Result<Metadata, MetadataError> {
    let duration = decoder.duration().context(MetadataSnafu)?;
    let frame_rate = decoder.frame_rate();
    let n_frames = decoder.frames().context(MetadataSnafu)?;
    let (width, height) = decoder.size();
    let (width, height) = (width as usize, height as usize);
    let n_pixels = width * height;
    let n_pixels_per_video = n_pixels as u64 * n_frames;

    debug!("duration: {:.2}s", duration.as_secs());
    debug!("frame rate: {frame_rate}");
    debug!("total frames: {n_frames}");
    debug!("total pixels: {n_pixels_per_video}");
    debug!("dimensions: {width}x{height}");
    debug!("pixels per frame: {n_pixels}");

    Ok(Metadata {
        duration,
        frame_rate,
        n_frames,
        width,
        height,
        n_pixels,
        n_pixels_per_video,
    })
}
