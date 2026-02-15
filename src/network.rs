//! The client struct responsible for sending pixels to the server.

use std::{
    env::{VarError, var},
    time::Duration,
};

use futures::future::try_join_all;
use snafu::{ResultExt, Snafu};
use tokio::{
    io::{AsyncWriteExt, BufWriter},
    net::TcpStream,
    time::{MissedTickBehavior, interval},
};

use crate::{
    frame::{BINARY_COMMAND_LENGTH, CanvasSizeError, Dimensions, Frame},
    video::Video,
};

/// A wrapper around one or many TCP streams.
pub struct Network {
    /// The TCP stream(s).
    streams: Vec<BufWriter<TcpStream>>,
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
    ///
    /// # Panics
    ///
    /// Panics if setting `TCP_NODELAY` fails, which it should never do.
    pub async fn new() -> Result<Self, NetworkError> {
        let addr = var("PIXELFLUT_ADDRESS").context(NoAddressSetSnafu)?;
        let n_streams = var("NUMBER_OF_STREAMS")
            .unwrap_or_else(|_| "1".to_string())
            .parse()
            .unwrap_or(1);
        let mut stream = TcpStream::connect(&addr)
            .await
            .with_context(|_| TcpConnectSnafu { addr: addr.clone() })?;
        stream
            .set_nodelay(true)
            .expect("setting nodelay should never fail");
        let canvas = Dimensions::try_from_stream(&mut stream).await?;
        let mut streams = vec![BufWriter::new(stream)];
        for _ in 0..n_streams - 1 {
            let stream = TcpStream::connect(&addr)
                .await
                .with_context(|_| TcpConnectSnafu { addr: addr.clone() })?;
            stream
                .set_nodelay(true)
                .expect("setting nodelay should never fail");
            streams.push(BufWriter::new(stream));
        }

        let previous_frame = None;
        let command_buffer = Vec::new();

        Ok(Self {
            streams,
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
    /// Returns an error if writing to the TCP connection(s) fail(s).
    pub async fn send_video(&mut self, mut video: Video) -> Result<(), NetworkError> {
        let frame_duration = Duration::from_secs_f32(1.0 / video.frame_rate());

        let mut ticker = interval(frame_duration);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        while let Some(frame) = video.next_frame().await {
            // ticker.tick().await;
            self.send_frame(&frame).await?;
            self.previous_frame = Some(frame);
        }
        self.previous_frame = None;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if writing to a TCP stream failed.
    pub async fn send_frame(&mut self, frame: &Frame) -> Result<(), NetworkError> {
        frame.fill_command_buffer(
            &mut self.command_buffer,
            self.previous_frame.as_ref(),
            &self.canvas,
        );

        if self.command_buffer.is_empty() {
            return Ok(());
        }

        let total_bytes = self.command_buffer.len();
        let total_cmds = total_bytes / BINARY_COMMAND_LENGTH;
        let cmds_per_stream = total_cmds.div_ceil(self.streams.len());
        let chunk_size = cmds_per_stream * BINARY_COMMAND_LENGTH;

        let command_buffers = self.command_buffer.chunks(chunk_size);

        let addr = self.addr.clone();

        let futures = self
            .streams
            .iter_mut()
            .zip(command_buffers)
            .map(|(stream, chunk)| {
                let addr = addr.clone();
                async move {
                    stream
                        .write_all(chunk)
                        .await
                        .context(TcpWriteSnafu { addr: addr.clone() })?;
                    stream.flush().await.context(TcpFlushSnafu { addr })?; // TODO: remove?
                    Ok::<(), NetworkError>(())
                }
            });

        try_join_all(futures).await?;
        Ok(())
    }
}
