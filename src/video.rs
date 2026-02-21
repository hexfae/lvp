//! The video struct containing the bytes of the video.

use snafu::{ResultExt, Snafu};
use std::io;
use tokio::{
    sync::mpsc::{Receiver, Sender, channel, error::SendError},
    task::spawn_blocking,
};
use tracing::warn;
use video_rs::{DecoderBuilder, Resize, Url};

use crate::{CONFIG, frame::Frame};

/// A decoded video.
pub struct Video {
    /// The frame rate of the video.
    frame_rate: f32,
    /// The receiver that receives the decoded frames.
    receiver: Receiver<Frame>,
    /// The sender that sends old frames.
    recycle_sender: Sender<Frame>,
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
    /// An error occurred while creating a decoder from a URL.
    #[snafu(display("Failed to create decoder from {url}"))]
    CreateDecoder {
        /// The URL.
        url: String,
        /// The source of the error.
        source: video_rs::Error,
    },
    /// An error occured while sending a frame.
    #[snafu(display("Failed to send a frame"))]
    SendFrame {
        /// The source of the error.
        source: SendError<Frame>,
    },
}

impl Video {
    /// Builds a decoder for a video from a URL
    ///
    /// # Errors
    ///
    /// Returns an error if decoding the video fails.
    ///
    /// # Panics
    ///
    /// Panics on a Tokio join error (I don't know when these can happen).
    pub async fn from_url(url: Url) -> Result<Self, ReadVideoError> {
        let width = CONFIG.max_width;
        let height = CONFIG.max_height;

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
        let (recycle_sender, mut recycle_receiver) = channel(5);

        let (width, height) = decoder.size_out();
        for _ in 0..5 {
            recycle_sender
                .send(Frame::new_empty(width, height))
                .await
                .context(SendFrameSnafu)?;
        }

        spawn_blocking(move || {
            while let Some(mut frame_buffer) = recycle_receiver.blocking_recv() {
                let Ok((_, frame)) = decoder.decode() else {
                    break;
                };

                let Some(raw_slice) = frame.as_slice() else {
                    break;
                };

                // TODO: make the frame buffers big enough to hold the biggest frame (e.g. 640x540)
                if frame_buffer.data.len() != raw_slice.len() {
                    frame_buffer.data.resize(raw_slice.len(), 0);
                }
                frame_buffer.data.copy_from_slice(raw_slice);
                frame_buffer.width = width;
                frame_buffer.height = height;

                if let Err(why) = sender.blocking_send(frame_buffer).context(SendFrameSnafu) {
                    warn!("{why}");
                    break;
                }
            }
        });
        Ok(Self {
            frame_rate,
            receiver,
            recycle_sender,
        })
    }

    /// Recycle a previously used frame.
    pub async fn recycle(&self, frame: Frame) {
        // error here just means the video is finished, ignore it
        let _ = self.recycle_sender.send(frame).await;
    }

    /// The next frame of the video, if there is a next one.
    pub async fn next_frame(&mut self) -> Option<Frame> {
        self.receiver.recv().await
    }

    /// The frame rate of the video.
    #[must_use]
    pub const fn frame_rate(&self) -> f32 {
        self.frame_rate
    }
}
