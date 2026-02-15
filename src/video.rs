//! The video struct containing the bytes of the video.

use snafu::{ResultExt, Snafu};
use std::{
    env::{VarError, var},
    io,
    num::ParseIntError,
};
use tokio::{
    sync::mpsc::{Receiver, channel},
    task::spawn_blocking,
};
use video_rs::{DecoderBuilder, Resize, Url};

use crate::frame::Frame;

/// A decoded video.
pub struct Video {
    /// The frame rate of the video.
    frame_rate: f32,
    /// The receiver that receives the decoded frames.
    receiver: Receiver<Frame>,
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
    /// Builds a decoder for a video from a URL
    ///
    /// # Errors
    ///
    /// Returns an error if the `LVP_MAX_WIDTH` or `LVP_MAX_HEIGHT` environment variables
    /// aren't set or fail to parse, or if decoding the video fails.
    ///
    /// # Panics
    ///
    /// Panics on a Tokio join error (I don't know when these can happen).
    pub async fn from_url(url: Url) -> Result<Self, ReadVideoError> {
        let width = var("LVP_MAX_WIDTH")
            .context(NoWidthSnafu)?
            .parse()
            .context(InvalidWidthSnafu)?;
        let height = var("LVP_MAX_HEIGHT")
            .context(NoHeightSnafu)?
            .parse()
            .context(InvalidHeightSnafu)?;

        let (frame_rate, mut decoder) = spawn_blocking(move || {
            let decoder = DecoderBuilder::new(&url)
                .with_resize(Resize::FitEven(width, height))
                .build()
                .map_err(|why| ReadVideoError::CreateDecoder {
                    url: url.to_string(),
                    source: why,
                })?;
            Ok::<_, ReadVideoError>((decoder.frame_rate(), decoder))
        })
        .await
        .expect("tokio join error")?;

        let (sender, receiver) = channel(5);

        std::thread::spawn(move || {
            loop {
                let Ok((_, frame)) = decoder.decode() else {
                    break;
                };
                let data = frame.flatten().to_vec();
                let (width, height) = decoder.size_out();
                let frame = Frame {
                    data,
                    width,
                    height,
                };

                if let Err(why) = sender.blocking_send(frame) {
                    eprintln!("error while sending frame: {why}");
                    break;
                }
            }
        });
        Ok(Self {
            frame_rate,
            receiver,
        })
    }

    /// The next frame of the video, if there is a next.
    pub async fn next_frame(&mut self) -> Option<Frame> {
        self.receiver.recv().await
    }

    /// The frame rate of the video.
    #[must_use]
    pub const fn frame_rate(&self) -> f32 {
        self.frame_rate
    }
}
