//! The client struct responsible for sending pixels to the server.

use std::{
    env::{VarError, var},
    time::Duration,
};

use snafu::{ResultExt, Snafu};
use tokio::{
    io::{AsyncWriteExt, BufWriter},
    net::TcpStream,
    time::{MissedTickBehavior, interval},
};

use crate::{
    frame::{CanvasSizeError, Dimensions, Frame},
    video::Video,
};

/// A wrapper around a TCP stream.
pub struct Network {
    /// The TCP stream.
    stream: BufWriter<TcpStream>,
    /// The pixelflut server address.
    addr: String,
    /// The server's canvas' dimensions.
    canvas: Dimensions,
    /// The previous frame sent to the server, used for caching.
    previous_frame: Option<Frame>,
    /// The command buffer used to build the command.
    command_buffer: Vec<u8>,
}

/// A network-related error occured.
#[derive(Debug, Snafu)]
pub enum NetworkError {
    /// The `PIXELFLUT_ADDRESS` environment variable is not set.
    #[snafu(display("PIXELFLUT_ADDRESS environment variable is not set"))]
    NoAddressSet {
        /// The source of the error.
        source: VarError,
    },
    /// Failed to open TCP connection to the specified server.
    #[snafu(display("failed to open TCP connection to {addr}"))]
    TcpConnectError {
        /// The address of the server.
        addr: String,
        /// The source of the error.
        source: std::io::Error,
    },
    /// Failed to write to TCP stream on the specified server.
    #[snafu(display("failed to write to TCP stream on {addr}"))]
    TcpWriteError {
        /// The address of the server.
        addr: String,
        /// The source of the error.
        source: std::io::Error,
    },
    /// Failed to flush TCP stream on the specified server.
    #[snafu(display("failed to flush TCP stream on {addr}"))]
    TcpFlushError {
        /// The address of the server.
        addr: String,
        /// The source of the error.
        source: std::io::Error,
    },
    /// Failed to get the server's canvas size.
    #[snafu(transparent)]
    TcpSizeError {
        /// The source of the error.
        source: CanvasSizeError,
    },
}

impl Network {
    /// Creates a TCP connection to the pixelflut server on the address
    /// specified in the `PIXELFLUT_ADDRESS` environment variable.
    ///
    /// # Errors
    ///
    /// Returns an error if the `PIXELFLUT_ADDRESS` environment variable is
    /// not set or if the TCP connection fails.
    pub async fn new() -> Result<Self, NetworkError> {
        let addr = var("PIXELFLUT_ADDRESS").context(NoAddressSetSnafu)?;
        let mut stream = TcpStream::connect(&addr)
            .await
            .with_context(|_| TcpConnectSnafu { addr: addr.clone() })?;

        let canvas = Dimensions::try_from_stream(&mut stream).await?;

        let previous_frame = None;
        let command_buffer = Vec::new();

        Ok(Self {
            stream: BufWriter::new(stream),
            addr,
            canvas,
            previous_frame,
            command_buffer,
        })
    }

    /// Sends a video to the pixelflut server.
    ///
    /// # Errors
    ///
    /// Returns an error if writingidth  to the TCP connection fails.
    pub async fn send_video(&mut self, video: Video) -> Result<(), NetworkError> {
        let dimensions = video.dimensions();
        let frame_duration = Duration::from_secs_f32(1.0 / video.frame_rate());

        let mut ticker = interval(frame_duration);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        for frame in video {
            ticker.tick().await;
            self.send_frame(&frame, &dimensions).await?;
            self.previous_frame = Some(frame);
        }
        self.previous_frame = None;
        Ok(())
    }

    /// Sends a frame to the pixelflut server.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the TCP connection fails.
    pub async fn send_frame(
        &mut self,
        frame: &Frame,
        dimensions: &Dimensions,
    ) -> Result<(), NetworkError> {
        frame.fill_command_buffer(
            &mut self.command_buffer,
            self.previous_frame.as_ref(),
            dimensions,
            &self.canvas,
        );
        self.stream
            .write_all(&self.command_buffer)
            .await
            .with_context(|_| TcpWriteSnafu {
                addr: self.addr.clone(),
            })?;
        self.stream.flush().await.with_context(|_| TcpFlushSnafu {
            addr: self.addr.clone(),
        })
    }
}
