//! LUDD video processor.

use std::thread::available_parallelism;

use clap::Parser;

pub mod client;
pub mod frame;
pub mod network;
pub mod video;

pub static CONFIG: std::sync::LazyLock<Config> = std::sync::LazyLock::new(Config::parse);

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {
    #[arg(long, env = "AWS_ACCESS_KEY_ID")]
    pub aws_access_key_id: String,

    #[arg(long, env = "AWS_SECRET_ACCESS_KEY")]
    pub aws_secret_access_key: String,

    #[arg(long, env = "AWS_ENDPOINT_URL")]
    pub aws_endpoint_url: String,

    #[arg(long, env = "AWS_REGION", default_value = "default")]
    pub aws_region: String,

    #[arg(long, env = "S3_BUCKET_NAME", default_value = "lvp")]
    pub s3_bucket_name: String,

    #[arg(long, env = "PIXELFLUT_ADDRESS")]
    pub pixelflut_address: String,

    #[arg(long, env = "LVP_MAX_WIDTH", default_value_t = 640)]
    pub max_width: u32,

    #[arg(long, env = "LVP_MAX_HEIGHT", default_value_t = 540)]
    pub max_height: u32,

    #[arg(long, env = "PIXELFLUT_WIDTH")]
    pub pixelflut_width: Option<u32>,

    #[arg(long, env = "PIXELFLUT_HEIGHT")]
    pub pixelflut_height: Option<u32>,

    #[arg(long, env = "NUMBER_OF_STREAMS", default_value_t = available_parallelism().map(|n| n.get()).unwrap_or(10))]
    pub n_streams: usize,
}
