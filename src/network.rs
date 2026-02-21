//! The client struct responsible for sending pixels to the server.

use std::time::Duration;

use futures::future::try_join_all;
use snafu::{ResultExt, Snafu};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    time::{MissedTickBehavior, interval},
};

use crate::{
    CONFIG,
    frame::{BINARY_COMMAND_LENGTH, CanvasSizeError, Dimensions, Frame, PIXEL_BYTE_LENGTH},
    video::Video,
};

/// An "empty" pixel in the cache's eyes.
///
/// This is used for the first frame of a video. There, the "previous frame"
/// will be an entirely white (R255, G255, B255) image. Due to quantization,
/// no pixel can ever be 255 (closest it can be is 250). Thus, the first frame
/// will always be fully painted, without needing to put an Option around the
/// previous frame.
const EMPTY_PIXEL: u8 = 255;

/// A wrapper around one or many TCP streams.
pub struct Network {
    /// The TCP stream(s).
    streams: Vec<TcpStream>,
    /// The server's canvas' dimensions.
    canvas: Dimensions,
    /// The previous frame sent to the server, used for caching.
    previous_frame: Vec<u8>,
    /// The command buffer used to build the command.
    command_buffer: Vec<u8>,
}

/// A network-related error occured.
#[derive(Debug, Snafu)]
pub enum NetworkError {
    /// Failed to open TCP connection to the specified server.
    #[snafu(display("failed to open TCP connection to the server"))]
    Connect {
        /// The source of the error.
        source: std::io::Error,
    },
    /// Failed to write to TCP stream on the specified server.
    #[snafu(display("failed to write to a TCP stream"))]
    Write {
        /// The source of the error.
        source: std::io::Error,
    },
    /// Failed to get the server's canvas size.
    #[snafu(transparent)]
    Size {
        /// The source of the error.
        source: CanvasSizeError,
    },
}

impl Network {
    /// Creates a TCP connection to the configured pixelflut server.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP connection fails.
    ///
    /// # Panics
    ///
    /// Panics if setting `TCP_NODELAY` fails, which it should never do.
    pub async fn new() -> Result<Self, NetworkError> {
        let address = &CONFIG.pixelflut_address;
        let mut stream = TcpStream::connect(address).await.context(ConnectSnafu)?;
        stream.set_nodelay(true).expect("set_nodelay failed");
        let canvas = if let (Some(width), Some(height)) =
            (CONFIG.pixelflut_width, CONFIG.pixelflut_height)
        {
            Dimensions::new(width, height)
        } else {
            Dimensions::try_from_stream(&mut stream).await?
        };
        let mut streams = vec![stream];
        for _ in 0..CONFIG.n_streams - 1 {
            let stream = TcpStream::connect(address).await.context(ConnectSnafu)?;
            stream.set_nodelay(true).expect("set_nodelay failed");
            streams.push(stream);
        }

        let cache_capacity = PIXEL_BYTE_LENGTH * canvas.width() as usize * canvas.height() as usize;
        let command_capacity =
            BINARY_COMMAND_LENGTH * canvas.width() as usize * canvas.height() as usize;
        let previous_frame = vec![EMPTY_PIXEL; cache_capacity];
        let command_buffer = Vec::with_capacity(command_capacity);

        Ok(Self {
            streams,
            canvas,
            previous_frame,
            command_buffer,
        })
    }

    /// Sends a video to the pixelflut server.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the TCP connection(s) fail(s).
    pub async fn send_video(&mut self, mut video: Video) -> Result<(), NetworkError> {
        let frame_duration = Duration::from_secs_f32(1.0 / video.frame_rate());

        let mut ticker = interval(frame_duration);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        while let Some(frame) = video.next_frame().await {
            ticker.tick().await;
            self.send_frame(&frame).await?;
            video.recycle(frame).await;
        }
        self.previous_frame.fill(EMPTY_PIXEL);
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if writing to a TCP stream failed.
    pub async fn send_frame(&mut self, frame: &Frame) -> Result<(), NetworkError> {
        frame.fill_command_buffer(
            &mut self.command_buffer,
            &mut self.previous_frame,
            &self.canvas,
        );

        if self.command_buffer.is_empty() {
            return Ok(());
        }

        let chunks = self.command_buffer.len().div_ceil(self.streams.len());
        let command_buffers = self.command_buffer.chunks(chunks);

        let futures = self
            .streams
            .iter_mut()
            .zip(command_buffers)
            .map(|(stream, chunk)| async { stream.write_all(chunk).await.context(WriteSnafu) });

        try_join_all(futures).await?;
        Ok(())
    }
}
