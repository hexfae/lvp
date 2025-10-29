//! The frame struct representing a single frame of a video.

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
    /// How many pixels are in the frame.
    pub const fn len(&self) -> usize {
        self.pixels.len()
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
