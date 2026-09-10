use std::sync::{Arc, RwLock};

use crate::pointcloud::PointCloudLayout;

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
    pub layout: PointCloudLayout,
}

#[derive(Clone)]
pub struct ChannelRegistry {
    channels: Arc<RwLock<Vec<Channel>>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn register(&self, topic: impl Into<String>, layout: PointCloudLayout) -> ChannelId {
        let mut channels = self.channels.write().unwrap();
        let id = ChannelId(channels.len() as u32);

        channels.push(Channel {
            id,
            topic: topic.into(),
            layout,
        });

        id
    }

    pub fn get(&self, id: ChannelId) -> Option<Channel> {
        self.channels
            .read()
            .unwrap()
            .iter()
            .find(|channel| channel.id == id)
            .cloned()
    }

    pub fn channels(&self) -> Vec<Channel> {
        self.channels.read().unwrap().clone()
    }

    pub fn get_by_raw_id(&self, id: u32) -> Option<Channel> {
        self.channels
            .read()
            .unwrap()
            .iter()
            .find(|channel| channel.id.as_u32() == id)
            .cloned()
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn registry_assigns_unique_channel_ids() {
    let registry = ChannelRegistry::new();

    let front = registry.register("/lidar/front", PointCloudLayout::new(0, Vec::new()));
    let rear = registry.register("/lidar/rear", PointCloudLayout::new(0, Vec::new()));

    assert_eq!(front.as_u32(), 0);
    assert_eq!(rear.as_u32(), 1);

    assert_eq!(registry.channels().len(), 2);
}

#[test]
fn cloned_registry_shares_registered_channels() {
    let registry = ChannelRegistry::new();
    let cloned_registry = registry.clone();

    let channel_id = registry.register("/lidar/front", PointCloudLayout::new(0, Vec::new()));

    let channel = cloned_registry
        .get(channel_id)
        .expect("registered channel should be visible from cloned registry");

    assert_eq!(channel.topic, "/lidar/front");
}
