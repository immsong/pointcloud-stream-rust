use std::sync::{Arc, RwLock};

use serde_json::{Map, Value};

use crate::publisher::codec::{CodecAdapter, ErasedCodec};
use crate::publisher::protocols::foxglove::adapter::FoxgloveAdapter;
use crate::publisher::protocols::wire::WireSchema;
use crate::publisher::{MessageCodec, PublishError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(u32);

impl ChannelId {
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Clone)]
pub struct Channel {
    pub id: ChannelId,
    pub topic: String,
    pub instance_id: String,
    pub schema: WireSchema,
    pub subscription_metadata: Map<String, Value>,
    pub(crate) codec: Arc<dyn ErasedCodec>,
    pub(crate) foxglove: Option<Arc<dyn FoxgloveAdapter>>,
}

#[derive(Clone, Default)]
pub struct ChannelRegistry {
    channels: Arc<RwLock<Vec<Channel>>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register_message<C: MessageCodec>(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
        codec: C,
    ) -> Result<ChannelId, PublishError> {
        self.register_with_adapter(instance_id, topic, codec, None)
    }

    pub(crate) fn register_with_adapter<C: MessageCodec>(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
        codec: C,
        foxglove: Option<Arc<dyn FoxgloveAdapter>>,
    ) -> Result<ChannelId, PublishError> {
        let schema = codec.schema();

        if schema.schema_name.trim().is_empty()
            || schema.encoding.trim().is_empty()
            || schema.schema_version == 0
        {
            return Err(PublishError::InvalidSchema);
        }

        let metadata = codec.subscription_metadata();

        // 구독 응답의 기본 필드가 코덱 메타데이터와 중복되지 않도록 한다.
        for reserved in [
            "id",
            "channelId",
            "schemaName",
            "schemaVersion",
            "encoding",
            "topic",
            "instanceId",
            "op",
        ] {
            if metadata.contains_key(reserved) {
                return Err(PublishError::ReservedMetadata(reserved));
            }
        }

        let mut channels = self.channels.write().unwrap();
        let id = ChannelId(
            u32::try_from(channels.len()).map_err(|_| PublishError::LengthOverflow("channels"))?,
        );

        channels.push(Channel {
            id,
            topic: topic.into(),
            instance_id: instance_id.into(),
            schema,
            subscription_metadata: metadata,
            codec: Arc::new(CodecAdapter(codec)),
            foxglove,
        });

        Ok(id)
    }

    pub fn get(&self, id: ChannelId) -> Option<Channel> {
        self.get_by_raw_id(id.as_u32())
    }

    pub fn channels(&self) -> Vec<Channel> {
        self.channels.read().unwrap().clone()
    }

    pub fn get_by_raw_id(&self, id: u32) -> Option<Channel> {
        self.channels.read().unwrap().get(id as usize).cloned()
    }
}

#[cfg(test)]
use crate::pointcloud::PointCloudLayout;

#[test]
fn registry_assigns_unique_channel_ids() {
    let registry = ChannelRegistry::new();

    let front = registry.register(
        "instance_1",
        "/lidar/front",
        PointCloudLayout::new(0, Vec::new()),
    );
    let rear = registry.register(
        "instance_1",
        "/lidar/rear",
        PointCloudLayout::new(0, Vec::new()),
    );

    assert_eq!(front.as_u32(), 0);
    assert_eq!(rear.as_u32(), 1);

    assert_eq!(registry.channels().len(), 2);
}

#[test]
fn cloned_registry_shares_registered_channels() {
    let registry = ChannelRegistry::new();
    let cloned_registry = registry.clone();

    let channel_id = registry.register(
        "instance_1",
        "/lidar/front",
        PointCloudLayout::new(0, Vec::new()),
    );

    let channel = cloned_registry
        .get(channel_id)
        .expect("registered channel should be visible from cloned registry");

    assert_eq!(channel.topic, "/lidar/front");
}
