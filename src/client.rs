//! The client responsible for sending TCP packets to the pixelflut (pixelpwnr) server.

use crate::{
    ProcessingError,
    video::{STATIC_VIDEO, Video},
};
use miette::Diagnostic;
use snafu::{ResultExt, Snafu};
use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
    ops::Range,
    path::PathBuf,
    thread::sleep,
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

/// The amount of bytes per full frame in PB commands.
const BYTES_PER_FRAME: usize = VIDEO_WIDTH * VIDEO_HEIGHT * BYTES_PER_COMMAND;

pub struct Client {
    /// The TCP stream to send frames to.
    stream: TcpStream,
    /// The path to a folder of videos.
    path: PathBuf,
    /// The currently lodaed video, to be processed and sent.
    video: Video,
    /// The command buffer, to be sent and reused.
    commands: Vec<u8>,
}

/// All errors that can occur while sending a video.
#[derive(Debug, Snafu, Diagnostic)]
pub enum ClientError {
    /// Creating the connection failed.
    ///
    /// Probably the address was incorrect (or is not hosting pixelflut).
    CreateConnection { source: std::io::Error },
    /// Sending the packet failed.
    ///
    /// Probably an invalid command was sent.
    SendPacket { source: std::io::Error },
    #[snafu(display("no videos found in the selected folder"))]
    NoVideoFound,
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
    pub fn new(path: impl Into<PathBuf>, address: impl ToSocketAddrs) -> Result<Self, ClientError> {
        let path = path.into();
        let video = Video::from_directory(&path, Some(STATIC_VIDEO.into()))?;
        let stream = TcpStream::connect(address).context(CreateConnectionSnafu)?;
        stream.set_nodelay(true).context(CreateConnectionSnafu)?;

        let mut commands = vec![0; BYTES_PER_FRAME];
        for (index, command) in commands.chunks_exact_mut(BYTES_PER_COMMAND).enumerate() {
            let (x, y) = bottom_right_coordinates(index);
            command[OPERATION].copy_from_slice(PB);
            command[X].copy_from_slice(&x.to_le_bytes());
            command[Y].copy_from_slice(&y.to_le_bytes());
            // the rgb bytes are black from the initialization, to be replaced in send() later
            command[ALPHA] = OPAQUE;
        }

        Ok(Self {
            stream,
            path,
            video,
            commands,
        })
    }

    /// Play static for one second then switch the currently processing video.
    ///
    /// Scans the directory every time this is called, to support "hot-reloading" videos to be played.
    ///
    /// # Errors
    ///
    /// Returns an error if no video was found or if a [`ProcessingError`] occured while processing it.
    pub fn switch_video(&mut self) -> Result<(), ClientError> {
        let old = self.video.name().to_owned();
        self.video = Video::from_directory(&self.path, None)?;
        self.send(ONE_SECOND)?;
        self.video = Video::from_directory(&self.path, Some(old))?;
        Ok(())
    }

    /// Send TCP packets to the set address of the frames of the video.
    ///
    /// # Errors
    ///
    /// Returns an error if a packet failed to send.
    pub fn send(&mut self, play_for: Duration) -> Result<(), ClientError> {
        let frame_rate = self.video.frame_rate();
        let desired = Duration::from_secs_f32(1.0 / frame_rate);
        let start_time = Instant::now();
        let mut next_frame_time = Instant::now();

        for result in self.video.by_ref() {
            let loop_start = Instant::now();
            if start_time.elapsed() > play_for {
                break;
            }
            for (pixels, commands) in result?
                .chunks_exact(BYTES_PER_PIXEL)
                .zip(self.commands.chunks_mut(BYTES_PER_COMMAND))
            {
                commands[RGB].copy_from_slice(pixels);
            }

            self.stream
                .write_all(&self.commands)
                .context(SendPacketSnafu)?;

            next_frame_time += desired;
            let sleep_time = next_frame_time.saturating_duration_since(Instant::now());

            if sleep_time.is_zero() {
                let took = loop_start.elapsed();
                let over = took.saturating_sub(desired);
                warn!(
                    "frame took {took:.2?} to render and send, which is {over:.2?} over the desired {desired:.2?}",
                );
                next_frame_time = Instant::now(); // we're behind, display the next frame asap
            } else {
                sleep(sleep_time);
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
