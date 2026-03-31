//! The client struct responsible for sending pixels to the server.

use std::{
    mem,
    time::{Duration, Instant},
};

use futures::future::try_join_all;
use snafu::{ResultExt, Snafu};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::watch::{Sender, channel},
    task::{JoinHandle, spawn_blocking},
    time::{MissedTickBehavior, interval, sleep},
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
    workers: Vec<JoinHandle<()>>,
    tx: Sender<Vec<Vec<u8>>>,
    /// The server's canvas' dimensions.
    canvas: Dimensions,
    /// The previous frame sent to the server, used for caching.
    previous_frame: Vec<u8>,
    /// The command buffer used to build the command.
    command_buffers: Vec<Vec<u8>>,
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
    /// Failed to join a tokio task.
    #[snafu(display("Error while joining tokio tasks"))]
    Join {
        /// The source of the error.
        source: tokio::task::JoinError,
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
        let (tx, rx) = channel(vec![vec![]; CONFIG.n_streams]);
        let mut workers = Vec::new();

        for stream_idx in 0..CONFIG.n_streams.max(1) {
            let mut rx = rx.clone();
            let addr = address.clone();

            let worker = tokio::spawn(async move {
                loop {
                    let mut stream = if let Ok(s) = TcpStream::connect(&addr).await {
                        let _ = s.set_nodelay(true);
                        s
                    } else {
                        sleep(Duration::from_millis(500)).await;
                        continue;
                    };
                    loop {
                        if rx.changed().await.is_err() {
                            return;
                        }

                        let buffers = rx.borrow().clone();
                        if let Some(chunk) = buffers.get(stream_idx)
                            && !chunk.is_empty()
                            && stream.write_all(chunk).await.is_err()
                        {
                            break;
                        }
                    }
                }
            });
            workers.push(worker);
        }

        let mut first_stream = TcpStream::connect(address).await.context(ConnectSnafu)?;
        first_stream.set_nodelay(true).context(ConnectSnafu)?;

        let canvas = if let (Some(width), Some(height)) =
            (CONFIG.pixelflut_width, CONFIG.pixelflut_height)
        {
            Dimensions::new(width, height)
        } else {
            Dimensions::try_from_stream(&mut first_stream).await?
        };

        let mut streams = Vec::with_capacity(CONFIG.n_streams.max(1));
        streams.push(first_stream);

        if CONFIG.n_streams > 1 {
            let connect_futures = (1..CONFIG.n_streams).map(|_| async {
                let stream = TcpStream::connect(address).await.context(ConnectSnafu)?;
                stream.set_nodelay(true).context(ConnectSnafu)?;
                Ok::<_, NetworkError>(stream)
            });

            streams.extend(try_join_all(connect_futures).await?);
        }

        let width = CONFIG.max_width as usize;
        let height = CONFIG.max_height as usize;
        let n_streams = streams.len();

        let cache_capacity = PIXEL_BYTE_LENGTH * width * height;
        let command_capacity = BINARY_COMMAND_LENGTH * width * height / n_streams;

        let previous_frame = vec![EMPTY_PIXEL; cache_capacity];
        let command_buffers = vec![Vec::with_capacity(command_capacity); n_streams];

        Ok(Self {
            workers,
            tx,
            canvas,
            previous_frame,
            command_buffers,
        })
    }

    /// Sends a video to the pixelflut server.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the TCP connection(s) fail(s).
    pub async fn send_video(&mut self, mut video: Video) -> Result<(), NetworkError> {
        let frame_duration = Duration::from_secs_f32(1.0 / video.frame_rate());
        let mut last_refresh = Instant::now();

        let mut ticker = interval(frame_duration);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        while let Some(frame) = video.next_frame().await {
            ticker.tick().await;
            if last_refresh.elapsed() >= Duration::from_secs(2) {
                self.previous_frame.fill(EMPTY_PIXEL);
                last_refresh = Instant::now();
            }
            let used_frame = self.send_frame(frame).await?;
            video.recycle(used_frame).await;
        }
        self.previous_frame.fill(EMPTY_PIXEL);
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if writing to a TCP stream failed.
    pub async fn send_frame(&mut self, frame: Frame) -> Result<Frame, NetworkError> {
        let mut buffers = mem::take(&mut self.command_buffers);
        let mut cache = mem::take(&mut self.previous_frame);
        let canvas = self.canvas.clone();

        let (filled_buffers, returned_cache, returned_frame) = spawn_blocking(move || {
            frame.fill_command_buffers(&mut buffers, &mut cache, &canvas);
            (buffers, cache, frame)
        })
        .await
        .context(JoinSnafu)?;

        self.command_buffers = filled_buffers;
        self.previous_frame = returned_cache;

        let _ = self.tx.send(self.command_buffers.clone());

        Ok(returned_frame)
    }
}
