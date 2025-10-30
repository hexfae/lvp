//! The frame struct representing a single frame of a video.

use ndarray::{ArrayBase, Dim, ViewRepr};

/// A single frame of a video.
#[derive(Debug)]
pub struct Frame {
    /// The pixels of the frame.
    pixels: Vec<Pixel>,
}

/// A single pixel of a frame.
#[derive(Debug)]
pub struct Pixel(Red, Green, Blue);

/// A single red component of a pixel.
#[derive(Debug)]
struct Red(u8);

/// A single green component of a pixel.
#[derive(Debug)]
struct Green(u8);

/// A single blue component of a pixel.
#[derive(Debug)]
struct Blue(u8);

impl Frame {
    /// Converts the frame to a
    /// [pixelpwnr-server](https://github.com/timvisee/pixelpwnr-server) binary
    /// PX command.
    // videos are not expected to be larger than 65535x65535 pixels
    #[expect(clippy::cast_possible_truncation)]
    pub fn to_command(&self) -> Vec<u8> {
        let mut command = vec![];
        for (index, pixel) in self.pixels.iter().enumerate() {
            let x = ((index % 720) as u16).to_le_bytes();
            let y = ((index / 720) as u16).to_le_bytes();
            command.extend(b"PB");
            command.extend(&x);
            command.extend(&y);
            command.extend(&[pixel.0.0, pixel.1.0, pixel.2.0, u8::MAX]);
        }
        command
    }
}

impl Pixel {
    /// Create a new pixel with the given red, green, and blue components.
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self(Red(red), Green(green), Blue(blue))
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
