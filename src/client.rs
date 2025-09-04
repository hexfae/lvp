//! The client responsible for sending TCP packets to the pixelflut (pixelpwnr) server.
use crate::{
    ProcessingError,
    video::{STATIC_VIDEO, Video},
};
use rand::rngs::ThreadRng;
use snafu::{ResultExt, Snafu};
use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
    ops::Range,
    path::PathBuf,
    thread::sleep_until,
    time::{Duration, Instant},
};
use tracing::warn;

/// One second. How long the static plays when switching a video.
const ONE_SECOND: Duration = Duration::from_secs(1);

/// The first and second byte of the command, where the operation is placed.
const OPERATION: Range<usize> = 0..2;

/// The pixel binary command as specified by [pixelpwnr](https://github.com/timvisee/pixelpwnr-server).
const PB: &[u8; 2] = b"PB";

/// The third and fourth byte of the command, where the x position is placed.
const X: Range<usize> = 2..4;

/// The fifth and sixth byte of the command, where the y position is placed.
const Y: Range<usize> = 4..6;

/// The seventh to ninth bytes of the command, where the color is placed.
const RGB: Range<usize> = 6..9;

/// The tenth and final byte of the command, where the alpha is placed.
const ALPHA: usize = 9;

/// A fully opaque pixel, alpha 255.
const OPAQUE: u8 = u8::MAX;

/// The width of the resolution of the LUDD TV.
const CANVAS_WIDTH: u16 = 1920;

/// The height of the resolution of the LUDD TV.
const CANVAS_HEIGHT: u16 = 1080;

/// The width of a standard video (takes up 1/3rd of the width, total area 1/9th).
const VIDEO_WIDTH: usize = 640;

/// The height of a standard video (takes up 1/3rd of the height, total area 1/9th).
const VIDEO_HEIGHT: usize = 360;

/// The amount of bytes per pixel in RGB24 (R, G, B);
const BYTES_PER_PIXEL: usize = 3;

/// The amount of bytes per PB command.
const BYTES_PER_COMMAND: usize = 10;

/// The amount of bytes per frame as represented by RGB24.
const PIXEL_BUFFER_SIZE: usize = VIDEO_WIDTH * VIDEO_HEIGHT * BYTES_PER_PIXEL;

/// The amount of bytes per frame as represented by PB commands.
const COMMAND_BUFFER_SIZE: usize = VIDEO_WIDTH * VIDEO_HEIGHT * BYTES_PER_COMMAND;

pub struct Client {
    /// The TCP stream to send frames to.
    stream: TcpStream,
    /// The path to a folder of videos.
    path: PathBuf,
    /// The currently loaded video, to be processed and sent.
    video: Video,
    /// A cache of the previous frame's pixels, to only send changed pixels.
    pixel_cache: Vec<u8>,
    /// The command to be sent, field used for reusing memory allocation.
    command_buffer: Vec<u8>,
}

/// All errors that can occur while sending a video.
#[derive(Debug, Snafu)]
pub enum ClientError {
    /// Creating the connection failed.
    ///
    /// Probably the address was incorrect (or is not hosting pixelflut).
    CreateConnection { source: std::io::Error },
    /// Sending a packet failed.
    ///
    /// Probably an invalid command was sent.
    #[snafu(display("failed to send a packet to the server"))]
    SendPacket { source: std::io::Error },
    /// No video (besides static or maybe the current video) could be found.
    #[snafu(display("no videos found in the selected folder"))]
    NoVideoFound,
    /// Processing the video failed somehow.
    ///
    /// Probably something wrong with the video.
    #[snafu(transparent)]
    ProcessingError { source: ProcessingError },
}

impl Client {
    /// Create a new [`Client`].
    ///
    /// # Errors
    ///
    /// Returns an error if a video could not be loaded (see [`Video::from_directory`]
    /// or a TCP stream could not be created.
    pub fn new(
        rng: &mut ThreadRng,
        path: impl Into<PathBuf>,
        address: impl ToSocketAddrs,
    ) -> Result<Self, ClientError> {
        let path = path.into();
        let video = Video::from_directory(rng, &path, Some(STATIC_VIDEO.into()))?;
        let stream = TcpStream::connect(address).context(CreateConnectionSnafu)?;
        stream.set_nodelay(true).context(CreateConnectionSnafu)?;

        let pixel_cache = vec![0; PIXEL_BUFFER_SIZE];
        let command_buffer = vec![0; COMMAND_BUFFER_SIZE];

        Ok(Self {
            stream,
            path,
            video,
            pixel_cache,
            command_buffer,
        })
    }

    /// Play static for one second then switch the currently processing video.
    ///
    /// Scans the directory every time this is called, to support "hot-reloading" videos to be played.
    ///
    /// # Errors
    ///
    /// Returns an error if no video was found or if a [`ProcessingError`] occured while processing it.
    pub fn switch_video(&mut self, rng: &mut ThreadRng) -> Result<(), ClientError> {
        let old = self.video.name().to_owned();
        self.pixel_cache.fill(0);
        self.video = Video::from_directory(rng, &self.path, None)?;
        self.send(ONE_SECOND)?;
        self.video = Video::from_directory(rng, &self.path, Some(old))?;
        Ok(())
    }

    /// Send TCP packets to the set address of the frames of the video.
    ///
    /// # Errors
    ///
    /// Returns an error if a packet failed to send.
    pub fn send(&mut self, play_for: Duration) -> Result<(), ClientError> {
        let video_start = Instant::now();
        let time_between_frames = Duration::from_secs_f32(1.0 / self.video.frame_rate());
        let mut deadline = Instant::now();
        let mut frames_to_drop = 0;

        for result in self.video.by_ref() {
            deadline += time_between_frames;
            if frames_to_drop > 0 {
                frames_to_drop -= 1;
                continue;
            } else if video_start.elapsed() > play_for {
                break;
            }
            self.command_buffer.clear();

            for (index, new_pixel_chunk) in result?.chunks_exact(BYTES_PER_PIXEL).enumerate() {
                let cache_start_index = index * BYTES_PER_PIXEL;
                let cache_end_index = cache_start_index + BYTES_PER_PIXEL;
                let cached_pixel_chunk = &self.pixel_cache[cache_start_index..cache_end_index];

                if new_pixel_chunk != cached_pixel_chunk {
                    let (x, y) = bottom_right_coordinates(index);
                    let mut command = [0u8; BYTES_PER_COMMAND];

                    // https://github.com/timvisee/pixelpwnr-server#the-binary-px-command
                    command[OPERATION].copy_from_slice(PB);
                    command[X].copy_from_slice(&x.to_le_bytes());
                    command[Y].copy_from_slice(&y.to_le_bytes());
                    command[RGB].copy_from_slice(new_pixel_chunk);
                    command[ALPHA] = OPAQUE;

                    self.command_buffer.extend_from_slice(&command);

                    self.pixel_cache[cache_start_index..cache_end_index]
                        .copy_from_slice(new_pixel_chunk);
                }
            }

            sleep_until(deadline);
            if !self.command_buffer.is_empty() {
                self.stream
                    .write_all(&self.command_buffer)
                    .context(SendPacketSnafu)?;
            }

            let finished_at = Instant::now();

            #[expect(clippy::cast_possible_truncation)] // will never drop enough frames for this
            #[expect(clippy::cast_sign_loss)] // neither of these can ever be negative
            if finished_at > deadline {
                let late_by = finished_at.duration_since(deadline);
                frames_to_drop =
                    (late_by.as_secs_f64() / time_between_frames.as_secs_f64()).round() as u128;
                if frames_to_drop > 0 {
                    warn!("behind by {late_by:.2?}! dropping {frames_to_drop} frames");
                }
            }
        }
        Ok(())
    }
}

/// Calculates the x and y coordinates to anchor a video to the bottom right corner.
#[expect(clippy::cast_possible_truncation)] // this is fine, ludd's tv is 1920x1080
const fn bottom_right_coordinates(index: usize) -> (u16, u16) {
    let video_x = (index % VIDEO_WIDTH) as u16;
    let video_y = (index / VIDEO_WIDTH) as u16;
    let start_x = CANVAS_WIDTH.saturating_sub(VIDEO_WIDTH as u16);
    let start_y = CANVAS_HEIGHT.saturating_sub(VIDEO_HEIGHT as u16);
    (start_x + video_x, start_y + video_y)
}
