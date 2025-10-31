//! The frame struct representing a single frame of a video.

use ndarray::{ArrayBase, Dim, ViewRepr};

/// A single frame of a video.
#[derive(Debug)]
pub struct Frame {
    /// The pixels of the frame.
    pixels: Vec<Pixel>,
}

/// A single pixel of a frame.
#[derive(Debug, PartialEq)]
pub struct Pixel(Red, Green, Blue);

/// A single red component of a pixel.
#[derive(Debug, PartialEq)]
struct Red(u8);

/// A single green component of a pixel.
#[derive(Debug, PartialEq)]
struct Green(u8);

/// A single blue component of a pixel.
#[derive(Debug, PartialEq)]
struct Blue(u8);

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
        command_buffer.reserve(self.pixels.len());
        for (index, pixel) in self.pixels.iter().enumerate() {
            if previous_frame.is_some_and(|previous| previous.pixels[index] == *pixel) {
                continue;
            }
            let part = to_binary_command(index, width, pixel);
            command_buffer.extend(part);
        }
    }
}

fn to_binary_command(index: usize, width: u32, pixel: &Pixel) -> Vec<u8> {
    let mut command = Vec::with_capacity(10);
    let index = index as u32;
    // the reason why we cast the products as u16 is because
    // pixelpwnr-server's binary PX command expects u16 coordinates
    let x = (index % width) as u16;
    let y = (index / width) as u16;
    command.extend(b"PB");
    command.extend(&x.to_le_bytes());
    command.extend(&y.to_le_bytes());
    command.extend(pixel);
    command
}

impl Pixel {
    /// Create a new pixel with the given red, green, and blue components.
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self(Red(red), Green(green), Blue(blue))
    }
}

impl IntoIterator for &Pixel {
    type Item = u8;
    type IntoIter = std::array::IntoIter<u8, 4>;

    fn into_iter(self) -> Self::IntoIter {
        [self.0.0, self.1.0, self.2.0, u8::MAX].into_iter()
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
