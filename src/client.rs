//! The client responsible for sending TCP packets to the LUDD TV.

use miette::Diagnostic;
use snafu::{ResultExt, Snafu};

use crate::Video;
use std::{fmt::Write, net::TcpStream, thread::sleep_until, time::Instant};

pub struct Client {
    /// The decoded video, to be sent.
    video: Video,
    /// The address hosting a pixelflut server to send to.
    address: String,
}

/// All errors that can occur while sending a video.
#[derive(Debug, Snafu, Diagnostic)]
pub enum NetworkError {
    /// Creating the connection failed.
    ///
    /// Probably the address was incorrect (or is not hosting pixelflut).
    CreateConnection { source: std::io::Error },
    /// Constructing the command failed
    ///
    /// Probably it grew to be too big.
    ConstructCommand { source: std::fmt::Error },
    /// Sending the packet failed.
    ///
    /// Probably an invalid command was sent.
    SendPacket { source: std::io::Error },
}

impl Client {
    #[must_use]
    pub fn new(video: Video, address: impl Into<String>) -> Self {
        let address = address.into();
        Self { video, address }
    }

    /// Send TCP packets to the set address of the frames of the video.
    ///
    /// # Errors
    ///
    /// Returns an error if a TCP connection could not be established or if
    #[allow(clippy::many_single_char_names)] // they're all obvious
    #[allow(clippy::cast_possible_truncation)] // this is fine, most servers are 1920x1080
    pub fn send(&mut self) -> Result<(), NetworkError> {
        let mut stream = TcpStream::connect(&self.address).context(CreateConnectionSnafu)?;
        let width = self.video.width() as usize;
        let height = self.video.height() as usize;
        let frame_rate = self.video.frame_rate();
        let mut display_next_frame = Instant::now();

        while let Some(Ok(frame)) = self.video.next() {
            let commands = frame.chunks_exact(3).enumerate().try_fold(
                String::new(),
                |mut previous, (index, pixel)| {
                    let x = ((index % width) + 1920 - width) as u16;
                    let y = ((index / width) + 1080 - height) as u16;
                    // indexing is safe because `chunks_exact` always gives exactly 3
                    let (mut r, mut g, mut b) = (pixel[0], pixel[1], pixel[2]);
                    if r < 16 || g < 16 || b < 16 {
                        (r, g, b) = (0, 0, 0);
                    }
                    writeln!(&mut previous, "PX {x} {y} {r:02X}{g:02X}{b:02X}")
                        .context(ConstructCommandSnafu)
                        .map(|()| previous)
                },
            )?;
            sleep_until(display_next_frame);
            std::io::Write::write_all(&mut stream, commands.as_bytes()).context(SendPacketSnafu)?;
            display_next_frame += std::time::Duration::from_secs_f32(1.0 / frame_rate);
        }
        Ok(())
    }
}
