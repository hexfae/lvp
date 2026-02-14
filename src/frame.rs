//! The frame struct representing a single frame of a video.

use snafu::{ResultExt, Snafu};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// A single frame of a video.
#[derive(Debug)]
pub struct Frame {
    /// The pixels of the frame.
    pixels: Vec<Pixel>,
}

/// A single pixel of a frame.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Pixel {
    /// The red component of the pixel.
    r: u8,
    /// The green component of the pixel.
    g: u8,
    /// The blue component of the pixel.
    b: u8,
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

/// The binary command used to send a pixel to the server.
const PIXEL_BINARY_COMMAND: &[u8; 2] = b"PB";

/// The maximum alpha value for a pixel.
const OPAQUE_ALPHA: u8 = 255;

impl Frame {
    /// Converts the frame to a
    /// [pixelpwnr-server](https://github.com/timvisee/pixelpwnr-server) binary
    /// PX command.
    pub fn fill_command_buffer(
        &self,
        command_buffer: &mut Vec<u8>,
        previous_frame: Option<&Self>,
        video: &Dimensions,
        canvas: &Dimensions,
    ) {
        command_buffer.clear();

        let start_x = canvas.width().saturating_sub(video.width());
        let start_y = canvas.height().saturating_sub(video.height());

        let mut current_x = 0;
        let mut current_y = 0;

        for (index, pixel) in self.pixels.iter().enumerate() {
            if current_x >= video.width() {
                current_x = 0;
                current_y += 1;
            }

            if previous_frame.is_some_and(|previous| previous.pixels[index] == *pixel) {
                current_x += 1;
                continue;
            }

            let final_x = start_x + current_x;
            let final_y = start_y + current_y;

            command_buffer.extend(PIXEL_BINARY_COMMAND);
            command_buffer.extend(&final_x.to_le_bytes()[0..2]);
            command_buffer.extend(&final_y.to_le_bytes()[0..2]);
            command_buffer.extend(pixel.as_rgba_bytes());
            current_x += 1;
        }
    }
}

impl Pixel {
    /// Create a new pixel with the given red, green, and blue components.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Converts the pixel to its RGBA byte representation.
    const fn as_rgba_bytes(self) -> [u8; 4] {
        [self.r, self.g, self.b, OPAQUE_ALPHA]
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

impl FromIterator<Pixel> for Frame {
    fn from_iter<T: IntoIterator<Item = Pixel>>(iter: T) -> Self {
        Self {
            pixels: iter.into_iter().collect(),
        }
    }
}
