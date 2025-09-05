//! The client responsible for sending TCP packets to the pixelflut (pixelpwnr) server.
use crate::{Args, video::Video};
use snafu::{ResultExt, Snafu};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::mpsc::channel,
    time::{MissedTickBehavior, interval, timeout},
};

/// The pixel binary command as specified by [pixelpwnr](https://github.com/timvisee/pixelpwnr-server).
const PB: &[u8; 2] = b"PB";

/// A fully opaque pixel, alpha 255.
const OPAQUE: u8 = u8::MAX;

/// The width of the resolution of the LUDD TV.
const CANVAS_WIDTH: usize = 1920;

/// The height of the resolution of the LUDD TV.
const CANVAS_HEIGHT: usize = 1080;

/// The width of a standard video (takes up 1/3rd of the width, total area 1/9th).
const VIDEO_WIDTH: usize = 640;

/// The height of a standard video (takes up 1/3rd of the height, total area 1/9th).
const VIDEO_HEIGHT: usize = 360;

/// The amount of bytes per pixel in RGB24 (R, G, B);
const BYTES_PER_PIXEL: usize = 3;

/// The (expected) max amount of bytes per PX command.
///
/// 3 for `PX` and a space, 5 for the x position and a space, 5 for the y position
/// and a space, 7 for RR/GG/BB and a newline.
const MAX_BYTES_PER_TEXT_COMMAND: usize = 20;

/// The amount of bytes per PB command (for
/// [pixelpwnr-server](github.com/timvisee/pixelpwnr-server)).
///
/// 2 for `PB`, 2 for the x position, 2 for the y position, 4 for R/G/B/A.
const BYTES_PER_BINARY_COMMAND: usize = 10;

/// The amount of bytes per frame as represented by RGB24.
const PIXEL_BUFFER_SIZE: usize = VIDEO_WIDTH * VIDEO_HEIGHT * BYTES_PER_PIXEL;

/// The amount of bytes per frame as represented by PX commands.
const TEXT_COMMAND_BUFFER_SIZE: usize = VIDEO_WIDTH * VIDEO_HEIGHT * MAX_BYTES_PER_TEXT_COMMAND;

/// The amount of bytes per frame as represented by PB commands.
const BINARY_COMMAND_BUFFER_SIZE: usize = VIDEO_WIDTH * VIDEO_HEIGHT * BYTES_PER_BINARY_COMMAND;

pub struct Client {
    /// The TCP stream to send frames to.
    stream: TcpStream,
    /// The path to a folder of videos.
    path: PathBuf,
    /// Whether to use binary commands, for pixelpwnr servers.
    binary: bool,
    /// A cache of the previous frame's pixels, to only send changed pixels.
    pixel_cache: Vec<u8>,
}

/// All errors that can occur while sending a video.
#[derive(Debug, Snafu)]
pub enum ClientError {
    JoinError {
        source: tokio::task::JoinError,
    },
    /// Creating the connection failed.
    ///
    /// Probably the address was incorrect (or is not hosting pixelflut).
    #[snafu(display("could not create tcp connection to {address}"))]
    CreateConnection {
        source: std::io::Error,
        address: String,
    },
    /// Setting `TCP_NODELAY` on the TCP socket failed.
    #[snafu(display("setting TCP_NODELAY on {address} failed: {source}"))]
    SetNoDelay {
        source: std::io::Error,
        address: String,
    },
    /// Sending a packet failed.
    ///
    /// Probably an invalid command was sent.
    #[snafu(display("failed to send a packet to the server"))]
    SendPacket {
        source: std::io::Error,
    },
}

impl Client {
    /// The path of the directory containing the videos to be played.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create a new [`Client`].
    ///
    /// # Errors
    ///
    /// Returns an error if a video could not be loaded (see [`Video::from_directory`]
    /// or a TCP stream could not be created.
    pub async fn new(args: Args) -> Result<Self, ClientError> {
        let path = args.path.into();
        let binary = args.binary;
        let address = &args.address;
        let stream = TcpStream::connect(address)
            .await
            .context(CreateConnectionSnafu { address })?;
        stream
            .set_nodelay(true)
            .context(CreateConnectionSnafu { address })?;

        let pixel_cache = vec![0; PIXEL_BUFFER_SIZE];

        Ok(Self {
            stream,
            path,
            binary,
            pixel_cache,
        })
    }

    /// Send TCP packets to the set address of the frames of the video.
    ///
    /// # Errors
    ///
    /// Returns an error if a packet failed to send.
    pub async fn send(&mut self, video: Video, play_for: Duration) -> Result<(), ClientError> {
        let (tx, mut rx) = channel::<Vec<u8>>(2);
        let binary = self.binary;
        let mut pixel_cache = std::mem::take(&mut self.pixel_cache);
        let frame_duration = Duration::from_secs_f32(1.0 / video.frame_rate());

        let producer_handle = tokio::spawn(async move {
            let mut command_buffer = if binary {
                Vec::with_capacity(BINARY_COMMAND_BUFFER_SIZE)
            } else {
                Vec::with_capacity(TEXT_COMMAND_BUFFER_SIZE)
            };

            let mut ticker = interval(frame_duration);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            for result in video {
                ticker.tick().await;

                let Ok(frame_pixels) = result else {
                    break;
                };

                for (index, new_pixel_chunk) in
                    frame_pixels.chunks_exact(BYTES_PER_PIXEL).enumerate()
                {
                    let cache_start_index = index * BYTES_PER_PIXEL;
                    let cache_end_index = cache_start_index + BYTES_PER_PIXEL;
                    let cached_pixel_chunk = &pixel_cache[cache_start_index..cache_end_index];

                    if new_pixel_chunk != cached_pixel_chunk {
                        let (x, y) = bottom_right_coordinates(index);

                        #[expect(unused_must_use)]
                        if binary {
                            command_buffer.extend_from_slice(PB);
                            command_buffer.extend_from_slice(&x.to_le_bytes());
                            command_buffer.extend_from_slice(&y.to_le_bytes());
                            command_buffer.extend_from_slice(new_pixel_chunk);
                            command_buffer.push(OPAQUE);
                        } else {
                            let (red, green, blue) =
                                (new_pixel_chunk[0], new_pixel_chunk[1], new_pixel_chunk[2]);
                            writeln!(
                                &mut command_buffer,
                                "PX {x} {y} {red:02X}{green:02X}{blue:02X}"
                            );
                        }

                        pixel_cache[cache_start_index..cache_end_index]
                            .copy_from_slice(new_pixel_chunk);
                    }
                }

                if command_buffer.is_empty() {
                    continue;
                }

                if tx.send(std::mem::take(&mut command_buffer)).await.is_err() {
                    break;
                }
            }
            pixel_cache
        });

        if let Ok(Err(why)) = timeout(play_for, async {
            while let Some(buffer) = rx.recv().await {
                if !buffer.is_empty() {
                    self.stream
                        .write_all(&buffer)
                        .await
                        .context(SendPacketSnafu)?;
                }
            }
            Ok(())
        })
        .await
        {
            return Err(why);
        }

        drop(rx);
        self.pixel_cache = producer_handle.await.context(JoinSnafu)?;
        Ok(())
    }
}

/// Calculates the x and y coordinates to anchor a video to the bottom right corner.
#[expect(clippy::cast_possible_truncation)] // this is fine, ludd's tv is 1920x1080
const fn bottom_right_coordinates(index: usize) -> (u16, u16) {
    let video_x = index % VIDEO_WIDTH;
    let video_y = index / VIDEO_WIDTH;
    let start_x = CANVAS_WIDTH.saturating_sub(VIDEO_WIDTH);
    let start_y = CANVAS_HEIGHT.saturating_sub(VIDEO_HEIGHT);
    ((start_x + video_x) as u16, (start_y + video_y) as u16)
}
