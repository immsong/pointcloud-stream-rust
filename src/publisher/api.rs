//! Point Cloud와 Detection 채널의 등록 및 발행 API.

use std::sync::Arc;

use crate::detection::DetectionInfo;
use crate::pointcloud::{PointCloudFrame, PointCloudLayout};

use crate::publisher::codecs::{DetectionCodec, PointCloudCodec};
use crate::publisher::protocols::foxglove::adapter::PointCloudAdapter;
use crate::publisher::websocket::WebsocketServer;
use crate::publisher::{ChannelId, ChannelRegistry, PublishError};

impl ChannelRegistry {
    /// Wire와 Foxglove 프로토콜에서 사용할 Point Cloud 채널을 등록한다.
    pub fn register_pointcloud(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
        layout: PointCloudLayout,
    ) -> Result<ChannelId, PublishError> {
        self.register_with_adapter(
            instance_id,
            topic,
            PointCloudCodec::new(layout),
            Some(Arc::new(PointCloudAdapter)),
        )
    }

    /// 기존 API와의 호환을 위해 Point Cloud 채널을 등록한다.
    ///
    /// 등록에 실패하면 panic이 발생한다.
    pub fn register(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
        layout: PointCloudLayout,
    ) -> ChannelId {
        self.register_pointcloud(instance_id, topic, layout)
            .expect("built-in PointCloud schema is valid and channel IDs must fit in u32")
    }

    /// Wire 프로토콜에서 사용할 Detection 채널을 등록한다.
    pub fn register_detection(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
    ) -> Result<ChannelId, PublishError> {
        self.register_message(instance_id, topic, DetectionCodec)
    }
}

impl WebsocketServer {
    /// Wire와 Foxglove 프로토콜에서 사용할 Point Cloud 채널을 등록한다.
    ///
    /// 클라이언트에 채널 목록이 전달되도록 연결 전에 호출한다.
    pub fn register_pointcloud_channel(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
        layout: PointCloudLayout,
    ) -> Result<ChannelId, PublishError> {
        self.channels
            .register_pointcloud(instance_id, topic, layout)
    }

    /// 기존 API와의 호환을 위해 Point Cloud 채널을 등록한다.
    ///
    /// 등록에 실패하면 panic이 발생한다.
    pub fn register_channel(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
        layout: PointCloudLayout,
    ) -> ChannelId {
        self.channels.register(instance_id, topic, layout)
    }

    /// Wire 프로토콜에서 사용할 Detection 채널을 등록한다.
    ///
    /// 클라이언트에 채널 목록이 전달되도록 연결 전에 호출한다.
    pub fn register_detection_channel(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
    ) -> Result<ChannelId, PublishError> {
        self.register_message_channel(instance_id, topic, DetectionCodec)
    }

    /// 등록된 채널의 코덱으로 Point Cloud를 검증하고 발행한다.
    pub async fn publish_pointcloud(
        &self,
        channel_id: ChannelId,
        frame: &PointCloudFrame,
    ) -> Result<(), PublishError> {
        self.publish_message(channel_id, frame).await
    }

    /// 등록된 채널의 코덱으로 Detection 정보를 검증하고 발행한다.
    pub async fn publish_detection(
        &self,
        channel_id: ChannelId,
        info: &DetectionInfo,
    ) -> Result<(), PublishError> {
        self.publish_message(channel_id, info).await
    }
}
