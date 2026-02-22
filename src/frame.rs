//! The frame struct representing a single frame of a video.

use bytes::BufMut;
use rayon::prelude::*;
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
#[derive(Clone)]
pub struct Dimensions {
    /// The width of the video, in pixels.
    width: u32,
    /// The height of the video, in pixels.
    height: u32,
}

/// "PB" in bytes.
const PB: u16 = 0x4250;

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
    pub fn fill_command_buffers(
        &self,
        command_buffers: &mut [Vec<u8>],
        previous_cache: &mut [u8],
        canvas: &Dimensions,
    ) {
        let offset_x = canvas.width() - self.width;
        let offset_y = canvas.height() - self.height;

        let width = self.width as usize;
        let height = self.height as usize;
        let n_streams = command_buffers.len();

        let rows_per_stream = height.div_ceil(n_streams);
        let bytes_per_chunk = rows_per_stream * width * PIXEL_BYTE_LENGTH;

        command_buffers
            .par_iter_mut()
            .zip(self.data.par_chunks(bytes_per_chunk))
            .zip(previous_cache.par_chunks_mut(bytes_per_chunk))
            .enumerate()
            .for_each(|(chunk_index, ((buffer, pixel_chunk), cache_chunk))| {
                buffer.clear();

                let start_y = chunk_index * rows_per_stream;

                for (row_index, (pixel_row, cache_row)) in pixel_chunk
                    .chunks_exact(width * PIXEL_BYTE_LENGTH)
                    .zip(cache_chunk.chunks_exact_mut(width * PIXEL_BYTE_LENGTH))
                    .enumerate()
                {
                    let current_y = (offset_y as usize + start_y + row_index) as u16;

                    for (column_index, (pixel, cached_pixel)) in pixel_row
                        .chunks_exact(PIXEL_BYTE_LENGTH)
                        .zip(cache_row.chunks_exact_mut(PIXEL_BYTE_LENGTH))
                        .enumerate()
                    {
                        let r = pixel[0];
                        let g = pixel[1];
                        let b = pixel[2];

                        let diff = u16::from(r.abs_diff(cached_pixel[0]))
                            + u16::from(g.abs_diff(cached_pixel[1]))
                            + u16::from(g.abs_diff(cached_pixel[2]));

                        if diff < 15 {
                            continue;
                        }

                        cached_pixel[0] = r;
                        cached_pixel[1] = g;
                        cached_pixel[2] = b;

                        let current_x = (offset_x as usize + column_index) as u16;

                        buffer.put_u16_le(PB);
                        buffer.put_u16_le(current_x);
                        buffer.put_u16_le(current_y);
                        buffer.put_u8(r);
                        buffer.put_u8(g);
                        buffer.put_u8(b);
                        buffer.put_u8(OPAQUE);
                    }
                }
            });
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
