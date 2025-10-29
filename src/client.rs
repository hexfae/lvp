//! The client struct responsible for interacting with the S3 bucket.

use std::{
    env::{VarError, var},
    fmt::Display,
};

use aws_sdk_s3::{error::SdkError, operation::list_objects_v2::ListObjectsV2Error};
use snafu::{ResultExt, Snafu};

/// A wrapper around the S3 client.
pub struct Client {
    /// The S3 client.
    client: aws_sdk_s3::Client,
    /// The name of the S3 bucket containing videos.
    bucket_name: String,
}

/// The name of a video.
pub struct VideoName(String);

/// The `S3_BUCKET_NAME` environment variable is not set.
#[derive(Debug, Snafu)]
#[snafu(display("S3_BUCKET_NAME environment variable is not set"))]
pub struct BucketNameUnsetError {
    /// The source of the error.
    source: VarError,
}

/// An error occurred while listing videos.
#[derive(Debug, Snafu)]
#[snafu(transparent)]
pub struct ListVideosError {
    /// The source of the error.
    source: SdkError<ListObjectsV2Error>,
}

impl Client {
    /// Creates a new client.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `S3_BUCKET_NAME` environment variable is not set.
    pub async fn new() -> Result<Self, BucketNameUnsetError> {
        let sdk_config = aws_config::load_from_env().await;
        let config = aws_sdk_s3::config::Builder::from(&sdk_config)
            .force_path_style(true)
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);

        let bucket_name = var("S3_BUCKET_NAME").context(BucketNameUnsetSnafu)?;

        Ok(Self {
            client,
            bucket_name,
        })
    }

    /// Returns a list of video names. Returns an empty list if no videos are found.
    ///
    /// # Errors
    ///
    /// This function will return an error if the S3 API call fails.
    pub async fn list_videos(&self) -> Result<Vec<VideoName>, ListVideosError> {
        let resp = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket_name)
            .send()
            .await?;

        let names = resp
            .contents()
            .to_owned()
            .into_iter()
            .filter_map(|obj| obj.key.map(VideoName))
            .collect();

        Ok(names)
    }
}

impl Display for VideoName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
