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
}

#[derive(Clone)]
pub struct ChannelRegistry {
    channels: Vec<Channel>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    pub fn register(&mut self, topic: impl Into<String>) -> ChannelId {
        let id = ChannelId(self.channels.len() as u32);
        self.channels.push(Channel {
            id,
            topic: topic.into(),
        });

        id
    }

    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    pub fn get_by_raw_id(&self, id: u32) -> Option<&Channel> {
        self.channels
            .iter()
            .find(|channel| channel.id.as_u32() == id)
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn registry_assigns_unique_channel_ids() {
    let mut registry = ChannelRegistry::new();

    let front = registry.register("/lidar/front");
    let rear = registry.register("/lidar/rear");

    assert_eq!(front.as_u32(), 0);
    assert_eq!(rear.as_u32(), 1);

    assert_eq!(registry.channels().len(), 2);
}
