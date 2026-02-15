//! The client struct responsible for interacting with the S3 bucket.

use crate::video::{ReadVideoError, Video};
use aws_sdk_s3::{
    Client,
    error::SdkError,
    operation::{get_object::GetObjectError, list_objects_v2::ListObjectsV2Error},
    presigning::PresigningConfig,
};
use snafu::{OptionExt, ResultExt, Snafu};
use std::{
    env::{VarError, var},
    time::Duration,
};
use url::Url;

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
    /// An error occured while parsing a video's URL.
    #[snafu(display("An error occured while parsing a video's URL."))]
    ParseUrlError {
        /// The source of the error.
        source: url::ParseError,
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

    /// Gets the url for a video from its name.
    ///
    /// # Errors
    ///
    /// Returns an error if getting the video from the S3 bucket failed,
    /// or parsing the video's url failed.
    ///
    /// # Panics
    ///
    /// Panics if the presigning config is not given an expiration time,
    /// or if it's longer than 1 week (neither of which are true).
    pub async fn get_video_url(&self, name: &str) -> Result<Url, S3Error> {
        let presigning_config = PresigningConfig::expires_in(Duration::from_mins(5))
            .expect("presigning config should be given and be less than 1 week");

        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(name)
            .presigned(presigning_config)
            .await
            .context(GetObjectSnafu)?;

        let url = Url::parse(presigned_request.uri()).context(ParseUrlSnafu)?;
        Ok(url)
    }

    /// Returns a random video from the S3 bucket.
    ///
    /// # Errors
    ///
    /// Returns an error if it could not connect to the S3 bucket,
    /// there are no videos in the S3 bucket, or parsing the video failed.
    pub async fn random_video(&self) -> Result<Video, S3Error> {
        let videos = self.video_names().await?;
        let random_index = fastrand::usize(..videos.len());
        let random_video_name = videos.get(random_index).context(NoVideosFoundSnafu)?;
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
        let url = self.get_video_url(name.as_ref()).await?;

        let video = Video::from_url(url).await.context(ReadVideoSnafu)?;

        Ok(video)
    }
}
