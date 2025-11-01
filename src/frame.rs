//! The frame struct representing a single frame of a video.

use ndarray::{ArrayBase, Dim, ViewRepr};

/// A single frame of a video.
#[derive(Debug)]
pub struct Frame {
    /// The pixels of the frame.
    pixels: Vec<Pixel>,
}

/// A single pixel of a frame.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Pixel {
    r: u8,
    g: u8,
    b: u8,
}

/// The length of a pixelpwnr-server binary PX command in bytes: `PBxxyyrgba`.
const BINARY_COMMAND_LENGTH: usize = 10;

/// The maximum alpha value for a pixel.
const OPAQUE_ALPHA: u8 = 255;

impl Frame {
    /// Converts the frame to a
    /// [pixelpwnr-server](https://github.com/timvisee/pixelpwnr-server) binary
    /// PX command.
    // videos are not expected to be larger than 65535x65535 pixels
    #[expect(clippy::cast_possible_truncation)]
    pub fn fill_command_buffer(
        &self,
        command_buffer: &mut Vec<u8>,
        previous_frame: Option<&Frame>,
        width: u32,
    ) {
        command_buffer.clear();
        command_buffer.reserve(self.pixels.len() * BINARY_COMMAND_LENGTH);
        for (index, pixel) in self.pixels.iter().enumerate() {
            if previous_frame.is_some_and(|previous| previous.pixels[index] == *pixel) {
                continue;
            }

            command_buffer.extend(PIXEL_BINARY_COMMAND);
            command_buffer.extend(coordinates_from(index, width));
            command_buffer.extend(pixel.as_rgba_bytes());
        }
    }
}

const PIXEL_BINARY_COMMAND: [u8; 2] = *b"PB";

const fn coordinates_from(index: usize, width: u32) -> [u8; 4] {
    let index = index as u32;
    let x = ((index % width) as u16).to_le_bytes();
    let y = ((index / width) as u16).to_le_bytes();

    [x[0], x[1], y[0], y[1]]
}

impl Pixel {
    /// Create a new pixel with the given red, green, and blue components.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    const fn as_rgba_bytes(self) -> [u8; 4] {
        [self.r, self.g, self.b, OPAQUE_ALPHA]
    }
}

impl FromIterator<Pixel> for Frame {
    fn from_iter<T: IntoIterator<Item = Pixel>>(iter: T) -> Self {
        Self {
            pixels: iter.into_iter().collect(),
        }
    }
}

impl From<ArrayBase<ViewRepr<&u8>, Dim<[usize; 1]>>> for Pixel {
    fn from(rgb: ArrayBase<ViewRepr<&u8>, Dim<[usize; 1]>>) -> Self {
        Self::new(rgb[0], rgb[1], rgb[2])
    }
}
