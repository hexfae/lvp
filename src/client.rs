//! The client struct responsible for interacting with the S3 bucket.

use crate::video::{ReadVideoError, Video};
use aws_sdk_s3::{
    Client,
    error::SdkError,
    operation::{get_object::GetObjectError, list_objects_v2::ListObjectsV2Error},
};
use rand::{rng, seq::IndexedRandom};
use snafu::{OptionExt, ResultExt, Snafu};
use std::env::{VarError, var};

/// A wrapper around the S3 client.
pub struct S3Client {
    /// The S3 client.
    client: Client,
    /// The name of the S3 bucket containing videos.
    bucket_name: String,
}

/// An error occurred with the S3 client.
#[derive(Debug, Snafu)]
pub enum S3Error {
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
        #[snafu(source(from(SdkError<ListObjectsV2Error>, Box::new)))]
        source: Box<SdkError<ListObjectsV2Error>>,
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

impl S3Client {
    /// Creates a new client.
    ///
    /// # Errors
    ///
    /// This function will return an error if the `S3_BUCKET_NAME` environment variable is not set.
    pub async fn new() -> Result<Self, S3Error> {
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

    /// Returns a random video from the S3 bucket.
    pub async fn random_video(&self) -> Result<Video, S3Error> {
        let videos = self.video_names().await?;
        let random_video_name = videos.choose(&mut rng()).context(NoVideosFoundSnafu)?;
        self.select_video(random_video_name).await
    }

    /// Returns a list of video names. Returns an empty list if no videos are found.
    ///
    /// # Errors
    ///
    /// This function will return an error if the S3 API call fails.
    async fn video_names(&self) -> Result<Vec<String>, S3Error> {
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
            .filter_map(|obj| obj.key.clone())
            .collect();

        Ok(names)
    }

    /// Returns a video by name.
    ///
    /// # Errors
    ///
    /// This function will return an error if the S3 API call fails or if the video is empty.
    async fn select_video(&self, name: impl AsRef<str>) -> Result<Video, S3Error> {
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
}
