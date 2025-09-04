//! LUDD video processor.
#![feature(thread_sleep_until)]
pub mod client;
pub mod video;

pub use client::Client;
pub use video::ProcessingError;
pub use video::Video;
