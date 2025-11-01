//! The client struct responsible for interacting with the S3 bucket.

use std::env::{VarError, var};

use aws_sdk_s3::{
    error::SdkError,
    operation::{get_object::GetObjectError, list_objects_v2::ListObjectsV2Error},
};
use rand::{rng, seq::IndexedRandom};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::video::{ReadVideoError, Video};

/// A wrapper around the S3 client.
pub struct Client {
    /// The S3 client.
    client: aws_sdk_s3::Client,
    /// The name of the S3 bucket containing videos.
    bucket_name: String,
}

/// The name of a video.
pub struct VideoName(String);

#[derive(Debug, Snafu)]
pub enum ClientError {
    /// The `S3_BUCKET_NAME` environment variable is not set.
    #[snafu(display("S3_BUCKET_NAME environment variable is not set"))]
    BucketNameUnsetError {
        /// The source of the error.
        source: VarError,
    },
    /// An error occurred while listing videos.
    #[snafu(display("An error occurred while listing videos"))]
    ListVideos {
        /// The source of the error.
        source: SdkError<ListObjectsV2Error>,
    },
    /// There are no videos found in the bucket.
    #[snafu(display("No videos found in the S3 bucket"))]
    NoVideosFound,
    /// An error occurred while getting a video.
    #[snafu(display("An error occurred while getting a video"))]
    GetObject {
        /// The source of the error.
        #[snafu(source(from(SdkError<GetObjectError>, Box::new)))]
        source: Box<SdkError<GetObjectError>>,
    },
    /// An error occurred while reading a video.
    #[snafu(display("An error occurred while reading a video"))]
    ReadVideo {
        /// The source of the error.
        source: ReadVideoError,
    },
}

impl Client {
    /// Creates a new client.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `S3_BUCKET_NAME` environment variable is not set.
    pub async fn new() -> Result<Self, ClientError> {
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
    pub async fn list_videos(&self) -> Result<Vec<VideoName>, ClientError> {
        let resp = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket_name)
            .send()
            .await
            .context(ListVideosSnafu)?;

        let names = resp
            .contents()
            .iter()
            .filter_map(|obj| obj.key.as_ref().map(|s| VideoName(s.clone())))
            .collect();

        Ok(names)
    }

    /// Returns a video by name.
    ///
    /// # Errors
    ///
    /// This function will return an error if the S3 API call fails or if the video is empty.
    pub async fn select_video(&self, name: &VideoName) -> Result<Video, ClientError> {
        let object = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(name.as_ref())
            .send()
            .await
            .context(GetObjectSnafu)?;

        let video = Video::from_object(object).await.context(ReadVideoSnafu)?;

        Ok(video)
    }

    pub async fn random_video(&self) -> Result<Video, ClientError> {
        let videos = self.list_videos().await?;
        let random_video_name = videos.choose(&mut rng()).context(NoVideosFoundSnafu)?;
        self.select_video(random_video_name).await
    }
}

impl AsRef<str> for VideoName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
