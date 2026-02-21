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

/// The length of a pixel in bytes (RGB).
pub const PIXEL_BYTE_LENGTH: usize = 3;

/// The maximum alpha value for a pixel.
const OPAQUE: u8 = 255;

impl Frame {
    #[must_use]
    /// Create a new empty frame with enough capacity for a full image.
    pub fn new_empty(width: u32, height: u32) -> Self {
        Self {
            data: Vec::with_capacity(PIXEL_BYTE_LENGTH * width as usize * height as usize),
            height,
            width,
        }
    }

    /// Converts the frame to a
    /// [pixelpwnr-server](https://github.com/timvisee/pixelpwnr-server) binary
    /// PX command.
    #[expect(clippy::cast_possible_truncation)] // this is the desired behavior
    pub fn fill_command_buffer(
        &self,
        command_buffer: &mut Vec<u8>,
        previous_cache: &mut [u8],
        canvas: &Dimensions,
    ) {
        command_buffer.clear();

        let offset_x = canvas.width() - self.width;
        let offset_y = canvas.height() - self.height;

        for (index, (pixel, cached_pixel)) in self
            .data
            .chunks_exact(PIXEL_BYTE_LENGTH)
            .zip(previous_cache.chunks_exact_mut(PIXEL_BYTE_LENGTH))
            .enumerate()
        {
            // reduce (quantize) color "resolution" to cache more pixels
            let r = pixel[0] & 0xF0;
            let g = pixel[1] & 0xF0;
            let b = pixel[2] & 0xF0;

            if r == cached_pixel[0] && g == cached_pixel[1] && b == cached_pixel[2] {
                continue;
            }

            cached_pixel[0] = r;
            cached_pixel[1] = g;
            cached_pixel[2] = b;

            let x_pos = (index as u32) % self.width;
            let y_pos = (index as u32) / self.width;

            let x_bytes = ((offset_x + x_pos) as u16).to_le_bytes();
            let y_bytes = ((offset_y + y_pos) as u16).to_le_bytes();

            command_buffer.extend_from_slice(&[
                b'P', b'B', x_bytes[0], x_bytes[1], y_bytes[0], y_bytes[1], r, g, b, OPAQUE,
            ]);
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
    /// Constructs dimensions from a given width and height.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

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
