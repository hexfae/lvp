//! The frame struct representing a single frame of a video.

use snafu::{ResultExt, Snafu};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// A single frame of a video.
#[derive(Debug)]
pub struct Frame {
    /// The bytes that make up the video.
    pub data: Vec<u8>,
    /// The width of the video.
    pub width: u32,
    /// The height of the video.
    pub height: u32,
}

/// The dimensions of the video, in pixels.
pub struct Dimensions {
    /// The width of the video, in pixels.
    width: u32,
    /// The height of the video, in pixels.
    height: u32,
}

/// Enough bytes for `SIZE xxxx yyyy`.
const SIZE_BUF_SIZE: usize = 14;

/// The SIZE command for asking for the server's dimensions.
const SIZE_COMMAND: &[u8; 5] = b"SIZE\n";

/// The length of a pixelpwnr-server binary PX command in bytes: `PBxxyyrgba`.
pub const BINARY_COMMAND_LENGTH: usize = 10;

/// The maximum alpha value for a pixel.
const OPAQUE: u8 = 255;

impl Frame {
    /// Converts the frame to a
    /// [pixelpwnr-server](https://github.com/timvisee/pixelpwnr-server) binary
    /// PX command.
    pub fn fill_command_buffer(
        &self,
        command_buffer: &mut Vec<u8>,
        previous_frame: Option<&Self>,
        canvas: &Dimensions,
    ) {
        command_buffer.clear();

        let offset_x = canvas.width() - self.width;
        let offset_y = canvas.height() - self.height;

        let mut index = 0;
        for y in 0..self.height {
            let final_y = (offset_y + y).to_le_bytes();

            for x in 0..self.width {
                // reduce color "resolution" to cache more pixels
                let r = (self.data[index] / 10) * 10;
                let g = (self.data[index + 1] / 10) * 10;
                let b = (self.data[index + 2] / 10) * 10;

                if let Some(previous) = previous_frame {
                    let pr = previous.data[index];
                    let pg = previous.data[index + 1];
                    let pb = previous.data[index + 2];
                    if r == pr && g == pg && b == pb {
                        index += 3;
                        continue;
                    }
                }

                let final_x = (offset_x + x).to_le_bytes();

                command_buffer.push(b'P');
                command_buffer.push(b'B');
                command_buffer.push(final_x[0]);
                command_buffer.push(final_x[1]);
                command_buffer.push(final_y[0]);
                command_buffer.push(final_y[1]);
                command_buffer.push(r);
                command_buffer.push(g);
                command_buffer.push(b);
                command_buffer.push(OPAQUE);
                index += 3;
            }
        }
    }
}

/// Requesting the server's canvas size failed.
#[derive(Debug, Snafu)]
pub enum CanvasSizeError {
    /// Writing to the TCP stream failed.
    #[snafu(display("failed to send size command to server"))]
    Write {
        /// The source of the error.
        source: std::io::Error,
    },
    /// Reading from the TCP stream failed.
    #[snafu(display("failed to read size response from server"))]
    Read {
        /// The source of the error.
        source: std::io::Error,
    },
    /// Parsing the response failed.
    #[snafu(display("failed to parse size response"))]
    Parse {
        /// The source of the error.
        source: std::num::ParseIntError,
    },
}

impl Dimensions {
    /// # Errors
    ///
    /// Returns an error if sending the SIZE command failed, reading the response
    /// failed, or parsing the response failed.
    pub async fn try_from_stream(stream: &mut TcpStream) -> Result<Self, CanvasSizeError> {
        stream.write_all(SIZE_COMMAND).await.context(WriteSnafu)?;
        let mut buf = [0; SIZE_BUF_SIZE];
        stream.read_exact(&mut buf).await.context(ReadSnafu)?;

        let width_string = String::from_utf8_lossy(&buf[5..9]);
        let height_string = String::from_utf8_lossy(&buf[10..14]);

        let width = width_string.parse().context(ParseSnafu)?;
        let height = height_string.parse().context(ParseSnafu)?;

        Ok(Self { width, height })
    }

    /// The width of the video.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The height of the video.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
}

impl From<(u32, u32)> for Dimensions {
    fn from((width, height): (u32, u32)) -> Self {
        Self { width, height }
    }
}
